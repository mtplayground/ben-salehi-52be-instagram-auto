use axum::{routing::get, Json, Router};
use serde::Serialize;

use crate::error::AppError;

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    service: &'static str,
}

pub fn api_router() -> Router {
    Router::new()
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
