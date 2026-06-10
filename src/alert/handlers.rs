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
use crate::alert::rule::{AlertRule, AggType, CompareOp, Severity, SubCondition, LogicOperator};
use crate::alert::event::AlertEvent;
use crate::api::routes::AppState;

#[derive(Deserialize)]
pub struct SubConditionRequest {
    pub metric: String,
    #[serde(default)]
    pub tags: BTreeMap<String, String>,
    pub aggregation: String,
    pub window_secs: u64,
    pub operator: String,
    pub threshold: f64,
}

impl SubConditionRequest {
    fn to_sub_condition(&self) -> Result<SubCondition, (StatusCode, Json<serde_json::Value>)> {
        let aggregation = match AggType::from_str(&self.aggregation) {
            Some(a) => a,
            None => {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({"error": "invalid aggregation in sub-condition, must be one of: avg, max, min, sum, count"})),
                ));
            }
        };
        let operator = match CompareOp::from_str(&self.operator) {
            Some(o) => o,
            None => {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({"error": "invalid operator in sub-condition, must be one of: >, >=, <, <=, ==, !="})),
                ));
            }
        };
        Ok(SubCondition {
            metric: self.metric.clone(),
            tags: self.tags.clone(),
            aggregation,
            window_secs: self.window_secs,
            operator,
            threshold: self.threshold,
        })
    }
}

