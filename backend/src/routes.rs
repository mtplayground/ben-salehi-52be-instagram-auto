use axum::{routing::get, Json, Router};
use serde::Serialize;

use crate::{
    auth, compositor, error::AppError, generation, instagram, queue, schedule, settings, stock,
    storage, AppState,
};

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    service: &'static str,
}

pub fn api_router() -> Router<AppState> {
    Router::new()
        .nest("/auth", auth::routes())
        .nest("/compositor", compositor::routes())
        .nest("/generation", generation::routes())
        .nest("/instagram", instagram::routes())
        .nest("/queue", queue::routes())
        .nest("/schedule", schedule::routes())
        .nest("/settings", settings::routes())
        .nest("/stock", stock::routes())
        .nest("/media", storage::routes())
        .route("/health", get(health))
        .fallback(api_not_found)
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        service: "instagram-auto-backend",
    })
}

async fn api_not_found() -> Result<(), AppError> {
    Err(AppError::NotFound)
}
