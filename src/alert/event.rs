use std::collections::BTreeMap;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertEvent {
    pub id: String,
    pub rule_id: String,
    pub rule_name: String,
    pub event_type: String,
    pub timestamp: i64,
    pub value: f64,
    pub threshold: f64,
    pub severity: String,
    pub metric: String,
    #[serde(default)]
    pub tags: BTreeMap<String, String>,
    #[serde(default)]
    pub acknowledged: bool,
    #[serde(default)]
    pub acknowledged_by: Option<String>,
}

pub struct EventStore {
    dir: std::path::PathBuf,
    max_retention_days: u64,
}

impl EventStore {
    pub fn new(data_dir: &std::path::Path) -> Self {
        let dir = data_dir.join("alerts").join("events");
        std::fs::create_dir_all(&dir).ok();
        Self {
            dir,
            max_retention_days: 7,
        }
    }

    fn day_key(ts_nanos: i64) -> String {
        let secs = ts_nanos / 1_000_000_000;
        let dt = chrono::DateTime::from_timestamp(secs, 0).unwrap_or_default();
        dt.format("%Y-%m-%d").to_string()
    }

    fn file_path(&self, day_key: &str) -> std::path::PathBuf {
        self.dir.join(format!("events-{}.jsonl", day_key))
    }

    pub fn append(&self, event: &AlertEvent) {
        let day_key = Self::day_key(event.timestamp);
        let path = self.file_path(&day_key);

        let line = match serde_json::to_string(event) {
            Ok(l) => l,
            Err(_) => return,
        };

        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
        {
            use std::io::Write;
            let _ = writeln!(f, "{}", line);
        }
    }

    pub fn query(
        &self,
        start_time: Option<i64>,
        end_time: Option<i64>,
        severity: Option<&str>,
        rule_name: Option<&str>,
        cursor: Option<i64>,
        limit: usize,
    ) -> (Vec<AlertEvent>, Option<i64>) {
        let mut all_events = Vec::new();

        if let Ok(entries) = std::fs::read_dir(&self.dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if !filename.starts_with("events-") || !filename.ends_with(".jsonl") {
                    continue;
                }

                let effective_start = if let Some(cursor_ts) = cursor {
                    Some(std::cmp::max(start_time.unwrap_or(i64::MIN), cursor_ts))
                } else {
                    start_time
                };

                if let Some(start) = effective_start {
                    let day_start_nanos = self.day_start_nanos_from_filename(filename);
                    if let Some(ds) = day_start_nanos {
                        let day_end_nanos = ds + 86400_000_000_000i64;
                        if day_end_nanos < start {
                            continue;
                        }
                    }
                }

                if let Some(end) = end_time {
                    let day_start_nanos = self.day_start_nanos_from_filename(filename);
                    if let Some(ds) = day_start_nanos {
                        if ds > end {
                            continue;
                        }
                    }
                }

                if let Ok(content) = std::fs::read_to_string(&path) {
                    for line in content.lines() {
                        if let Ok(event) = serde_json::from_str::<AlertEvent>(line) {
                            all_events.push(event);
                        }
                    }
                }
            }
        }

        if severity.is_some() || rule_name.is_some() {
            all_events.retain(|e| {
                if let Some(sev) = severity {
                    if e.severity != sev {
                        return false;
                    }
                }
                if let Some(name) = rule_name {
                    if e.rule_name != name {
                        return false;
                    }
                }
                true
            });
        }

        if start_time.is_some() || end_time.is_some() || cursor.is_some() {
            all_events.retain(|e| {
                if let Some(start) = start_time {
                    if e.timestamp < start {
                        return false;
                    }
                }
                if let Some(end) = end_time {
                    if e.timestamp > end {
                        return false;
                    }
                }
                if let Some(cursor_ts) = cursor {
                    if e.timestamp > cursor_ts {
                        return false;
                    }
                }
                true
            });
        }

        all_events.sort_by(|a, b| b.timestamp.cmp(&a.timestamp).then_with(|| b.id.cmp(&a.id)));

        if all_events.is_empty() {
            return (Vec::new(), None);
        }

        if all_events.len() <= limit {
            return (all_events, None);
        }

        let boundary_ts = all_events[limit - 1].timestamp;
        let mut end_idx = limit;
        while end_idx < all_events.len() && all_events[end_idx].timestamp == boundary_ts {
            end_idx += 1;
        }

        let next_cursor = if end_idx < all_events.len() {
            Some(all_events[end_idx].timestamp)
        } else {
            None
        };

        let events = all_events.into_iter().take(end_idx).collect();

        (events, next_cursor)
    }

    fn day_start_nanos_from_filename(&self, filename: &str) -> Option<i64> {
        let date_str = filename.strip_prefix("events-")?.strip_suffix(".jsonl")?;
        let dt = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d").ok()?;
        let dt_utc = dt.and_hms_opt(0, 0, 0)?;
        Some(dt_utc.and_utc().timestamp_nanos_opt()?)
    }

    pub fn cleanup_old(&self) {
        let now_nanos = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
        let cutoff_nanos = now_nanos - (self.max_retention_days as i64 * 86400_000_000_000i64);

        if let Ok(entries) = std::fs::read_dir(&self.dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if !filename.starts_with("events-") || !filename.ends_with(".jsonl") {
                    continue;
                }
                if let Some(day_start) = self.day_start_nanos_from_filename(filename) {
                    let day_end = day_start + 86400_000_000_000i64;
                    if day_end < cutoff_nanos {
                        let _ = std::fs::remove_file(&path);
                        tracing::info!("Cleaned up old alert events file: {}", filename);
                    }
                }
            }
        }
    }

    pub fn active_firing(&self) -> Vec<AlertEvent> {
        let mut firing_events: BTreeMap<String, AlertEvent> = BTreeMap::new();

        if let Ok(entries) = std::fs::read_dir(&self.dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if !filename.starts_with("events-") || !filename.ends_with(".jsonl") {
                    continue;
                }
                if let Ok(content) = std::fs::read_to_string(&path) {
                    for line in content.lines() {
                        if let Ok(event) = serde_json::from_str::<AlertEvent>(line) {
                            match event.event_type.as_str() {
                                "firing" => {
                                    firing_events.insert(event.rule_id.clone(), event);
                                }
                                "resolved" => {
                                    firing_events.remove(&event.rule_id);
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }

        firing_events.into_values().collect()
    }

    pub fn acknowledge_event(&self, event_id: &str, operator: &str) -> Option<AlertEvent> {
        if let Ok(entries) = std::fs::read_dir(&self.dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if !filename.starts_with("events-") || !filename.ends_with(".jsonl") {
                    continue;
                }

                if let Ok(content) = std::fs::read_to_string(&path) {
                    let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
                    let mut found_event: Option<AlertEvent> = None;

                    for line in lines.iter_mut() {
                        if let Ok(mut event) = serde_json::from_str::<AlertEvent>(line) {
                            if event.id == event_id && event.event_type == "firing" && !event.acknowledged {
                                event.acknowledged = true;
                                event.acknowledged_by = Some(operator.to_string());
                                found_event = Some(event.clone());
                                *line = serde_json::to_string(&event).unwrap_or(line.clone());
                                break;
                            }
                        }
                    }

                    if found_event.is_some() {
                        let new_content = lines.join("\n");
                        let _ = std::fs::write(&path, new_content);
                        return found_event;
                    }
                }
            }
        }
        None
    }
}
