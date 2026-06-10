use std::sync::Arc;
use std::collections::BTreeMap;
use axum::{
    extract::{State, Path, Query},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Serialize, Deserialize};
use crate::alert::engine::AlertEngine;
use crate::alert::rule::{AlertRule, AggType, CompareOp, Severity};
use crate::api::routes::AppState;

#[derive(Deserialize)]
pub struct CreateRuleRequest {
    pub name: String,
    pub metric: String,
    #[serde(default)]
    pub tags: BTreeMap<String, String>,
    pub aggregation: String,
    pub window_secs: u64,
    pub operator: String,
    pub threshold: f64,
    #[serde(default = "default_trigger_count")]
    pub trigger_count: u32,
    pub severity: String,
    #[serde(default = "default_silence_secs")]
    pub silence_secs: u64,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_trigger_count() -> u32 { 1 }
fn default_silence_secs() -> u64 { 300 }
fn default_true() -> bool { true }

#[derive(Serialize)]
pub struct RuleWithState {
    #[serde(flatten)]
    pub rule: AlertRule,
    pub state: String,
    pub consecutive_count: u32,
    pub current_value: Option<f64>,
    pub last_eval_time: Option<i64>,
    pub last_fire_time: Option<i64>,
}

#[derive(Deserialize)]
pub struct UpdateRuleRequest {
    pub name: Option<String>,
    pub metric: Option<String>,
    pub tags: Option<BTreeMap<String, String>>,
    pub aggregation: Option<String>,
    pub window_secs: Option<u64>,
    pub operator: Option<String>,
    pub threshold: Option<f64>,
    pub trigger_count: Option<u32>,
    pub severity: Option<String>,
    pub silence_secs: Option<u64>,
    pub enabled: Option<bool>,
}

#[derive(Deserialize)]
pub struct EventsQuery {
    pub start_time: Option<i64>,
    pub end_time: Option<i64>,
    pub severity: Option<String>,
    pub rule_name: Option<String>,
    #[serde(default = "default_offset")]
    pub offset: usize,
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_offset() -> usize { 0 }
fn default_limit() -> usize { 50 }

#[derive(Serialize)]
pub struct EventsResponse {
    pub events: Vec<crate::alert::event::AlertEvent>,
    pub total: usize,
}

pub async fn create_rule_handler(
    State(state): State<AppState>,
    Json(req): Json<CreateRuleRequest>,
) -> impl IntoResponse {
    let alert_engine = &state.alert_engine;
    let aggregation = match AggType::from_str(&req.aggregation) {
        Some(a) => a,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "invalid aggregation, must be one of: avg, max, min, sum, count"})),
            ).into_response();
        }
    };

    let operator = match CompareOp::from_str(&req.operator) {
        Some(o) => o,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "invalid operator, must be one of: >, >=, <, <=, ==, !="})),
            ).into_response();
        }
    };

    let severity = match Severity::from_str(&req.severity) {
        Some(s) => s,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "invalid severity, must be one of: critical, warning, info"})),
            ).into_response();
        }
    };

    let rule = AlertRule {
        id: uuid::Uuid::new_v4().to_string(),
        name: req.name,
        metric: req.metric,
        tags: req.tags,
        aggregation,
        window_secs: req.window_secs,
        operator,
        threshold: req.threshold,
        trigger_count: req.trigger_count,
        severity,
        silence_secs: req.silence_secs,
        enabled: req.enabled,
    };

    alert_engine.add_eval_state(&rule.id);
    let created = alert_engine.rule_store().create(rule);
    Json(serde_json::json!({"status": "ok", "rule": created})).into_response()
}

pub async fn list_rules_handler(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let alert_engine = &state.alert_engine;
    let rules = alert_engine.rule_store().list();
    let eval_states = alert_engine.get_eval_states();

    let rules_with_state: Vec<RuleWithState> = rules.into_iter().map(|rule| {
        let state = eval_states.get(&rule.id);
        RuleWithState {
            state: state.map(|s| s.state.as_str().to_string()).unwrap_or_else(|| "inactive".to_string()),
            consecutive_count: state.map(|s| s.consecutive_count).unwrap_or(0),
            current_value: state.and_then(|s| s.current_value),
            last_eval_time: state.and_then(|s| s.last_eval_time),
            last_fire_time: state.and_then(|s| s.last_fire_time),
            rule,
        }
    }).collect();

    Json(rules_with_state).into_response()
}

