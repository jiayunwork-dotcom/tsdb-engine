use crate::engine::query::aggregation::AggFunc;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GroupByTime {
    pub interval_secs: i64,
}

impl GroupByTime {
    pub fn from_str(s: &str) -> Option<Self> {
        let interval_secs = match s.to_lowercase().as_str() {
            "10s" => 10,
            "1m" => 60,
            "5m" => 300,
            "1h" => 3600,
            "1d" => 86400,
            _ => {
                let s = s.trim_end_matches('s');
                s.parse::<i64>().ok()?
            }
        };
        Some(Self { interval_secs })
    }

    pub fn interval_ns(&self) -> i64 {
        self.interval_secs * 1_000_000_000
    }
}
