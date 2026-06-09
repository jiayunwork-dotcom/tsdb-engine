use std::collections::BTreeMap;
use crate::engine::TsdbEngine;
use crate::engine::query::aggregation::{AggFunc, aggregate, group_by_time};
use crate::engine::query::group_by::GroupByTime;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct QueryRequest {
    pub metric: String,
    pub tags: BTreeMap<String, String>,
    pub start_time: i64,
    pub end_time: i64,
    pub field: Option<String>,
    pub aggregation: Option<String>,
    pub group_by: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct QueryResult {
    pub metric: String,
    pub series: Vec<SeriesResult>,
    pub truncated: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SeriesResult {
    pub tags: BTreeMap<String, String>,
    pub values: Vec<(i64, f64)>,
}

const QUERY_TIMEOUT_SECS: u64 = 30;

pub fn execute_query(engine: &TsdbEngine, req: &QueryRequest) -> QueryResult {
    let start = std::time::Instant::now();

    let agg_func = req.aggregation.as_ref().and_then(|a| AggFunc::from_str(a));
    let group_by = req.group_by.as_ref().and_then(|g| GroupByTime::from_str(g));
    let field_name = req.field.clone().unwrap_or_else(|| "value".to_string());

    let series_ids = if req.tags.is_empty() {
        engine.inverted_index.lookup_metric(&req.metric)
    } else {
        let indexed_tags: BTreeMap<String, String> = req.tags.iter()
            .filter(|(k, _)| engine.inverted_index.is_indexable(k))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        if indexed_tags.is_empty() {
            engine.inverted_index.lookup_metric(&req.metric)
        } else {
            engine.inverted_index.lookup_multi(&indexed_tags)
        }
    };

    let mut results = Vec::new();
    let mut truncated = false;

    for sid in &series_ids {
        let mut all_points: Vec<(i64, f64)> = Vec::new();

        {
            let active = engine.active_block.read();
            let data = active.query(*sid, req.start_time, req.end_time);
            for (ts, fields) in data {
                if let Some(v) = fields.get(&field_name).and_then(|v| v.as_f64()) {
                    all_points.push((ts, v));
                }
            }
        }

        let blocks = engine.time_index.find_blocks(&req.metric, req.start_time, req.end_time);
        for meta in &blocks {
            if let Ok(decoded) = engine.block_store.read_block(meta) {
                for series_data in &decoded.series {
                    let point_count = estimate_point_count(&series_data.header);
                    let timestamps = crate::engine::encoding::delta_of_delta::decode_timestamps(&series_data.timestamps, point_count);

                    if let Some(field_data) = series_data.fields.get(&field_name) {
                        if !field_data.is_empty() {
                            let field_type = field_data[0];
                            match field_type {
                                1 => {
                                    let float_values = crate::engine::encoding::xor::decode_floats(&field_data[1..], timestamps.len());
                                    for (i, ts) in timestamps.iter().enumerate() {
                                        if *ts >= req.start_time && *ts < req.end_time {
                                            if let Some(v) = float_values.get(i) {
                                                all_points.push((*ts, *v));
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
                                            if *ts >= req.start_time && *ts < req.end_time {
                                                if let Some((v, consumed)) = crate::engine::encoding::varint::decode_signed_varint(&int_data[int_pos..]) {
                                                    all_points.push((*ts, v as f64));
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

            if start.elapsed().as_secs() >= QUERY_TIMEOUT_SECS {
                truncated = true;
                break;
            }
        }

        if truncated { break; }

        all_points.sort_by_key(|(ts, _)| *ts);
        let tags = get_series_tags(engine, *sid);

        if let (Some(agg), Some(gb)) = (&agg_func, &group_by) {
            let groups = group_by_time(&all_points, gb.interval_ns());
            let mut agg_points = Vec::new();
            for (bucket, vals) in &groups {
                let agg_val = aggregate(vals, agg);
                agg_points.push((*bucket, agg_val));
            }
            agg_points.sort_by_key(|(ts, _)| *ts);
            results.push(SeriesResult { tags, values: agg_points });
        } else if let Some(agg) = &agg_func {
            let agg_val = aggregate(&all_points.iter().map(|(_, v)| *v).collect::<Vec<_>>(), agg);
            results.push(SeriesResult { tags, values: vec![(req.start_time, agg_val)] });
        } else {
            results.push(SeriesResult { tags, values: all_points });
        }
    }

    let elapsed = start.elapsed();
    let mut stats = engine.stats.lock();
    stats.total_queries += 1;
    stats.query_latency_p99_us = stats.query_latency_p99_us.max(elapsed.as_micros() as u64);

    QueryResult {
        metric: req.metric.clone(),
        series: results,
        truncated,
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

fn get_series_tags(engine: &TsdbEngine, series_id: u64) -> BTreeMap<String, String> {
    if let Some(entry) = engine.series_registry.get(&series_id) {
        entry.value().tags.clone()
    } else {
        BTreeMap::new()
    }
}
