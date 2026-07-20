pub mod models;
pub mod repository;

use std::env;

use sqlx::{postgres::PgPoolOptions, PgPool};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DatabaseError {
    #[error("DATABASE_URL must be set")]
    MissingDatabaseUrl,
    #[error("database operation failed: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("database migration failed: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),
}

pub async fn connect_from_env() -> Result<PgPool, DatabaseError> {
    let database_url = env::var("DATABASE_URL").map_err(|_| DatabaseError::MissingDatabaseUrl)?;
    connect(&database_url).await
}

pub async fn connect(database_url: &str) -> Result<PgPool, DatabaseError> {
    PgPoolOptions::new()
        .max_connections(5)
        .connect(database_url)
        .await
        .map_err(DatabaseError::Sqlx)
}

pub async fn run_migrations(pool: &PgPool) -> Result<(), DatabaseError> {
    sqlx::migrate!("../migrations").run(pool).await?;
    Ok(())
}
