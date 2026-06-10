mod config;
mod model;
mod engine;
mod api;
mod alert;

use std::sync::Arc;
use std::path::PathBuf;
use config::EngineConfig;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let config = EngineConfig::default();
    let tsdb = match engine::TsdbEngine::new(config.clone()) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("Failed to initialize TSDB engine: {}", e);
            std::process::exit(1);
        }
    };

    let engine = Arc::new(tsdb);
    engine.run_background_tasks();

    let data_dir = PathBuf::from(&config.data_dir);
    let alert_engine = Arc::new(alert::engine::AlertEngine::new(engine.clone(), &data_dir));
    alert_engine.clone().start_background_eval();

    let app = api::routes::create_router(api::routes::AppState {
        engine: engine.clone(),
        alert_engine: alert_engine.clone(),
    });

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], 8080));
    tracing::info!("TSDB Engine listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
