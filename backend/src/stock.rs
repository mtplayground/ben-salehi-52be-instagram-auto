use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    auth::AuthError,
    config::StockImageConfig,
    db::models::ContentSettings,
    AppState,
};

const PEXELS_LICENSE_URL: &str = "https://www.pexels.com/license/";

#[derive(Clone)]
pub struct StockImageClient {
    config: StockImageConfig,
    client: reqwest::Client,
}

#[derive(Debug, Deserialize)]
struct SourceStockImageRequest {
    subject_hint: Option<String>,
}

#[derive(Debug, Serialize)]
struct SourceStockImageResponse {
    image: StockImagePayload,
}

#[derive(Debug, Serialize)]
pub struct StockImagePayload {
    pub provider: String,
    pub provider_asset_id: String,
    pub image_url: String,
    pub preview_url: String,
    pub source_url: String,
    pub width: i32,
    pub height: i32,
    pub alt_text: Option<String>,
    pub photographer_name: String,
    pub photographer_url: String,
    pub license_name: String,
    pub license_url: String,
    pub attribution_required: bool,
    pub query: String,
}

#[derive(Debug, Deserialize)]
struct PexelsSearchResponse {
    photos: Vec<PexelsPhoto>,
}

#[derive(Debug, Deserialize)]
struct PexelsPhoto {
    id: i64,
    width: i32,
    height: i32,
    url: String,
    photographer: String,
    photographer_url: String,
    src: PexelsPhotoSources,
    alt: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PexelsPhotoSources {
    large2x: Option<String>,
    large: String,
    medium: String,
}

#[derive(Debug, Error)]
pub enum StockImageError {
    #[error("{0}")]
    Auth(#[from] AuthError),
    #[error("stock image sourcing is not configured")]
    MissingConfig,
    #[error("content settings are required before sourcing stock images")]
    MissingContentSettings,
    #[error("{0}")]
    Validation(String),
    #[error("stock image provider returned no suitable royalty-free images")]
    NoResults,
    #[error("stock image provider request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("database operation failed: {0}")]
    Database(#[from] sqlx::Error),
}

pub fn routes() -> Router<AppState> {
    Router::new().route("/image", post(source_stock_image))
}

pub async fn source_royalty_free_stock_image_for_creator(
    state: &AppState,
    creator_id: Uuid,
    subject_hint: Option<&str>,
) -> Result<StockImagePayload, StockImageError> {
    let settings = state
        .repository
        .get_content_settings(creator_id)
        .await?
        .ok_or(StockImageError::MissingContentSettings)?;
    let client = stock_image_client(state)?;

    client.source_image(&settings, subject_hint).await
}

async fn source_stock_image(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<SourceStockImageRequest>,
) -> Result<Json<SourceStockImageResponse>, StockImageError> {
    let creator = state.auth.current_creator(&headers).await?;
    let subject_hint = request.validate_subject_hint()?;
    let image = source_royalty_free_stock_image_for_creator(
        &state,
        creator.creator.id,
        subject_hint.as_deref(),
    )
    .await?;

    Ok(Json(SourceStockImageResponse { image }))
}

impl SourceStockImageRequest {
    fn validate_subject_hint(self) -> Result<Option<String>, StockImageError> {
        let Some(value) = self.subject_hint else {
            return Ok(None);
        };
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Ok(None);
        }
        if trimmed.len() > 120 {
            return Err(StockImageError::Validation(
                "Stock image subject hint must be 120 characters or fewer.".to_owned(),
            ));
        }

        Ok(Some(trimmed.to_owned()))
    }
}

impl StockImageClient {
    async fn source_image(
        &self,
        settings: &ContentSettings,
        subject_hint: Option<&str>,
    ) -> Result<StockImagePayload, StockImageError> {
        let query = stock_query(settings, subject_hint);
        let per_page = self.config.per_page.to_string();
        let response = self
            .client
            .get(&self.config.search_url)
            .header("Authorization", &self.config.api_key)
            .query(&[
                ("query", query.as_str()),
                ("orientation", "square"),
                ("size", "medium"),
                ("per_page", per_page.as_str()),
            ])
            .send()
            .await?
            .error_for_status()?
            .json::<PexelsSearchResponse>()
            .await?;

        let photo = response.photos.into_iter().next().ok_or(StockImageError::NoResults)?;
        Ok(StockImagePayload::from_pexels_photo(
            self.config.provider.clone(),
            query,
            photo,
        ))
    }
}

impl StockImagePayload {
    fn from_pexels_photo(provider: String, query: String, photo: PexelsPhoto) -> Self {
        let image_url = photo
            .src
            .large2x
            .clone()
            .unwrap_or_else(|| photo.src.large.clone());

        Self {
            provider,
            provider_asset_id: photo.id.to_string(),
            image_url,
            preview_url: photo.src.medium,
            source_url: photo.url,
            width: photo.width,
            height: photo.height,
            alt_text: photo.alt,
            photographer_name: photo.photographer,
            photographer_url: photo.photographer_url,
            license_name: "Pexels License".to_owned(),
            license_url: PEXELS_LICENSE_URL.to_owned(),
            attribution_required: false,
            query,
        }
    }
}

fn stock_image_client(state: &AppState) -> Result<StockImageClient, StockImageError> {
    let config = state
        .config
        .stock_images
        .clone()
        .ok_or(StockImageError::MissingConfig)?;

    Ok(StockImageClient {
        config,
        client: reqwest::Client::new(),
    })
}

fn stock_query(settings: &ContentSettings, subject_hint: Option<&str>) -> String {
    let subject = subject_hint.unwrap_or(settings.theme_topic.as_str());
    let mut query = format!("{} {}", settings.theme_topic, subject);
    if !settings.style_notes.trim().is_empty() {
        query.push(' ');
        query.push_str(settings.style_notes.trim());
    }

    query
        .split_whitespace()
        .take(12)
        .collect::<Vec<&str>>()
        .join(" ")
}

impl IntoResponse for StockImageError {
    fn into_response(self) -> Response {
        match self {
            StockImageError::Auth(error) => error.into_response(),
            StockImageError::Validation(message) => {
                (StatusCode::BAD_REQUEST, Json(error_body(message))).into_response()
            }
            StockImageError::MissingContentSettings => (
                StatusCode::BAD_REQUEST,
                Json(error_body(
                    "Content settings are required before sourcing stock images.".to_owned(),
                )),
            )
                .into_response(),
            StockImageError::MissingConfig => (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(error_body("Stock image sourcing is not configured.".to_owned())),
            )
                .into_response(),
            StockImageError::NoResults => (
                StatusCode::NOT_FOUND,
                Json(error_body(
                    "No royalty-free stock image matched this creator theme.".to_owned(),
                )),
            )
                .into_response(),
            StockImageError::Request(error) => {
                tracing::error!(%error, "stock image provider request failed");
                (
                    StatusCode::BAD_GATEWAY,
                    Json(error_body("Stock image sourcing failed.".to_owned())),
                )
                    .into_response()
            }
            StockImageError::Database(error) => {
                tracing::error!(%error, "stock image database operation failed");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(error_body("Stock image sourcing could not load settings.".to_owned())),
                )
                    .into_response()
            }
        }
    }
}

fn error_body(message: String) -> serde_json::Value {
    serde_json::json!({ "error": message })
}
