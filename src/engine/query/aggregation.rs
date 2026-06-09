use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum AggFunc {
    Avg,
    Sum,
    Max,
    Min,
    Count,
    First,
    Last,
    Percentile(f64),
    Rate,
    Derivative,
}

impl AggFunc {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "avg" => Some(AggFunc::Avg),
            "sum" => Some(AggFunc::Sum),
            "max" => Some(AggFunc::Max),
            "min" => Some(AggFunc::Min),
            "count" => Some(AggFunc::Count),
            "first" => Some(AggFunc::First),
            "last" => Some(AggFunc::Last),
            "p50" => Some(AggFunc::Percentile(50.0)),
            "p90" => Some(AggFunc::Percentile(90.0)),
            "p99" => Some(AggFunc::Percentile(99.0)),
            "rate" => Some(AggFunc::Rate),
            "derivative" => Some(AggFunc::Derivative),
            _ => None,
        }
    }
}

pub fn aggregate(values: &[f64], func: &AggFunc) -> f64 {
    if values.is_empty() {
        return 0.0;
    }

    match func {
        AggFunc::Avg => values.iter().sum::<f64>() / values.len() as f64,
        AggFunc::Sum => values.iter().sum(),
        AggFunc::Max => values.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
        AggFunc::Min => values.iter().cloned().fold(f64::INFINITY, f64::min),
        AggFunc::Count => values.len() as f64,
        AggFunc::First => values[0],
        AggFunc::Last => values[values.len() - 1],
        AggFunc::Percentile(p) => {
            let mut sorted = values.to_vec();
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let idx = (p / 100.0 * (sorted.len() - 1) as f64).round() as usize;
            sorted[idx.min(sorted.len() - 1)]
        }
        AggFunc::Rate => {
            if values.len() < 2 {
                0.0
            } else {
                values[values.len() - 1] - values[0]
            }
        }
        AggFunc::Derivative => {
            if values.len() < 2 {
                0.0
            } else {
                values[values.len() - 1] - values[values.len() - 2]
            }
        }
    }
}

pub fn group_by_time(points: &[(i64, f64)], interval_ns: i64) -> BTreeMap<i64, Vec<f64>> {
    let mut groups: BTreeMap<i64, Vec<f64>> = BTreeMap::new();

    for (ts, val) in points {
        let bucket = (ts / interval_ns) * interval_ns;
        groups.entry(bucket).or_default().push(*val);
    }

    groups
}
