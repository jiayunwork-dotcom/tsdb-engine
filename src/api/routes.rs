use std::sync::Arc;
use axum::{
    routing::{get, post, put, delete},
    Router,
};
use tower_http::cors::{CorsLayer, Any};
use crate::engine::TsdbEngine;
use crate::alert::engine::AlertEngine;

#[derive(Clone)]
pub struct AppState {
    pub engine: Arc<TsdbEngine>,
    pub alert_engine: Arc<AlertEngine>,
}

pub fn create_router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/api/write", post(super::handlers::write_handler))
        .route("/api/delete", post(super::handlers::delete_handler))
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
        .route("/api/alerts/rules", post(crate::alert::handlers::create_rule_handler))
        .route("/api/alerts/rules", get(crate::alert::handlers::list_rules_handler))
        .route("/api/alerts/rules/:id", put(crate::alert::handlers::update_rule_handler))
        .route("/api/alerts/rules/:id", delete(crate::alert::handlers::delete_rule_handler))
        .route("/api/alerts/rules/:id/enable", post(crate::alert::handlers::enable_rule_handler))
        .route("/api/alerts/rules/:id/disable", post(crate::alert::handlers::disable_rule_handler))
        .route("/api/alerts/events", get(crate::alert::handlers::list_events_handler))
        .route("/api/alerts/events/:id/ack", post(crate::alert::handlers::acknowledge_event_handler))
        .route("/api/alerts/active", get(crate::alert::handlers::active_alerts_handler))
        .route("/api/alerts/templates", get(crate::alert::handlers::list_templates_handler))
        .route("/ws/alerts", get(crate::alert::handlers::ws_alerts_handler))
        .with_state(state)
        .layer(cors)
}
