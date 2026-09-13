mod api;
mod checkpoint_db;
mod checkpoints;
mod checker;
mod config;
mod db;
mod feishu;
mod history;
mod models;
mod probe;
mod pushplus;

use axum::Router;
use std::net::SocketAddr;
use tower_http::cors::CorsLayer;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config_path = config::default_config_path();
    let cfg = config::AppConfig::load(&config_path)?;
    config::init(cfg);
    let cfg = config::get();

    let log_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .or_else(|_| tracing_subscriber::EnvFilter::try_new(&cfg.log.level))
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    tracing_subscriber::fmt().with_env_filter(log_filter).init();

    tracing::info!("loaded configuration from {:?}", config_path);

    let pool = db::init_pool(&cfg.database_path()).await?;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(cfg.http_client.timeout_secs))
        .build()?;

    let history_retention = history::HistoryRetention::from_config(&cfg.history);
    history::spawn_cleanup_job(pool.clone(), history_retention);

    checker::spawn_scheduler(pool.clone(), client.clone(), cfg.scheduler.tick_secs);

    let assets = cfg.assets_dir();
    let index = assets.join("index.html");

    let spa = ServeDir::new(&assets).not_found_service(ServeFile::new(index));

    let state = api::AppState {
        pool,
        http: client,
    };

    let app = Router::new()
        .merge(api::router())
        .fallback_service(spa)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr: SocketAddr = format!("{}:{}", cfg.server.host, cfg.server.port).parse()?;
    tracing::info!("listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
