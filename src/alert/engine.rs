use std::collections::BTreeMap;
use std::collections::VecDeque;
use std::sync::Arc;
use std::path::PathBuf;

use crate::engine::TsdbEngine;
use crate::engine::query::aggregation::{AggFunc, aggregate};
use crate::alert::rule::{AlertRule, RuleState, RuleEvalState, AggType, SubCondition};
use crate::alert::event::{AlertEvent, EventStore};
use crate::alert::notifier::AlertNotifier;

const AGG_CACHE_MAX_ENTRIES: usize = 500;
const AGG_CACHE_TTL_SECS: u64 = 15;

struct CacheEntry {
    value: f64,
    computed_at: i64,
}

struct AggCache {
    entries: BTreeMap<String, CacheEntry>,
    order: VecDeque<String>,
}

impl AggCache {
    fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
            order: VecDeque::new(),
        }
    }

    fn make_key(metric: &str, tags: &BTreeMap<String, String>, window_secs: u64) -> String {
        let mut parts = vec![metric.to_string()];
        let mut sorted_tags: Vec<_> = tags.iter().collect();
        sorted_tags.sort_by_key(|(k, _)| *k);
        for (k, v) in sorted_tags {
            parts.push(format!("{}={}", k, v));
        }
        parts.push(format!("w{}", window_secs));
        parts.join("|")
    }

    fn get(&self, key: &str, now_nanos: i64) -> Option<f64> {
        let entry = self.entries.get(key)?;
        let elapsed_nanos = now_nanos - entry.computed_at;
        let ttl_nanos = AGG_CACHE_TTL_SECS as i64 * 1_000_000_000;
        if elapsed_nanos < ttl_nanos {
            Some(entry.value)
        } else {
            None
        }
    }

    fn put(&mut self, key: String, value: f64, now_nanos: i64) {
        if self.entries.contains_key(&key) {
            if let Some(entry) = self.entries.get_mut(&key) {
                entry.value = value;
                entry.computed_at = now_nanos;
            }
            if let Some(pos) = self.order.iter().position(|k| k == &key) {
                self.order.remove(pos);
            }
            self.order.push_back(key);
        } else {
            if self.entries.len() >= AGG_CACHE_MAX_ENTRIES {
                if let Some(evict_key) = self.order.pop_front() {
                    self.entries.remove(&evict_key);
                }
            }
            self.entries.insert(key.clone(), CacheEntry { value, computed_at: now_nanos });
            self.order.push_back(key);
        }
    }
}

pub struct AlertEngine {
    engine: Arc<TsdbEngine>,
    rule_store: Arc<crate::alert::rule::RuleStore>,
    event_store: Arc<EventStore>,
    notifier: Arc<AlertNotifier>,
    eval_states: parking_lot::RwLock<BTreeMap<String, RuleEvalState>>,
    agg_cache: parking_lot::RwLock<AggCache>,
}

impl AlertEngine {
    pub fn new(
        engine: Arc<TsdbEngine>,
        data_dir: &PathBuf,
    ) -> Self {
        let rule_store = Arc::new(crate::alert::rule::RuleStore::new(data_dir));
        let event_store = Arc::new(EventStore::new(data_dir));
        let notifier = Arc::new(AlertNotifier::new());

        let mut eval_states = BTreeMap::new();
        for rule in rule_store.list() {
            eval_states.insert(rule.id.clone(), RuleEvalState::new(rule.id.clone()));
        }

        Self {
            engine,
            rule_store,
            event_store,
            notifier,
            eval_states: parking_lot::RwLock::new(eval_states),
            agg_cache: parking_lot::RwLock::new(AggCache::new()),
        }
    }

    pub fn rule_store(&self) -> Arc<crate::alert::rule::RuleStore> {
        self.rule_store.clone()
    }

    pub fn event_store(&self) -> Arc<EventStore> {
        self.event_store.clone()
    }

    pub fn notifier(&self) -> Arc<AlertNotifier> {
        self.notifier.clone()
    }

    pub fn reset_eval_state(&self, rule_id: &str) {
        let mut states = self.eval_states.write();
        states.insert(rule_id.to_string(), RuleEvalState::new(rule_id.to_string()));
    }

    pub fn remove_eval_state(&self, rule_id: &str) {
        let mut states = self.eval_states.write();
        states.remove(rule_id);
    }

    pub fn get_eval_states(&self) -> BTreeMap<String, RuleEvalState> {
        self.eval_states.read().clone()
    }

    pub fn get_eval_state(&self, rule_id: &str) -> Option<RuleEvalState> {
        self.eval_states.read().get(rule_id).cloned()
    }

