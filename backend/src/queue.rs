use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde::Serialize;
use uuid::Uuid;

use crate::{
    auth::AuthError,
    db::{
        models::PostStatus,
        repository::QueueCalendarPost,
    },
    AppState,
};

#[derive(Debug, Serialize)]
struct QueueResponse {
    posts: Vec<QueuePostPayload>,
}

#[derive(Debug, Serialize)]
struct QueuePostPayload {
    post_id: Uuid,
    queue_id: Option<Uuid>,
    media_asset_id: Option<Uuid>,
    image_url: Option<String>,
    image_source: Option<String>,
    image_license: Option<String>,
    image_width: Option<i32>,
    image_height: Option<i32>,
    image_mime_type: Option<String>,
    image_reference: Option<String>,
    header_text: String,
    paragraph_text: String,
    caption: String,
    status: String,
    scheduled_at: Option<String>,
    scheduled_for: Option<String>,
    published_at: Option<String>,
    failed_at: Option<String>,
    failure_message: Option<String>,
    publish_retry_count: i32,
    last_publish_attempt_at: Option<String>,
    next_retry_at: Option<String>,
    queue_position: Option<i32>,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, thiserror::Error)]
enum QueueError {
    #[error("{0}")]
    Auth(#[from] AuthError),
    #[error("database operation failed: {0}")]
    Database(#[from] sqlx::Error),
}

pub fn routes() -> Router<AppState> {
    Router::new().route("/", get(list_queue))
}

async fn list_queue(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<QueueResponse>, QueueError> {
    let creator = state.auth.current_creator(&headers).await?;
    let posts = state
        .repository
        .list_calendar_posts_for_creator(creator.creator.id)
        .await?
        .into_iter()
        .map(QueuePostPayload::from)
        .collect();

    Ok(Json(QueueResponse { posts }))
}

impl From<QueueCalendarPost> for QueuePostPayload {
    fn from(post: QueueCalendarPost) -> Self {
        Self {
            post_id: post.post_id,
            queue_id: post.queue_id,
            media_asset_id: post.media_asset_id,
            image_url: post.image_url,
            image_source: post.image_source,
            image_license: post.image_license,
            image_width: post.image_width,
            image_height: post.image_height,
            image_mime_type: post.image_mime_type,
            image_reference: post.image_reference,
            header_text: post.header_text,
            paragraph_text: post.paragraph_text,
            caption: post.caption,
            status: post_status_label(&post.status).to_owned(),
            scheduled_at: post.scheduled_at.map(|value| value.to_rfc3339()),
            scheduled_for: post.scheduled_for.map(|value| value.to_rfc3339()),
            published_at: post.published_at.map(|value| value.to_rfc3339()),
            failed_at: post.failed_at.map(|value| value.to_rfc3339()),
            failure_message: post.failure_message,
            publish_retry_count: post.publish_retry_count,
            last_publish_attempt_at: post.last_publish_attempt_at.map(|value| value.to_rfc3339()),
            next_retry_at: post.next_retry_at.map(|value| value.to_rfc3339()),
            queue_position: post.queue_position,
            created_at: post.created_at.to_rfc3339(),
            updated_at: post.updated_at.to_rfc3339(),
        }
    }
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

impl IntoResponse for QueueError {
    fn into_response(self) -> Response {
        match self {
            QueueError::Auth(error) => error.into_response(),
            QueueError::Database(error) => {
                tracing::error!(%error, "queue database operation failed");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": "queue could not be loaded" })),
                )
                    .into_response()
            }
        }
    }
}