#[derive(Deserialize)]
pub struct CreateRuleRequest {
    pub name: String,
    #[serde(default)]
    pub conditions: Vec<SubConditionRequest>,
    #[serde(default = "default_logic")]
    pub logic: String,
    #[serde(default)]
    pub metric: Option<String>,
    #[serde(default)]
    pub tags: Option<BTreeMap<String, String>>,
    #[serde(default)]
    pub aggregation: Option<String>,
    #[serde(default)]
    pub window_secs: Option<u64>,
    #[serde(default)]
    pub operator: Option<String>,
    #[serde(default)]
    pub threshold: Option<f64>,
    #[serde(default = "default_trigger_count")]
    pub trigger_count: u32,
    #[serde(default = "default_severity")]
    pub severity: String,
    #[serde(default = "default_silence_secs")]
    pub silence_secs: u64,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_trigger_count() -> u32 { 1 }
fn default_silence_secs() -> u64 { 300 }
fn default_true() -> bool { true }
fn default_logic() -> String { "and".to_string() }
fn default_severity() -> String { "warning".to_string() }

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
    #[serde(default)]
    pub conditions: Option<Vec<SubConditionRequest>>,
    pub logic: Option<String>,
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
    pub cursor: Option<i64>,
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_limit() -> usize { 50 }

#[derive(Serialize)]
pub struct EventsResponse {
    pub events: Vec<AlertEvent>,
    pub next_cursor: Option<i64>,
}

#[derive(Deserialize)]
pub struct AckRequest {
    pub operator: String,
}

#[derive(Serialize)]
pub struct AlertTemplate {
    pub id: String,
    pub name: String,
    pub description: String,
    pub conditions: Vec<TemplateSubCondition>,
    pub logic: String,
    pub severity: String,
    pub trigger_count: u32,
    pub silence_secs: u64,
}

#[derive(Serialize)]
pub struct TemplateSubCondition {
    pub metric: String,
    pub aggregation: String,
    pub window_secs: u64,
    pub operator: String,
    pub threshold: f64,
}

fn get_templates() -> Vec<AlertTemplate> {
    vec![
        AlertTemplate {
            id: "cpu-high-load".to_string(),
            name: "CPU High Load".to_string(),
            description: "Alerts when CPU usage exceeds threshold over a time window".to_string(),
            conditions: vec![
                TemplateSubCondition {
                    metric: "cpu_usage".to_string(),
                    aggregation: "avg".to_string(),
                    window_secs: 300,
                    operator: ">".to_string(),
                    threshold: 80.0,
                },
            ],
            logic: "and".to_string(),
            severity: "critical".to_string(),
            trigger_count: 2,
            silence_secs: 600,
        },
        AlertTemplate {
            id: "memory-low".to_string(),
            name: "Memory Insufficient".to_string(),
            description: "Alerts when available memory drops below threshold".to_string(),
            conditions: vec![
                TemplateSubCondition {
                    metric: "memory_available".to_string(),
                    aggregation: "avg".to_string(),
                    window_secs: 180,
                    operator: "<".to_string(),
                    threshold: 10.0,
                },
            ],
            logic: "and".to_string(),
            severity: "warning".to_string(),
            trigger_count: 1,
            silence_secs: 300,
        },
        AlertTemplate {
            id: "disk-io-high".to_string(),
            name: "Disk IO Too High".to_string(),
            description: "Alerts when disk IO utilization exceeds threshold".to_string(),
            conditions: vec![
                TemplateSubCondition {
                    metric: "disk_io_util".to_string(),
                    aggregation: "max".to_string(),
                    window_secs: 120,
                    operator: ">".to_string(),
                    threshold: 90.0,
                },
            ],
            logic: "and".to_string(),
            severity: "critical".to_string(),
            trigger_count: 1,
            silence_secs: 300,
        },
    ]
}

pub async fn create_rule_handler(
    State(state): State<AppState>,
    Json(req): Json<CreateRuleRequest>,
) -> impl IntoResponse {
    let alert_engine = &state.alert_engine;

    if req.conditions.is_empty() && req.metric.is_none() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "must provide either conditions array or top-level metric field"})),
        ).into_response();
    }

    let conditions: Vec<SubCondition> = match req.conditions.iter().map(|c| c.to_sub_condition()).collect::<Result<Vec<_>, _>>() {
        Ok(cs) => {
            if cs.len() > 5 {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({"error": "maximum 5 sub-conditions allowed"})),
                ).into_response();
            }
            cs
        },
        Err(e) => return e.into_response(),
    };

    let logic = match LogicOperator::from_str(&req.logic) {
        Some(l) => l,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "invalid logic, must be one of: and, or"})),
            ).into_response();
        }
    };

    let first_cond = conditions.first();

    let metric = match req.metric {
        Some(m) => m,
        None => match first_cond {
            Some(c) => c.metric.clone(),
            None => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({"error": "metric is required when no conditions provided"})),
                ).into_response();
            }
        },
    };

    let tags = req.tags.unwrap_or_else(|| {
        first_cond.map(|c| c.tags.clone()).unwrap_or_default()
    });

    let aggregation_str = req.aggregation.unwrap_or_else(|| {
        first_cond.map(|c| c.aggregation.as_str().to_string()).unwrap_or_else(|| "avg".to_string())
    });
    let aggregation = match AggType::from_str(&aggregation_str) {
        Some(a) => a,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "invalid aggregation, must be one of: avg, max, min, sum, count"})),
            ).into_response();
        }
    };

    let operator_str = req.operator.unwrap_or_else(|| {
        first_cond.map(|c| c.operator.as_str().to_string()).unwrap_or_else(|| ">".to_string())
    });
    let operator = match CompareOp::from_str(&operator_str) {
        Some(o) => o,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "invalid operator, must be one of: >, >=, <, <=, ==, !="})),
            ).into_response();
        }
    };

    let window_secs = req.window_secs.unwrap_or_else(|| {
        first_cond.map(|c| c.window_secs).unwrap_or(300)
    });

    let threshold = req.threshold.unwrap_or_else(|| {
        first_cond.map(|c| c.threshold).unwrap_or(0.0)
    });

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
        conditions,
        logic,
        metric,
        tags,
        aggregation,
        window_secs,
        operator,
        threshold,
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

    let conditions = match req.conditions {
        Some(cs) => {
            let parsed: Vec<SubCondition> = match cs.iter().map(|c| c.to_sub_condition()).collect::<Result<Vec<_>, _>>() {
                Ok(ps) => {
                    if ps.len() > 5 {
                        return (
                            StatusCode::BAD_REQUEST,
                            Json(serde_json::json!({"error": "maximum 5 sub-conditions allowed"})),
                        ).into_response();
                    }
                    ps
                },
                Err(e) => return e.into_response(),
            };
            parsed
        },
        None => existing.conditions,
    };

    let logic = match req.logic {
        Some(ref l) => match LogicOperator::from_str(l) {
            Some(op) => op,
            None => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({"error": "invalid logic"})),
                ).into_response();
            }
        },
        None => existing.logic,
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
        conditions,
        logic,
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
    let (events, next_cursor) = alert_engine.event_store().query(
        query.start_time,
        query.end_time,
        query.severity.as_deref(),
        query.rule_name.as_deref(),
        query.cursor,
        query.limit,
    );
    Json(EventsResponse { events, next_cursor }).into_response()
}

pub async fn acknowledge_event_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<AckRequest>,
) -> impl IntoResponse {
    let alert_engine = &state.alert_engine;
    match alert_engine.acknowledge_event(&id, &req.operator) {
        Some(event) => Json(serde_json::json!({"status": "ok", "event": event})).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "event not found or already acknowledged"})),
        ).into_response(),
    }
}

pub async fn active_alerts_handler(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let active = state.alert_engine.event_store().active_firing();
    Json(active).into_response()
}

pub async fn list_templates_handler() -> impl IntoResponse {
    Json(get_templates()).into_response()
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
