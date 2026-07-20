pub mod config;
pub mod db;
pub mod error;
pub mod routes;

use std::path::PathBuf;

use axum::Router;
use tower_http::{
    services::{ServeDir, ServeFile},
    trace::TraceLayer,
};

pub fn build_app() -> Router {
    let frontend_dist = frontend_dist_path();
    let index_file = frontend_dist.join("index.html");

    Router::new()
        .nest("/api", routes::api_router())
        .fallback_service(ServeDir::new(frontend_dist).not_found_service(ServeFile::new(index_file)))
        .layer(TraceLayer::new_for_http())
}

fn frontend_dist_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../frontend/dist")
}
