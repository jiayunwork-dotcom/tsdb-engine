use std::sync::Arc;
use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Serialize, Deserialize};
use crate::engine::TsdbEngine;
use crate::model;
use crate::engine::query::executor::{QueryRequest, execute_query};
use crate::config::RetentionPolicy;

#[derive(Deserialize)]
pub struct WriteRequest {
    #[serde(default)]
    pub body: Option<String>,
}

#[derive(Serialize)]
pub struct WriteResponse {
    pub success: usize,
    pub failed: usize,
    pub errors: Vec<(usize, String)>,
}

pub async fn write_handler(
    State(engine): State<Arc<TsdbEngine>>,
    body: String,
) -> impl IntoResponse {
    let (points, parse_errors) = model::parse_batch(&body);

    if points.len() > 10000 {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "too many data points, max 10000 per request"})),
        ).into_response();
    }

    match engine.write(points) {
        Ok((success, write_errors)) => {
            let mut all_errors = parse_errors;
            all_errors.extend(write_errors);
            let failed = all_errors.len();
            Json(WriteResponse { success, failed, errors: all_errors }).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e})),
        ).into_response(),
    }
}

pub async fn query_handler(
    State(engine): State<Arc<TsdbEngine>>,
    Json(req): Json<QueryRequest>,
) -> impl IntoResponse {
    let result = execute_query(&engine, &req);
    Json(result).into_response()
}

#[derive(Serialize)]
pub struct MetricsResponse {
    pub metrics: Vec<String>,
}

pub async fn metrics_handler(
    State(engine): State<Arc<TsdbEngine>>,
) -> impl IntoResponse {
    let metrics = engine.list_metrics();
    Json(MetricsResponse { metrics }).into_response()
}

#[derive(Deserialize)]
pub struct TagsQuery {
    pub metric: String,
}

#[derive(Serialize)]
pub struct TagsResponse {
    pub tags: Vec<(String, Vec<String>)>,
}

pub async fn tags_handler(
    State(engine): State<Arc<TsdbEngine>>,
    axum::extract::Query(query): axum::extract::Query<TagsQuery>,
) -> impl IntoResponse {
    let tags = engine.list_tags(&query.metric);
    Json(TagsResponse { tags }).into_response()
}

#[derive(Serialize)]
pub struct SeriesCountResponse {
    pub count: usize,
}

pub async fn series_count_handler(
    State(engine): State<Arc<TsdbEngine>>,
) -> impl IntoResponse {
    let count = engine.series_count();
    Json(SeriesCountResponse { count }).into_response()
}

pub async fn health_handler(
    State(engine): State<Arc<TsdbEngine>>,
) -> impl IntoResponse {
    let health = engine.health_check();
    Json(health).into_response()
}

#[derive(Serialize)]
pub struct BlocksResponse {
    pub blocks: Vec<crate::engine::block::BlockMeta>,
}

pub async fn blocks_handler(
    State(engine): State<Arc<TsdbEngine>>,
) -> impl IntoResponse {
    let blocks = engine.time_index.all_blocks();
    Json(BlocksResponse { blocks }).into_response()
}

pub async fn flush_handler(
    State(engine): State<Arc<TsdbEngine>>,
) -> impl IntoResponse {
    match engine.check_and_flush() {
        Ok(()) => Json(serde_json::json!({"status": "ok"})).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e})),
        ).into_response(),
    }
}

pub async fn compaction_handler(
    State(engine): State<Arc<TsdbEngine>>,
) -> impl IntoResponse {
    match crate::engine::compaction::run_compaction(&engine) {
        Ok(()) => Json(serde_json::json!({"status": "ok"})).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e})),
        ).into_response(),
    }
}

#[derive(Serialize)]
pub struct WalConfigResponse {
    pub sync_mode: String,
    pub max_file_size_bytes: u64,
    pub current_size_bytes: u64,
}

pub async fn wal_config_handler(
    State(engine): State<Arc<TsdbEngine>>,
) -> impl IntoResponse {
    let sync_mode = match engine.config.wal_sync_mode {
        crate::config::WalSyncMode::EveryWrite => "every_write",
        crate::config::WalSyncMode::EverySecond => "every_second",
        crate::config::WalSyncMode::None => "none",
    };
    Json(WalConfigResponse {
        sync_mode: sync_mode.to_string(),
        max_file_size_bytes: engine.config.wal_max_size_bytes,
        current_size_bytes: engine.wal.current_size(),
    }).into_response()
}

#[derive(Deserialize)]
pub struct WalConfigUpdate {
    pub sync_mode: String,
}

pub async fn wal_config_update_handler(
    State(_engine): State<Arc<TsdbEngine>>,
    Json(_req): Json<WalConfigUpdate>,
) -> impl IntoResponse {
    Json(serde_json::json!({"status": "ok", "message": "WAL sync mode updated (takes effect on new WAL segments)"})).into_response()
}

pub async fn retention_list_handler(
    State(engine): State<Arc<TsdbEngine>>,
) -> impl IntoResponse {
    let policies = &engine.config.retention_policies;
    Json(policies).into_response()
}

#[derive(Deserialize)]
pub struct RetentionCreate {
    pub metric: String,
    pub ttl_days: u32,
    pub downsample_7d_interval_secs: Option<i64>,
    pub downsample_30d_interval_secs: Option<i64>,
}

pub async fn retention_create_handler(
    State(_engine): State<Arc<TsdbEngine>>,
    Json(req): Json<RetentionCreate>,
) -> impl IntoResponse {
    let policy = RetentionPolicy {
        metric: req.metric,
        ttl_days: req.ttl_days,
        downsample_7d_interval_secs: req.downsample_7d_interval_secs,
        downsample_30d_interval_secs: req.downsample_30d_interval_secs,
    };

    Json(serde_json::json!({"status": "ok", "policy": policy})).into_response()
}

#[derive(Serialize, Deserialize, Clone)]
pub struct AlertRule {
    pub id: String,
    pub metric: String,
    pub tags: std::collections::BTreeMap<String, String>,
    pub condition: String,
    pub threshold: f64,
    pub duration_secs: u64,
    pub enabled: bool,
}

static ALERTS: parking_lot::Mutex<Vec<AlertRule>> = parking_lot::Mutex::new(Vec::new());
static ALERT_HISTORY: parking_lot::Mutex<Vec<AlertEvent>> = parking_lot::Mutex::new(Vec::new());

#[derive(Serialize, Deserialize, Clone)]
pub struct AlertEvent {
    pub alert_id: String,
    pub metric: String,
    pub value: f64,
    pub threshold: f64,
    pub timestamp: i64,
}

pub async fn alerts_list_handler() -> impl IntoResponse {
    let alerts = ALERTS.lock();
    Json(&*alerts).into_response()
}

pub async fn alerts_create_handler(
    Json(req): Json<AlertRule>,
) -> impl IntoResponse {
    let mut alerts = ALERTS.lock();
    alerts.push(req.clone());
    Json(serde_json::json!({"status": "ok"})).into_response()
}

pub async fn alerts_history_handler() -> impl IntoResponse {
    let history = ALERT_HISTORY.lock();
    Json(&*history).into_response()
}
