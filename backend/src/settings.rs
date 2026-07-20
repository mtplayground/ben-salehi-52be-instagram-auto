use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    auth::AuthError,
    db::{
        models::ContentSettings,
        repository::NewContentSettings,
    },
    AppState,
};

#[derive(Debug, Serialize)]
struct ContentSettingsPayload {
    id: Uuid,
    creator_id: Uuid,
    theme_topic: String,
    style_notes: String,
    review_mode_enabled: bool,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Serialize)]
struct ContentSettingsResponse {
    settings: Option<ContentSettingsPayload>,
}

#[derive(Debug, Deserialize)]
struct SaveContentSettingsRequest {
    theme_topic: String,
    style_notes: String,
    #[serde(default = "default_review_mode_enabled")]
    review_mode_enabled: bool,
}

#[derive(Debug, Error)]
enum SettingsError {
    #[error("{0}")]
    Auth(#[from] AuthError),
    #[error("database operation failed: {0}")]
    Database(#[from] sqlx::Error),
    #[error("{0}")]
    Validation(String),
}

pub fn routes() -> Router<AppState> {
    Router::new().route("/content", get(get_content).put(save_content))
}

async fn get_content(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ContentSettingsResponse>, SettingsError> {
    let creator = state.auth.current_creator(&headers).await?;
    let settings = state
        .repository
        .get_content_settings(creator.creator.id)
        .await?
        .map(ContentSettingsPayload::from);

    Ok(Json(ContentSettingsResponse { settings }))
}

async fn save_content(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<SaveContentSettingsRequest>,
) -> Result<Json<ContentSettingsResponse>, SettingsError> {
    let creator = state.auth.current_creator(&headers).await?;
    let request = request.validate()?;
    let settings = state
        .repository
        .upsert_content_settings(NewContentSettings {
            creator_id: creator.creator.id,
            theme_topic: &request.theme_topic,
            style_notes: &request.style_notes,
            review_mode_enabled: request.review_mode_enabled,
        })
        .await?;

    Ok(Json(ContentSettingsResponse {
        settings: Some(settings.into()),
    }))
}

impl SaveContentSettingsRequest {
    fn validate(self) -> Result<Self, SettingsError> {
        let theme_topic = self.theme_topic.trim().to_owned();
        let style_notes = self.style_notes.trim().to_owned();

        if theme_topic.len() < 3 {
            return Err(SettingsError::Validation(
                "Theme or topic must be at least 3 characters.".to_owned(),
            ));
        }

        if theme_topic.len() > 180 {
            return Err(SettingsError::Validation(
                "Theme or topic must be 180 characters or fewer.".to_owned(),
            ));
        }

        if style_notes.len() < 3 {
            return Err(SettingsError::Validation(
                "Style notes must be at least 3 characters.".to_owned(),
            ));
        }

        if style_notes.len() > 1200 {
            return Err(SettingsError::Validation(
                "Style notes must be 1200 characters or fewer.".to_owned(),
            ));
        }

        Ok(Self {
            theme_topic,
            style_notes,
            review_mode_enabled: self.review_mode_enabled,
        })
    }
}

fn default_review_mode_enabled() -> bool {
    true
}

impl From<ContentSettings> for ContentSettingsPayload {
    fn from(settings: ContentSettings) -> Self {
        Self {
            id: settings.id,
            creator_id: settings.creator_id,
            theme_topic: settings.theme_topic,
            style_notes: settings.style_notes,
            review_mode_enabled: settings.review_mode_enabled,
            created_at: settings.created_at.to_rfc3339(),
            updated_at: settings.updated_at.to_rfc3339(),
        }
    }
}

impl IntoResponse for SettingsError {
    fn into_response(self) -> Response {
        match self {
            SettingsError::Auth(error) => error.into_response(),
            SettingsError::Validation(message) => {
                (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "error": message })))
                    .into_response()
            }
            SettingsError::Database(error) => {
                tracing::error!(%error, "settings database operation failed");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": "settings could not be saved" })),
                )
                    .into_response()
            }
        }
    }
}
