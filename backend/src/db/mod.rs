pub mod models;
pub mod repository;

use sqlx::{postgres::PgPoolOptions, PgPool};
use thiserror::Error;

use crate::config::DatabaseConfig;

#[derive(Debug, Error)]
pub enum DatabaseError {
    #[error("database operation failed: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("database migration failed: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),
}

pub async fn connect(database: &DatabaseConfig) -> Result<PgPool, DatabaseError> {
    PgPoolOptions::new()
        .max_connections(5)
        .connect(&database.url)
        .await
        .map_err(DatabaseError::Sqlx)
}

pub async fn run_migrations(pool: &PgPool) -> Result<(), DatabaseError> {
    sqlx::migrate!("../migrations").run(pool).await?;
    Ok(())
}