    pub fn add_eval_state(&self, rule_id: &str) {
        let mut states = self.eval_states.write();
        states.insert(rule_id.to_string(), RuleEvalState::new(rule_id.to_string()));
    }

    pub fn run_eval_cycle(&self) {
        let rules = self.rule_store.enabled_rules();
        let now_nanos = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);

        for rule in rules {
            self.evaluate_rule(&rule, now_nanos);
        }

        self.event_store.cleanup_old();
    }

    fn evaluate_rule(&self, rule: &AlertRule, now_nanos: i64) {
        let conditions = rule.effective_conditions();

        let mut condition_results = Vec::with_capacity(conditions.len());
        let mut primary_value: Option<f64> = None;

        for cond in &conditions {
            let agg_value = self.query_subcondition_aggregate(cond, now_nanos);
            let agg_value = match agg_value {
                Some(v) => v,
                None => {
                    condition_results.push(false);
                    continue;
                }
            };

            if primary_value.is_none() {
                primary_value = Some(agg_value);
            }

            let met = cond.operator.compare(agg_value, cond.threshold);
            condition_results.push(met);
        }

        let condition_met = rule.evaluate_logic(&condition_results);

        let mut states = self.eval_states.write();
        let state = states.entry(rule.id.clone()).or_insert_with(|| RuleEvalState::new(rule.id.clone()));

        state.last_eval_time = Some(now_nanos);
        if let Some(v) = primary_value {
            state.current_value = Some(v);
        }

        let prev_state = state.state;

        if condition_met {
            state.consecutive_count += 1;

            if state.consecutive_count >= rule.trigger_count {
                if prev_state != RuleState::Firing && prev_state != RuleState::Acknowledged {
                    let in_silence = if let Some(last_fire) = state.last_fire_time {
                        let silence_nanos = rule.silence_secs as i64 * 1_000_000_000;
                        now_nanos < last_fire + silence_nanos
                    } else {
                        false
                    };

                    if !in_silence {
                        state.state = RuleState::Firing;
                        state.last_fire_time = Some(now_nanos);
                        drop(states);

                        let event = AlertEvent {
                            id: uuid::Uuid::new_v4().to_string(),
                            rule_id: rule.id.clone(),
                            rule_name: rule.name.clone(),
                            event_type: "firing".to_string(),
                            timestamp: now_nanos,
                            value: primary_value.unwrap_or(0.0),
                            threshold: rule.threshold,
                            severity: rule.severity.as_str().to_string(),
                            metric: rule.metric.clone(),
                            tags: rule.tags.clone(),
                            acknowledged: false,
                            acknowledged_by: None,
                        };

                        self.event_store.append(&event);
                        self.notifier.notify(&event);
                    } else {
                        state.state = RuleState::Firing;
                    }
                }
            } else {
                if prev_state != RuleState::Pending && prev_state != RuleState::Firing && prev_state != RuleState::Acknowledged {
                    state.state = RuleState::Pending;
                }
            }
        } else {
            if prev_state == RuleState::Firing || prev_state == RuleState::Acknowledged || prev_state == RuleState::Pending {
                state.state = RuleState::Resolved;
                state.consecutive_count = 0;
                drop(states);

                let event = AlertEvent {
                    id: uuid::Uuid::new_v4().to_string(),
                    rule_id: rule.id.clone(),
                    rule_name: rule.name.clone(),
                    event_type: "resolved".to_string(),
                    timestamp: now_nanos,
                    value: primary_value.unwrap_or(0.0),
                    threshold: rule.threshold,
                    severity: rule.severity.as_str().to_string(),
                    metric: rule.metric.clone(),
                    tags: rule.tags.clone(),
                    acknowledged: false,
                    acknowledged_by: None,
                };

                self.event_store.append(&event);
                self.notifier.notify(&event);
            } else {
                state.state = RuleState::Inactive;
                state.consecutive_count = 0;
            }
        }
    }

    fn query_subcondition_aggregate(&self, cond: &SubCondition, now_nanos: i64) -> Option<f64> {
        let cache_key = AggCache::make_key(&cond.metric, &cond.tags, cond.window_secs);
        {
            let cache = self.agg_cache.read();
            if let Some(cached) = cache.get(&cache_key, now_nanos) {
                return Some(cached);
            }
        }

        let result = self.query_aggregate_raw(&cond.metric, &cond.tags, cond.aggregation, cond.window_secs, now_nanos);

        if let Some(v) = result {
            let mut cache = self.agg_cache.write();
            cache.put(cache_key, v, now_nanos);
        }

        result
    }

    fn query_aggregate_raw(
        &self,
        metric: &str,
        tags: &BTreeMap<String, String>,
        aggregation: AggType,
        window_secs: u64,
        now_nanos: i64,
    ) -> Option<f64> {
        let window_nanos = window_secs as i64 * 1_000_000_000;
        let start_time = now_nanos - window_nanos;
        let end_time = now_nanos;

        let series_ids = if tags.is_empty() {
            self.engine.inverted_index.lookup_metric(metric)
        } else {
            let indexed_tags: BTreeMap<String, String> = tags.iter()
                .filter(|(k, _)| self.engine.inverted_index.is_indexable(k))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            if indexed_tags.is_empty() {
                self.engine.inverted_index.lookup_metric(metric)
            } else {
                self.engine.inverted_index.lookup_multi(&indexed_tags)
            }
        };

        if series_ids.is_empty() {
            return None;
        }

        let agg_func = match aggregation {
            AggType::Avg => AggFunc::Avg,
            AggType::Max => AggFunc::Max,
            AggType::Min => AggFunc::Min,
            AggType::Sum => AggFunc::Sum,
            AggType::Count => AggFunc::Count,
        };

        let mut all_values: Vec<f64> = Vec::new();

        for sid in &series_ids {
            {
                let active = self.engine.active_block.read();
                let data = active.query(*sid, start_time, end_time);
                for (_, fields) in data {
                    if let Some(v) = fields.get("value").and_then(|v| v.as_f64()) {
                        all_values.push(v);
                    }
                }
            }

            let blocks = self.engine.time_index.find_blocks(metric, start_time, end_time);
            for meta in &blocks {
                if let Ok(decoded) = self.engine.block_store.read_block(meta) {
                    for series_data in &decoded.series {
                        let point_count = estimate_point_count(&series_data.header);
                        let timestamps = crate::engine::encoding::delta_of_delta::decode_timestamps(&series_data.timestamps, point_count);

                        if let Some(field_data) = series_data.fields.get("value") {
                            if !field_data.is_empty() {
                                let field_type = field_data[0];
                                match field_type {
                                    1 => {
                                        let float_values = crate::engine::encoding::xor::decode_floats(&field_data[1..], timestamps.len());
                                        for (i, ts) in timestamps.iter().enumerate() {
                                            if *ts >= start_time && *ts < end_time {
                                                if let Some(v) = float_values.get(i) {
                                                    all_values.push(*v);
                                                }
                                            }
                                        }
                                    }
                                    2 => {
                                        let mut pos = 1usize;
                                        if let Some((data_len, n)) = crate::engine::encoding::varint::decode_varint(&field_data[pos..]) {
                                            pos += n;
                                            let int_data = &field_data[pos..pos + data_len as usize];
                                            let mut int_pos = 0usize;
                                            for (i, ts) in timestamps.iter().enumerate() {
                                                if *ts >= start_time && *ts < end_time {
                                                    if let Some((v, consumed)) = crate::engine::encoding::varint::decode_signed_varint(&int_data[int_pos..]) {
                                                        all_values.push(v as f64);
                                                        int_pos += consumed;
                                                    }
                                                } else {
                                                    if let Some((_, consumed)) = crate::engine::encoding::varint::decode_signed_varint(&int_data[int_pos..]) {
                                                        int_pos += consumed;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            }
        }

        if all_values.is_empty() {
            return None;
        }

        Some(aggregate(&all_values, &agg_func))
    }

    pub fn acknowledge_event(&self, event_id: &str, operator: &str) -> Option<AlertEvent> {
        let event = self.event_store.acknowledge_event(event_id, operator)?;

        let mut states = self.eval_states.write();
        if let Some(state) = states.get_mut(&event.rule_id) {
            if state.state == RuleState::Firing {
                state.state = RuleState::Acknowledged;
            }
        }

        Some(event)
    }

    pub fn start_background_eval(self: Arc<Self>) {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(15));
            loop {
                interval.tick().await;
                self.run_eval_cycle();
            }
        });
    }
}

fn estimate_point_count(header: &[u8]) -> usize {
    let mut pos = 0usize;
    if let Some((metric_len, n)) = crate::engine::encoding::varint::decode_varint(&header[pos..]) {
        pos += n + metric_len as usize;
    }
    if let Some((tag_count, n)) = crate::engine::encoding::varint::decode_varint(&header[pos..]) {
        pos += n;
        for _ in 0..tag_count {
            if let Some((_, n2)) = crate::engine::encoding::varint::decode_varint(&header[pos..]) {
                pos += n2;
            }
            if let Some((_, n2)) = crate::engine::encoding::varint::decode_varint(&header[pos..]) {
                pos += n2;
            }
        }
    }
    if pos < header.len() {
        if let Some((count, _)) = crate::engine::encoding::varint::decode_varint(&header[pos..]) {
            return count as usize;
        }
    }
    0
}
