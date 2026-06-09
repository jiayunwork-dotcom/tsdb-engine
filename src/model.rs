use serde::{Serialize, Deserialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataPoint {
    pub metric: String,
    pub tags: BTreeMap<String, String>,
    pub fields: BTreeMap<String, FieldValue>,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FieldValue {
    Float(f64),
    Integer(i64),
    String(String),
    Bool(bool),
}

impl FieldValue {
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            FieldValue::Float(v) => Some(*v),
            FieldValue::Integer(v) => Some(*v as f64),
            FieldValue::Bool(v) => Some(if *v { 1.0 } else { 0.0 }),
            _ => None,
        }
    }
}

pub fn compute_series_id(metric: &str, tags: &BTreeMap<String, String>) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    metric.hash(&mut hasher);
    let mut sorted_tags: Vec<(&String, &String)> = tags.iter().collect();
    sorted_tags.sort_by_key(|(k, _)| *k);
    for (k, v) in sorted_tags {
        k.hash(&mut hasher);
        v.hash(&mut hasher);
    }
    hasher.finish()
}

pub fn parse_line_protocol(line: &str) -> Result<DataPoint, String> {
    let line = line.trim();
    if line.is_empty() {
        return Err("empty line".to_string());
    }

    let parts: Vec<&str> = line.splitn(3, ' ').collect();
    if parts.len() < 2 {
        return Err("invalid line protocol format: need at least measurement and fields".to_string());
    }

    let measurement_with_tags = parts[0];
    let field_str = parts[1];
    let timestamp = if parts.len() == 3 {
        parts[2].parse::<i64>().map_err(|e| format!("invalid timestamp: {}", e))?
    } else {
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
    };

    let (metric, tags) = parse_measurement_and_tags(measurement_with_tags)?;
    let fields = parse_fields(field_str)?;

    Ok(DataPoint {
        metric,
        tags,
        fields,
        timestamp,
    })
}

fn parse_measurement_and_tags(input: &str) -> Result<(String, BTreeMap<String, String>), String> {
    let mut tags = BTreeMap::new();

    let comma_pos = input.find(',');
    let metric = if let Some(pos) = comma_pos {
        let m = input[..pos].to_string();
        let tag_str = &input[pos + 1..];
        for pair in tag_str.split(',') {
            let kv: Vec<&str> = pair.splitn(2, '=').collect();
            if kv.len() == 2 {
                tags.insert(kv[0].to_string(), kv[1].to_string());
            }
        }
        m
    } else {
        input.to_string()
    };

    if metric.is_empty() {
        return Err("metric name cannot be empty".to_string());
    }

    Ok((metric, tags))
}

fn parse_fields(input: &str) -> Result<BTreeMap<String, FieldValue>, String> {
    let mut fields = BTreeMap::new();

    for pair in input.split(',') {
        let kv: Vec<&str> = pair.splitn(2, '=').collect();
        if kv.len() != 2 {
            return Err(format!("invalid field pair: {}", pair));
        }
        let key = kv[0].to_string();
        let value_str = kv[1];

        let value = if value_str.ends_with('i') {
            let int_str = &value_str[..value_str.len() - 1];
            FieldValue::Integer(int_str.parse::<i64>().map_err(|e| format!("invalid integer {}: {}", int_str, e))?)
        } else if value_str == "true" || value_str == "TRUE" {
            FieldValue::Bool(true)
        } else if value_str == "false" || value_str == "FALSE" {
            FieldValue::Bool(false)
        } else if value_str.starts_with('"') && value_str.ends_with('"') {
            FieldValue::String(value_str[1..value_str.len() - 1].to_string())
        } else {
            FieldValue::Float(value_str.parse::<f64>().map_err(|e| format!("invalid float {}: {}", value_str, e))?)
        };

        fields.insert(key, value);
    }

    if fields.is_empty() {
        return Err("at least one field is required".to_string());
    }

    Ok(fields)
}

pub fn parse_batch(text: &str) -> (Vec<DataPoint>, Vec<(usize, String)>) {
    let mut points = Vec::new();
    let mut errors = Vec::new();

    for (i, line) in text.lines().enumerate() {
        match parse_line_protocol(line) {
            Ok(point) => points.push(point),
            Err(e) => errors.push((i, e)),
        }
    }

    (points, errors)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_line_protocol() {
        let line = "cpu,host=server01,region=us-west usage=80.5,cores=4i 1609459200000000000";
        let dp = parse_line_protocol(line).unwrap();
        assert_eq!(dp.metric, "cpu");
        assert_eq!(dp.tags.get("host").unwrap(), "server01");
        assert_eq!(dp.tags.get("region").unwrap(), "us-west");
        assert!(matches!(dp.fields.get("usage").unwrap(), FieldValue::Float(v) if *v == 80.5));
        assert!(matches!(dp.fields.get("cores").unwrap(), FieldValue::Integer(v) if *v == 4));
        assert_eq!(dp.timestamp, 1609459200000000000);
    }

    #[test]
    fn test_parse_no_tags() {
        let line = "temperature value=23.5 1609459200000000000";
        let dp = parse_line_protocol(line).unwrap();
        assert_eq!(dp.metric, "temperature");
        assert!(dp.tags.is_empty());
    }

    #[test]
    fn test_parse_no_timestamp() {
        let line = "cpu,host=server01 usage=80.5";
        let dp = parse_line_protocol(line).unwrap();
        assert_eq!(dp.metric, "cpu");
        assert!(dp.timestamp > 0);
    }
}
