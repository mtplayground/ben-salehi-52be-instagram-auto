use instagram_auto_backend::{
    config::AppConfig,
    db::{connect, run_migrations},
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[tokio::main]
async fn main() {
    init_tracing();

    if let Err(error) = migrate().await {
        tracing::error!(%error, ?error, "database migration failed");
        std::process::exit(1);
    }
}

async fn migrate() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config = AppConfig::from_env()?;
    let pool = connect(&config.database).await?;
    run_migrations(&pool).await?;
    tracing::info!("database migrations applied");
    Ok(())
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer())
        .init();
}
