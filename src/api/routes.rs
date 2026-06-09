use std::sync::Arc;
use axum::{
    routing::{get, post},
    Router,
};
use tower_http::cors::{CorsLayer, Any};
use crate::engine::TsdbEngine;

pub fn create_router(engine: Arc<TsdbEngine>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/api/write", post(super::handlers::write_handler))
        .route("/api/query", post(super::handlers::query_handler))
        .route("/api/metrics", get(super::handlers::metrics_handler))
        .route("/api/tags", get(super::handlers::tags_handler))
        .route("/api/series_count", get(super::handlers::series_count_handler))
        .route("/api/health", get(super::handlers::health_handler))
        .route("/api/blocks", get(super::handlers::blocks_handler))
        .route("/api/flush", post(super::handlers::flush_handler))
        .route("/api/compaction", post(super::handlers::compaction_handler))
        .route("/api/config/wal", get(super::handlers::wal_config_handler))
        .route("/api/config/wal", post(super::handlers::wal_config_update_handler))
        .route("/api/config/retention", get(super::handlers::retention_list_handler))
        .route("/api/config/retention", post(super::handlers::retention_create_handler))
        .route("/api/alerts", get(super::handlers::alerts_list_handler))
        .route("/api/alerts", post(super::handlers::alerts_create_handler))
        .route("/api/alerts/history", get(super::handlers::alerts_history_handler))
        .with_state(engine)
        .layer(cors)
}
