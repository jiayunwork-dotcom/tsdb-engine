use std::collections::BTreeMap;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubCondition {
    pub metric: String,
    #[serde(default)]
    pub tags: BTreeMap<String, String>,
    pub aggregation: AggType,
    pub window_secs: u64,
    pub operator: CompareOp,
    pub threshold: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogicOperator {
    And,
    Or,
}

impl LogicOperator {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "and" => Some(LogicOperator::And),
            "or" => Some(LogicOperator::Or),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            LogicOperator::And => "and",
            LogicOperator::Or => "or",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertRule {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub conditions: Vec<SubCondition>,
    #[serde(default = "default_logic_operator")]
    pub logic: LogicOperator,
    pub metric: String,
    #[serde(default)]
    pub tags: BTreeMap<String, String>,
    pub aggregation: AggType,
    pub window_secs: u64,
    pub operator: CompareOp,
    pub threshold: f64,
    pub trigger_count: u32,
    pub severity: Severity,
    pub silence_secs: u64,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

fn default_logic_operator() -> LogicOperator {
    LogicOperator::And
}

impl AlertRule {
    pub fn effective_conditions(&self) -> Vec<SubCondition> {
        if !self.conditions.is_empty() {
            self.conditions.clone()
        } else {
            vec![SubCondition {
                metric: self.metric.clone(),
                tags: self.tags.clone(),
                aggregation: self.aggregation,
                window_secs: self.window_secs,
                operator: self.operator,
                threshold: self.threshold,
            }]
        }
    }

    pub fn evaluate_logic(&self, results: &[bool]) -> bool {
        if results.is_empty() {
            return false;
        }
        match self.logic {
            LogicOperator::And => results.iter().all(|&r| r),
            LogicOperator::Or => results.iter().any(|&r| r),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AggType {
    Avg,
    Max,
    Min,
    Sum,
    Count,
}

impl AggType {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "avg" => Some(AggType::Avg),
            "max" => Some(AggType::Max),
            "min" => Some(AggType::Min),
            "sum" => Some(AggType::Sum),
            "count" => Some(AggType::Count),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            AggType::Avg => "avg",
            AggType::Max => "max",
            AggType::Min => "min",
            AggType::Sum => "sum",
            AggType::Count => "count",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CompareOp {
    Gt,
    Gte,
    Lt,
    Lte,
    Eq,
    Neq,
}

impl CompareOp {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            ">" => Some(CompareOp::Gt),
            ">=" => Some(CompareOp::Gte),
            "<" => Some(CompareOp::Lt),
            "<=" => Some(CompareOp::Lte),
            "==" => Some(CompareOp::Eq),
            "!=" => Some(CompareOp::Neq),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            CompareOp::Gt => ">",
            CompareOp::Gte => ">=",
            CompareOp::Lt => "<",
            CompareOp::Lte => "<=",
            CompareOp::Eq => "==",
            CompareOp::Neq => "!=",
        }
    }

    pub fn compare(&self, value: f64, threshold: f64) -> bool {
        match self {
            CompareOp::Gt => value > threshold,
            CompareOp::Gte => value >= threshold,
            CompareOp::Lt => value < threshold,
            CompareOp::Lte => value <= threshold,
            CompareOp::Eq => (value - threshold).abs() < f64::EPSILON,
            CompareOp::Neq => (value - threshold).abs() >= f64::EPSILON,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Critical,
    Warning,
    Info,
}

impl Severity {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "critical" => Some(Severity::Critical),
            "warning" => Some(Severity::Warning),
            "info" => Some(Severity::Info),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Severity::Critical => "critical",
            Severity::Warning => "warning",
            Severity::Info => "info",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RuleState {
    Inactive,
    Pending,
    Firing,
    Acknowledged,
    Resolved,
}

impl RuleState {
    pub fn as_str(&self) -> &str {
        match self {
            RuleState::Inactive => "inactive",
            RuleState::Pending => "pending",
            RuleState::Firing => "firing",
            RuleState::Acknowledged => "acknowledged",
            RuleState::Resolved => "resolved",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleEvalState {
    pub rule_id: String,
    pub state: RuleState,
    pub consecutive_count: u32,
    pub last_fire_time: Option<i64>,
    pub last_eval_time: Option<i64>,
    pub current_value: Option<f64>,
}

impl RuleEvalState {
    pub fn new(rule_id: String) -> Self {
        Self {
            rule_id,
            state: RuleState::Inactive,
            consecutive_count: 0,
            last_fire_time: None,
            last_eval_time: None,
            current_value: None,
        }
    }

    pub fn reset(&mut self) {
        self.state = RuleState::Inactive;
        self.consecutive_count = 0;
        self.last_fire_time = None;
        self.last_eval_time = None;
        self.current_value = None;
    }
}

pub struct RuleStore {
    path: std::path::PathBuf,
    rules: parking_lot::RwLock<Vec<AlertRule>>,
}

impl RuleStore {
    pub fn new(data_dir: &std::path::Path) -> Self {
        let dir = data_dir.join("alerts");
        std::fs::create_dir_all(&dir).ok();
        let path = dir.join("rules.json");

        let rules = if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
                Err(_) => Vec::new(),
            }
        } else {
            Vec::new()
        };

        Self {
            path,
            rules: parking_lot::RwLock::new(rules),
        }
    }

    fn persist(&self) {
        let rules = self.rules.read();
        if let Ok(json) = serde_json::to_string_pretty(&*rules) {
            let _ = std::fs::write(&self.path, json);
        }
    }

    pub fn create(&self, rule: AlertRule) -> AlertRule {
        let mut rules = self.rules.write();
        rules.push(rule.clone());
        drop(rules);
        self.persist();
        rule
    }

    pub fn list(&self) -> Vec<AlertRule> {
        self.rules.read().clone()
    }

    pub fn get(&self, id: &str) -> Option<AlertRule> {
        self.rules.read().iter().find(|r| r.id == id).cloned()
    }

    pub fn update(&self, id: &str, updated: AlertRule) -> Option<AlertRule> {
        let mut rules = self.rules.write();
        if let Some(pos) = rules.iter().position(|r| r.id == id) {
            rules[pos] = updated.clone();
            drop(rules);
            self.persist();
            Some(updated)
        } else {
            None
        }
    }

    pub fn delete(&self, id: &str) -> bool {
        let mut rules = self.rules.write();
        let before = rules.len();
        rules.retain(|r| r.id != id);
        let deleted = rules.len() < before;
        drop(rules);
        if deleted {
            self.persist();
        }
        deleted
    }

    pub fn set_enabled(&self, id: &str, enabled: bool) -> Option<AlertRule> {
        let mut rules = self.rules.write();
        if let Some(rule) = rules.iter_mut().find(|r| r.id == id) {
            rule.enabled = enabled;
            let updated = rule.clone();
            drop(rules);
            self.persist();
            Some(updated)
        } else {
            None
        }
    }

    pub fn enabled_rules(&self) -> Vec<AlertRule> {
        self.rules.read().iter().filter(|r| r.enabled).cloned().collect()
    }
}
