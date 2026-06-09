use std::collections::BTreeMap;
use crate::model::FieldValue;

pub struct ActiveBlock {
    window_secs: i64,
    series_data: BTreeMap<u64, SeriesBuffer>,
    window_start: i64,
}

struct SeriesBuffer {
    metric: String,
    tags: BTreeMap<String, String>,
    points: Vec<PointData>,
}

struct PointData {
    timestamp: i64,
    fields: BTreeMap<String, FieldValue>,
}

impl ActiveBlock {
    pub fn new(window_secs: i64) -> Self {
        Self {
            window_secs,
            series_data: BTreeMap::new(),
            window_start: 0,
        }
    }

    pub fn insert(&mut self, series_id: u64, metric: String, tags: BTreeMap<String, String>, fields: BTreeMap<String, FieldValue>, timestamp: i64) {
        if self.window_start == 0 {
            self.window_start = align_to_window(timestamp, self.window_secs);
        }

        let buffer = self.series_data.entry(series_id).or_insert_with(|| SeriesBuffer {
            metric,
            tags,
            points: Vec::new(),
        });

        buffer.points.push(PointData { timestamp, fields });
    }

    pub fn should_flush(&self) -> bool {
        if self.series_data.is_empty() {
            return false;
        }

        let now = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
        let window_end = self.window_start + self.window_secs * 1_000_000_000;

        now >= window_end
    }

    pub fn drain_for_flush(&mut self) -> Option<FlushData> {
        if self.series_data.is_empty() {
            return None;
        }

        let min_ts = self.series_data.values()
            .flat_map(|s| s.points.iter().map(|p| p.timestamp))
            .min()
            .unwrap_or(0);

        let max_ts = self.series_data.values()
            .flat_map(|s| s.points.iter().map(|p| p.timestamp))
            .max()
            .unwrap_or(0);

        let series_count = self.series_data.len();
        let total_points: usize = self.series_data.values().map(|s| s.points.len()).sum();

        let series: Vec<FlushSeries> = std::mem::take(&mut self.series_data)
            .into_iter()
            .map(|(id, buf)| FlushSeries {
                series_id: id,
                metric: buf.metric,
                tags: buf.tags,
                points: buf.points.into_iter().map(|p| FlushPoint {
                    timestamp: p.timestamp,
                    fields: p.fields,
                }).collect(),
            })
            .collect();

        self.window_start = 0;

        Some(FlushData {
            min_timestamp: min_ts,
            max_timestamp: max_ts,
            series_count,
            total_points,
            series,
        })
    }

    pub fn memory_size(&self) -> u64 {
        let mut size: u64 = 0;
        for buffer in self.series_data.values() {
            size += buffer.metric.len() as u64;
            for (k, v) in &buffer.tags {
                size += k.len() as u64 + v.len() as u64;
            }
            size += buffer.points.len() as u64 * 64;
        }
        size
    }

    pub fn query(&self, series_id: u64, start: i64, end: i64) -> Vec<(i64, BTreeMap<String, FieldValue>)> {
        if let Some(buffer) = self.series_data.get(&series_id) {
            buffer.points.iter()
                .filter(|p| p.timestamp >= start && p.timestamp < end)
                .map(|p| (p.timestamp, p.fields.clone()))
                .collect()
        } else {
            Vec::new()
        }
    }

    pub fn query_metric(&self, metric: &str, start: i64, end: i64) -> Vec<(u64, String, BTreeMap<String, String>, Vec<(i64, BTreeMap<String, FieldValue>)>)> {
        let mut results = Vec::new();
        for (&sid, buffer) in &self.series_data {
            if buffer.metric != metric {
                continue;
            }
            let points: Vec<(i64, BTreeMap<String, FieldValue>)> = buffer.points.iter()
                .filter(|p| p.timestamp >= start && p.timestamp < end)
                .map(|p| (p.timestamp, p.fields.clone()))
                .collect();
            if !points.is_empty() {
                results.push((sid, buffer.metric.clone(), buffer.tags.clone(), points));
            }
        }
        results
    }
}

fn align_to_window(ts: i64, window_secs: i64) -> i64 {
    let window_nanos = window_secs * 1_000_000_000;
    (ts / window_nanos) * window_nanos
}

#[derive(Debug)]
pub struct FlushData {
    pub min_timestamp: i64,
    pub max_timestamp: i64,
    pub series_count: usize,
    pub total_points: usize,
    pub series: Vec<FlushSeries>,
}

#[derive(Debug)]
pub struct FlushSeries {
    pub series_id: u64,
    pub metric: String,
    pub tags: BTreeMap<String, String>,
    pub points: Vec<FlushPoint>,
}

#[derive(Debug)]
pub struct FlushPoint {
    pub timestamp: i64,
    pub fields: BTreeMap<String, FieldValue>,
}