pub async fn update_rule_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<UpdateRuleRequest>,
) -> impl IntoResponse {
    let alert_engine = &state.alert_engine;
    let existing = match alert_engine.rule_store().get(&id) {
        Some(r) => r,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "rule not found"})),
            ).into_response();
        }
    };

    let aggregation = match req.aggregation {
        Some(ref a) => match AggType::from_str(a) {
            Some(agg) => agg,
            None => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({"error": "invalid aggregation"})),
                ).into_response();
            }
        },
        None => existing.aggregation,
    };

    let operator = match req.operator {
        Some(ref o) => match CompareOp::from_str(o) {
            Some(op) => op,
            None => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({"error": "invalid operator"})),
                ).into_response();
            }
        },
        None => existing.operator,
    };

    let severity = match req.severity {
        Some(ref s) => match Severity::from_str(s) {
            Some(sev) => sev,
            None => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({"error": "invalid severity"})),
                ).into_response();
            }
        },
        None => existing.severity,
    };

    let updated = AlertRule {
        id: existing.id,
        name: req.name.unwrap_or(existing.name),
        metric: req.metric.unwrap_or(existing.metric),
        tags: req.tags.unwrap_or(existing.tags),
        aggregation,
        window_secs: req.window_secs.unwrap_or(existing.window_secs),
        operator,
        threshold: req.threshold.unwrap_or(existing.threshold),
        trigger_count: req.trigger_count.unwrap_or(existing.trigger_count),
        severity,
        silence_secs: req.silence_secs.unwrap_or(existing.silence_secs),
        enabled: req.enabled.unwrap_or(existing.enabled),
    };

    alert_engine.reset_eval_state(&id);
    match alert_engine.rule_store().update(&id, updated) {
        Some(rule) => Json(serde_json::json!({"status": "ok", "rule": rule})).into_response(),
        None => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": "failed to update rule"})),
        ).into_response(),
    }
}

pub async fn delete_rule_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let alert_engine = &state.alert_engine;
    if alert_engine.rule_store().delete(&id) {
        alert_engine.remove_eval_state(&id);
        Json(serde_json::json!({"status": "ok"})).into_response()
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "rule not found"})),
        ).into_response()
    }
}

pub async fn enable_rule_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let alert_engine = &state.alert_engine;
    match alert_engine.rule_store().set_enabled(&id, true) {
        Some(rule) => {
            alert_engine.reset_eval_state(&id);
            Json(serde_json::json!({"status": "ok", "rule": rule})).into_response()
        }
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "rule not found"})),
        ).into_response(),
    }
}

pub async fn disable_rule_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let alert_engine = &state.alert_engine;
    match alert_engine.rule_store().set_enabled(&id, false) {
        Some(rule) => {
            alert_engine.reset_eval_state(&id);
            Json(serde_json::json!({"status": "ok", "rule": rule})).into_response()
        }
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "rule not found"})),
        ).into_response(),
    }
}

pub async fn list_events_handler(
    State(state): State<AppState>,
    Query(query): Query<EventsQuery>,
) -> impl IntoResponse {
    let alert_engine = &state.alert_engine;
    let (events, total) = alert_engine.event_store().query(
        query.start_time,
        query.end_time,
        query.severity.as_deref(),
        query.rule_name.as_deref(),
        query.offset,
        query.limit,
    );
    Json(EventsResponse { events, total }).into_response()
}

pub async fn active_alerts_handler(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let active = state.alert_engine.event_store().active_firing();
    Json(active).into_response()
}

pub async fn ws_alerts_handler(
    ws: axum::extract::ws::WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_ws_alerts(socket, state.alert_engine.clone()))
}

async fn handle_ws_alerts(mut socket: axum::extract::ws::WebSocket, alert_engine: Arc<AlertEngine>) {
    let mut rx = alert_engine.notifier().subscribe();

    loop {
        tokio::select! {
            msg = rx.recv() => {
                match msg {
                    Ok(json_str) => {
                        if socket.send(axum::extract::ws::Message::Text(json_str.into())).await.is_err() {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                        continue;
                    }
                    Err(_) => {
                        break;
                    }
                }
            }
            msg = socket.recv() => {
                match msg {
                    Some(Ok(axum::extract::ws::Message::Close(_))) | None => {
                        break;
                    }
                    _ => {}
                }
            }
        }
    }
}
