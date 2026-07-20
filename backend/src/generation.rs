use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    auth::AuthError,
    config::GenerationConfig,
    db::models::ContentSettings,
    AppState,
};

#[derive(Clone)]
pub struct ImageGenerationClient {
    config: GenerationConfig,
    client: reqwest::Client,
}

#[derive(Debug, Deserialize)]
struct GenerateImageRequest {
    subject_hint: Option<String>,
}

#[derive(Debug, Serialize)]
struct GenerateImageResponse {
    image: GeneratedImagePayload,
}

#[derive(Debug, Serialize)]
pub struct GeneratedImagePayload {
    pub image_base64: String,
    pub mime_type: String,
    pub prompt: String,
    pub revised_prompt: Option<String>,
    pub size: String,
    pub model: String,
    pub byte_length: usize,
}

#[derive(Debug, Deserialize)]
struct OpenAIImageResponse {
    data: Vec<OpenAIImageData>,
}

#[derive(Debug, Deserialize)]
struct OpenAIImageData {
    b64_json: Option<String>,
    revised_prompt: Option<String>,
}

#[derive(Debug, Error)]
pub enum GenerationError {
    #[error("{0}")]
    Auth(#[from] AuthError),
    #[error("image generation is not configured")]
    MissingConfig,
    #[error("content settings are required before generating images")]
    MissingContentSettings,
    #[error("{0}")]
    Validation(String),
    #[error("OpenAI image response did not include base64 image data")]
    MissingImageData,
    #[error("OpenAI image response included invalid base64 image data")]
    InvalidImageData,
    #[error("image generation request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("database operation failed: {0}")]
    Database(#[from] sqlx::Error),
}

pub fn routes() -> Router<AppState> {
    Router::new().route("/image", post(generate_image))
}

pub async fn generate_flat_illustration_for_creator(
    state: &AppState,
    creator_id: Uuid,
    subject_hint: Option<&str>,
) -> Result<GeneratedImagePayload, GenerationError> {
    let settings = state
        .repository
        .get_content_settings(creator_id)
        .await?
        .ok_or(GenerationError::MissingContentSettings)?;
    let client = image_client(state)?;

    client.generate_flat_illustration(&settings, subject_hint).await
}

async fn generate_image(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<GenerateImageRequest>,
) -> Result<Json<GenerateImageResponse>, GenerationError> {
    let creator = state.auth.current_creator(&headers).await?;
    let subject_hint = request.validate_subject_hint()?;
    let image =
        generate_flat_illustration_for_creator(&state, creator.creator.id, subject_hint.as_deref())
            .await?;

    Ok(Json(GenerateImageResponse { image }))
}

impl GenerateImageRequest {
    fn validate_subject_hint(self) -> Result<Option<String>, GenerationError> {
        let Some(value) = self.subject_hint else {
            return Ok(None);
        };
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Ok(None);
        }
        if trimmed.len() > 240 {
            return Err(GenerationError::Validation(
                "Image subject hint must be 240 characters or fewer.".to_owned(),
            ));
        }

        Ok(Some(trimmed.to_owned()))
    }
}

impl ImageGenerationClient {
    async fn generate_flat_illustration(
        &self,
        settings: &ContentSettings,
        subject_hint: Option<&str>,
    ) -> Result<GeneratedImagePayload, GenerationError> {
        let prompt = illustration_prompt(settings, subject_hint);
        let body = serde_json::json!({
            "model": &self.config.image_model,
            "prompt": &prompt,
            "n": 1,
            "size": &self.config.image_size,
            "quality": &self.config.image_quality,
            "output_format": &self.config.image_format,
        });

        let response = self
            .client
            .post(&self.config.image_generation_url)
            .bearer_auth(&self.config.openai_api_key)
            .json(&body)
            .send()
            .await?
            .error_for_status()?
            .json::<OpenAIImageResponse>()
            .await?;

        let first = response
            .data
            .into_iter()
            .next()
            .ok_or(GenerationError::MissingImageData)?;
        let image_base64 = first.b64_json.ok_or(GenerationError::MissingImageData)?;
        let byte_length = STANDARD
            .decode(&image_base64)
            .map_err(|_| GenerationError::InvalidImageData)?
            .len();

        Ok(GeneratedImagePayload {
            image_base64,
            mime_type: format!("image/{}", self.config.image_format),
            prompt,
            revised_prompt: first.revised_prompt,
            size: self.config.image_size.clone(),
            model: self.config.image_model.clone(),
            byte_length,
        })
    }
}

fn image_client(state: &AppState) -> Result<ImageGenerationClient, GenerationError> {
    let config = state
        .config
        .generation
        .clone()
        .ok_or(GenerationError::MissingConfig)?;

    Ok(ImageGenerationClient {
        config,
        client: reqwest::Client::new(),
    })
}

fn illustration_prompt(settings: &ContentSettings, subject_hint: Option<&str>) -> String {
    let subject = subject_hint.unwrap_or(settings.theme_topic.as_str());
    format!(
        "Create one simple flat vector-style illustration for an Instagram post. \
Use a clean minimal composition, solid shapes, soft contrast, limited color palette, \
no photorealism, no tiny details, no text, no letters, no logos, no watermark. \
Creator theme/topic: {}. Creator style notes: {}. Specific subject for this post: {}.",
        settings.theme_topic, settings.style_notes, subject
    )
}

impl IntoResponse for GenerationError {
    fn into_response(self) -> Response {
        match self {
            GenerationError::Auth(error) => error.into_response(),
            GenerationError::Validation(message) => {
                (StatusCode::BAD_REQUEST, Json(error_body(message))).into_response()
            }
            GenerationError::MissingContentSettings => (
                StatusCode::BAD_REQUEST,
                Json(error_body(
                    "Content settings are required before generating images.".to_owned(),
                )),
            )
                .into_response(),
            GenerationError::MissingConfig => (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(error_body("Image generation is not configured.".to_owned())),
            )
                .into_response(),
            error @ (GenerationError::MissingImageData | GenerationError::InvalidImageData) => {
                tracing::error!(%error, "OpenAI image generation returned invalid data");
                (
                    StatusCode::BAD_GATEWAY,
                    Json(error_body("Image generation returned invalid data.".to_owned())),
                )
                    .into_response()
            }
            GenerationError::Request(error) => {
                tracing::error!(%error, "OpenAI image generation request failed");
                (
                    StatusCode::BAD_GATEWAY,
                    Json(error_body("Image generation failed.".to_owned())),
                )
                    .into_response()
            }
            GenerationError::Database(error) => {
                tracing::error!(%error, "image generation database operation failed");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(error_body("Image generation could not load settings.".to_owned())),
                )
                    .into_response()
            }
        }
    }
}

fn error_body(message: String) -> serde_json::Value {
    serde_json::json!({ "error": message })
}
