pub mod auth;
pub mod config;
pub mod credentials;
pub mod db;
pub mod error;
pub mod generation;
pub mod instagram;
pub mod routes;
pub mod settings;
pub mod stock;

use std::{path::PathBuf, sync::Arc};

use axum::Router;
use auth::{AuthError, AuthService};
use config::AppConfig;
use db::repository::CoreRepository;
use tower_http::{
    services::{ServeDir, ServeFile},
    trace::TraceLayer,
};

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<AppConfig>,
    pub repository: CoreRepository,
    pub auth: AuthService,
}

impl AppState {
    pub fn new(config: AppConfig, repository: CoreRepository) -> Result<Self, AuthError> {
        let config = Arc::new(config);
        let auth = AuthService::new(config.clone(), repository.clone())?;

        Ok(Self {
            config,
            repository,
            auth,
        })
    }
}

pub fn build_app(state: AppState) -> Router {
    let frontend_dist = frontend_dist_path();
    let index_file = frontend_dist.join("index.html");

    Router::new()
        .nest("/api", routes::api_router())
        .fallback_service(
            ServeDir::new(frontend_dist).not_found_service(ServeFile::new(index_file)),
        )
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

fn frontend_dist_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../frontend/dist")
}
