mod api;
mod checkpoint_db;
mod checkpoints;
mod checker;
mod db;
mod feishu;
mod history;
mod models;
mod probe;
mod pushplus;

use axum::Router;
use std::net::SocketAddr;
use std::path::PathBuf;
use tower_http::cors::CorsLayer;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let db_path = std::env::var("DATABASE_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("data/health.db"));

    let pool = db::init_pool(&db_path).await?;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()?;

    let history_retention = history::HistoryRetention::from_env();
    history::spawn_cleanup_job(pool.clone(), history_retention);

    checker::spawn_scheduler(pool.clone(), client.clone());

    let assets = PathBuf::from(
        std::env::var("ASSETS_DIR").unwrap_or_else(|_| "assets".into()),
    );
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

    let host = std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0".into());
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);

    let addr: SocketAddr = format!("{}:{}", host, port).parse()?;
    tracing::info!("listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
