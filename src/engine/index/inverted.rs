use std::collections::{BTreeMap, HashSet, HashMap};
use dashmap::DashMap;
use crate::engine::SeriesInfo;

pub struct InvertedIndex {
    index: DashMap<String, DashMap<String, HashSet<u64>>>,
    metric_series: DashMap<String, HashSet<u64>>,
    tag_values: DashMap<String, HashSet<String>>,
}

impl InvertedIndex {
    pub fn new() -> Self {
        Self {
            index: DashMap::new(),
            metric_series: DashMap::new(),
            tag_values: DashMap::new(),
        }
    }

    pub fn add(&self, tag_key: &str, tag_value: &str, series_id: u64) {
        let key_entry = self.index.entry(tag_key.to_string()).or_insert_with(DashMap::new);
        let mut value_entry = key_entry.entry(tag_value.to_string()).or_insert_with(|| HashSet::new());
        value_entry.insert(series_id);

        let mut tv_entry = self.tag_values.entry(tag_key.to_string()).or_insert_with(|| HashSet::new());
        if tv_entry.len() < 100_000 {
            tv_entry.insert(tag_value.to_string());
        }
    }

    pub fn add_metric_series(&self, metric: &str, series_id: u64) {
        let mut entry = self.metric_series.entry(metric.to_string()).or_insert_with(|| HashSet::new());
        entry.insert(series_id);
    }

    pub fn lookup(&self, tag_key: &str, tag_value: &str) -> HashSet<u64> {
        if let Some(key_map) = self.index.get(tag_key) {
            if let Some(ids) = key_map.get(tag_value) {
                return ids.clone();
            }
        }
        HashSet::new()
    }

    pub fn lookup_multi(&self, filters: &BTreeMap<String, String>) -> HashSet<u64> {
        let mut result: Option<HashSet<u64>> = None;

        for (key, value) in filters {
            let ids = self.lookup(key, value);
            result = Some(match result {
                None => ids,
                Some(prev) => prev.intersection(&ids).cloned().collect(),
            });
        }

        result.unwrap_or_default()
    }

    pub fn lookup_metric(&self, metric: &str) -> HashSet<u64> {
        self.metric_series.get(metric).map(|s| s.clone()).unwrap_or_default()
    }

    pub fn get_tags_for_metric(
        &self,
        metric: &str,
        series_registry: &DashMap<u64, SeriesInfo>,
        _series_count: &dashmap::DashMap<String, usize>,
    ) -> Vec<(String, Vec<String>)> {
        let series_ids = self.lookup_metric(metric);
        let mut tag_keys: HashMap<String, HashSet<String>> = HashMap::new();

        for sid in &series_ids {
            if let Some(entry) = series_registry.get(sid) {
                let info = entry.value();
                for (k, v) in &info.tags {
                    tag_keys.entry(k.clone())
                        .or_insert_with(HashSet::new)
                        .insert(v.clone());
                }
            }
        }

        let mut result: Vec<(String, Vec<String>)> = tag_keys.into_iter()
            .map(|(k, vs)| {
                let mut vals: Vec<String> = vs.into_iter().collect();
                vals.sort();
                (k, vals)
            })
            .collect();
        result.sort_by(|a, b| a.0.cmp(&b.0));
        result
    }

    pub fn is_indexable(&self, tag_key: &str) -> bool {
        if let Some(tv) = self.tag_values.get(tag_key) {
            tv.len() < 100_000
        } else {
            true
        }
    }
}
