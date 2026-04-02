use backend::{app, config::AppConfig, db};

#[tokio::main]
async fn main() -> Result<(), backend::error::AppError> {
    let config = AppConfig::from_env()?;
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let pool = db::init_pool(&config.database_url).await?;
    db::run_migrations(&pool).await?;
    db::bootstrap_admin_if_missing(&pool, &config).await?;
    db::seed_demo_data(&pool, &config).await?;

    let app = app::build_router(pool, config.clone());
    let addr = format!("{}:{}", config.host, config.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    tracing::info!("backend listening on {}", addr);
    axum::serve(listener, app).await?;
    Ok(())
}
