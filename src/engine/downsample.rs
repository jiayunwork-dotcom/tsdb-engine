use crate::engine::TsdbEngine;
use crate::engine::query::aggregation::{AggFunc, aggregate, group_by_time};

pub struct DownsamplePolicy {
    pub age_threshold_ns: i64,
    pub interval_ns: i64,
    pub agg_func: AggFunc,
}

pub fn run_downsample(engine: &TsdbEngine, policies: &[DownsamplePolicy]) -> Result<(), String> {
    let now = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);

    for policy in policies {
        let cutoff = now - policy.age_threshold_ns;
        let blocks = engine.time_index.all_blocks();

        for meta in &blocks {
            if meta.max_timestamp < cutoff {
                if let Ok(decoded) = engine.block_store.read_block(meta) {
                    for series_data in &decoded.series {
                        let point_count = estimate_point_count(&series_data.header);
                        let timestamps = crate::engine::encoding::delta_of_delta::decode_timestamps(&series_data.timestamps, point_count);

                        for (_field_name, field_bytes) in &series_data.fields {
                            if field_bytes.is_empty() || field_bytes[0] != 1 {
                                continue;
                            }
                            let float_values = crate::engine::encoding::xor::decode_floats(&field_bytes[1..], timestamps.len());

                            let points: Vec<(i64, f64)> = timestamps.iter().zip(float_values.iter())
                                .map(|(ts, v)| (*ts, *v))
                                .collect();

                            let groups = group_by_time(&points, policy.interval_ns);
                            let mut _downsampled = Vec::new();
                            for (bucket, vals) in &groups {
                                let agg_val = aggregate(vals, &policy.agg_func);
                                _downsampled.push((*bucket, agg_val));
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(())
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
