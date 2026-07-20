use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    auth::AuthError,
    compositor::{compose_text_overlay, CompositedImagePayload, CompositorError, ImageSource, OverlayTextInput},
    db::{
        models::{GeneratedPost, MediaAsset, PostStatus},
        repository::ReviewPostUpdate,
    },
    generation::{generate_casual_caption_for_creator, generate_flat_illustration_for_creator, GenerationError},
    stock::{source_royalty_free_stock_image_for_creator, StockImageError},
    storage::{store_finished_image_for_creator, StoreFinishedImageInput, StorageError},
    AppState,
};

const MAX_HEADER_CHARS: usize = 64;
const MAX_PARAGRAPH_CHARS: usize = 220;
const MAX_CAPTION_CHARS: usize = 2200;

#[derive(Debug, Serialize)]
struct ReviewPostResponse {
    post: GeneratedPostPayload,
}

#[derive(Debug, Serialize)]
struct GeneratedPostPayload {
    id: Uuid,
    media_asset_id: Option<Uuid>,
    image_reference: Option<String>,
    header_text: String,
    paragraph_text: String,
    caption: String,
    status: String,
    scheduled_at: Option<String>,
    updated_at: String,
}

#[derive(Debug, Deserialize)]
struct EditPostRequest {
    header_text: String,
    paragraph_text: String,
    caption: String,
}

#[derive(Debug, Error)]
enum ReviewError {
    #[error("{0}")]
    Auth(#[from] AuthError),
    #[error("post was not found for this creator")]
    PostNotFound,
    #[error("{0}")]
    Validation(String),
    #[error("generation failed: {0}")]
    Generation(#[from] GenerationError),
    #[error("stock fallback failed: {0}")]
    Stock(#[from] StockImageError),
    #[error("image compositing failed: {0}")]
    Compositor(#[from] CompositorError),
    #[error("finished image storage failed: {0}")]
    Storage(#[from] StorageError),
    #[error("database operation failed: {0}")]
    Database(#[from] sqlx::Error),
}

#[derive(Debug)]
struct PreparedImage {
    source: ImageSource,
    reference: String,
    media_source: String,
    license: Option<String>,
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/:post_id/approve", post(approve_post))
        .route("/:post_id/reject", post(reject_post))
        .route("/:post_id/regenerate", post(regenerate_post))
        .route("/:post_id", put(edit_post))
}

async fn approve_post(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(post_id): Path<Uuid>,
) -> Result<Json<ReviewPostResponse>, ReviewError> {
    let creator = state.auth.current_creator(&headers).await?;
    let post = state
        .repository
        .update_generated_post_status(creator.creator.id, post_id, PostStatus::Approved)
        .await?
        .ok_or(ReviewError::PostNotFound)?;

    Ok(Json(ReviewPostResponse { post: post.into() }))
}

async fn reject_post(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(post_id): Path<Uuid>,
) -> Result<Json<ReviewPostResponse>, ReviewError> {
    let creator = state.auth.current_creator(&headers).await?;
    let post = state
        .repository
        .update_generated_post_status(creator.creator.id, post_id, PostStatus::Rejected)
        .await?
        .ok_or(ReviewError::PostNotFound)?;

    Ok(Json(ReviewPostResponse { post: post.into() }))
}

async fn edit_post(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(post_id): Path<Uuid>,
    Json(request): Json<EditPostRequest>,
) -> Result<Json<ReviewPostResponse>, ReviewError> {
    let creator = state.auth.current_creator(&headers).await?;
    state
        .repository
        .get_generated_post_for_creator(creator.creator.id, post_id)
        .await?
        .ok_or(ReviewError::PostNotFound)?;
    let request = request.validate()?;
    let post = state
        .repository
        .update_generated_post_review(ReviewPostUpdate {
            creator_id: creator.creator.id,
            post_id,
            media_asset_id: None,
            image_reference: None,
            header_text: &request.header_text,
            paragraph_text: &request.paragraph_text,
            caption: &request.caption,
            status: PostStatus::PendingReview,
        })
        .await?
        .ok_or(ReviewError::PostNotFound)?;

    Ok(Json(ReviewPostResponse { post: post.into() }))
}

async fn regenerate_post(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(post_id): Path<Uuid>,
) -> Result<Json<ReviewPostResponse>, ReviewError> {
    let creator = state.auth.current_creator(&headers).await?;
    let current = state
        .repository
        .get_generated_post_for_creator(creator.creator.id, post_id)
        .await?
        .ok_or(ReviewError::PostNotFound)?;
    let prepared = prepare_image(&state, creator.creator.id, &current.header_text).await?;
    let caption = generate_casual_caption_for_creator(
        &state,
        creator.creator.id,
        &current.header_text,
        &current.paragraph_text,
        current.image_reference.as_deref(),
    )
    .await?;
    let composited = compose_text_overlay(OverlayTextInput {
        image_source: prepared.source,
        header_text: current.header_text.clone(),
        paragraph_text: current.paragraph_text.clone(),
    })?;
    let media = store_image(
        &state,
        creator.creator.id,
        composited,
        prepared.media_source,
        prepared.license,
    )
    .await?;
    let post = state
        .repository
        .update_generated_post_review(ReviewPostUpdate {
            creator_id: creator.creator.id,
            post_id,
            media_asset_id: Some(media.id),
            image_reference: Some(&prepared.reference),
            header_text: &current.header_text,
            paragraph_text: &current.paragraph_text,
            caption: &caption.caption,
            status: PostStatus::PendingReview,
        })
        .await?
        .ok_or(ReviewError::PostNotFound)?;

    Ok(Json(ReviewPostResponse { post: post.into() }))
}

async fn prepare_image(
    state: &AppState,
    creator_id: Uuid,
    subject_hint: &str,
) -> Result<PreparedImage, ReviewError> {
    match generate_flat_illustration_for_creator(state, creator_id, Some(subject_hint)).await {
        Ok(image) => {
            let reference = image.revised_prompt.unwrap_or(image.prompt);
            Ok(PreparedImage {
                source: ImageSource::DataUri {
                    mime_type: image.mime_type,
                    image_base64: image.image_base64,
                },
                reference,
                media_source: "review-regenerated-illustration".to_owned(),
                license: None,
            })
        }
        Err(error) => {
            tracing::warn!(%error, creator_id = %creator_id, "review illustration regeneration failed, trying stock fallback");
            let stock = source_royalty_free_stock_image_for_creator(
                state,
                creator_id,
                Some(subject_hint),
            )
            .await?;

            Ok(PreparedImage {
                source: ImageSource::Url(stock.image_url),
                reference: stock.source_url,
                media_source: format!("{}-stock-review-regenerated", stock.provider),
                license: Some(stock.license_url),
            })
        }
    }
}

async fn store_image(
    state: &AppState,
    creator_id: Uuid,
    image: CompositedImagePayload,
    source: String,
    license: Option<String>,
) -> Result<MediaAsset, ReviewError> {
    let width = i32::try_from(image.width).map_err(|_| {
        ReviewError::Validation("Composited image width is too large.".to_owned())
    })?;
    let height = i32::try_from(image.height).map_err(|_| {
        ReviewError::Validation("Composited image height is too large.".to_owned())
    })?;
    let (media, _) = store_finished_image_for_creator(
        state,
        creator_id,
        StoreFinishedImageInput {
            image_base64: image.image_base64,
            mime_type: image.mime_type,
            source: Some(source),
            license,
            width: Some(width),
            height: Some(height),
            post_id: None,
        },
    )
    .await?;

    Ok(media)
}

impl EditPostRequest {
    fn validate(self) -> Result<Self, ReviewError> {
        let header_text = normalize_text(&self.header_text);
        let paragraph_text = normalize_text(&self.paragraph_text);
        let caption = self.caption.trim().to_owned();

        if header_text.is_empty() || header_text.chars().count() > MAX_HEADER_CHARS {
            return Err(ReviewError::Validation(
                "Header must be between 1 and 64 characters.".to_owned(),
            ));
        }
        if paragraph_text.is_empty() || paragraph_text.chars().count() > MAX_PARAGRAPH_CHARS {
            return Err(ReviewError::Validation(
                "Paragraph must be between 1 and 220 characters.".to_owned(),
            ));
        }
        if caption.is_empty() || caption.chars().count() > MAX_CAPTION_CHARS {
            return Err(ReviewError::Validation(
                "Caption must be between 1 and 2200 characters.".to_owned(),
            ));
        }

        Ok(Self {
            header_text,
            paragraph_text,
            caption,
        })
    }
}

impl From<GeneratedPost> for GeneratedPostPayload {
    fn from(post: GeneratedPost) -> Self {
        Self {
            id: post.id,
            media_asset_id: post.media_asset_id,
            image_reference: post.image_reference,
            header_text: post.header_text,
            paragraph_text: post.paragraph_text,
            caption: post.caption,
            status: post_status_label(&post.status).to_owned(),
            scheduled_at: post.scheduled_at.map(|value| value.to_rfc3339()),
            updated_at: post.updated_at.to_rfc3339(),
        }
    }
}

fn normalize_text(value: &str) -> String {
    value.split_whitespace().collect::<Vec<&str>>().join(" ")
}

fn post_status_label(status: &PostStatus) -> &'static str {
    match status {
        PostStatus::Draft => "draft",
        PostStatus::PendingReview => "pending-review",
        PostStatus::Approved => "approved",
        PostStatus::Scheduled => "scheduled",
        PostStatus::Published => "published",
        PostStatus::Failed => "failed",
        PostStatus::Rejected => "rejected",
    }
}

impl IntoResponse for ReviewError {
    fn into_response(self) -> Response {
        match self {
            ReviewError::Auth(error) => error.into_response(),
            ReviewError::PostNotFound => (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "error": "post was not found" })),
            )
                .into_response(),
            ReviewError::Validation(message) => (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": message })),
            )
                .into_response(),
            ReviewError::Generation(error) => {
                tracing::error!(%error, "review generation failed");
                (
                    StatusCode::BAD_GATEWAY,
                    Json(serde_json::json!({ "error": "post could not be regenerated" })),
                )
                    .into_response()
            }
            ReviewError::Stock(error) => {
                tracing::error!(%error, "review stock fallback failed");
                (
                    StatusCode::BAD_GATEWAY,
                    Json(serde_json::json!({ "error": "post could not be regenerated" })),
                )
                    .into_response()
            }
            ReviewError::Compositor(error) => {
                tracing::error!(%error, "review compositing failed");
                (
                    StatusCode::BAD_GATEWAY,
                    Json(serde_json::json!({ "error": "post image could not be composited" })),
                )
                    .into_response()
            }
            ReviewError::Storage(error) => {
                tracing::error!(%error, "review media storage failed");
                (
                    StatusCode::BAD_GATEWAY,
                    Json(serde_json::json!({ "error": "post image could not be stored" })),
                )
                    .into_response()
            }
            ReviewError::Database(error) => {
                tracing::error!(%error, "review database operation failed");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": "post review action could not be saved" })),
                )
                    .into_response()
            }
        }
    }
}
