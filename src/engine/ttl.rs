use crate::engine::TsdbEngine;
use crate::config::RetentionPolicy;
use chrono::Utc;

pub fn run_ttl(engine: &TsdbEngine, policies: &[RetentionPolicy]) -> Result<(), String> {
    let now_ns = Utc::now().timestamp_nanos_opt().unwrap_or(0);

    for policy in policies {
        let cutoff = now_ns - (policy.ttl_days as i64 * 86_400 * 1_000_000_000);
        let blocks = engine.time_index.find_blocks(&policy.metric, i64::MIN, cutoff);

        for meta in blocks {
            if meta.max_timestamp < cutoff {
                tracing::info!("TTL expiring block {} for metric {} (max_ts={})", meta.block_id, meta.metric, meta.max_timestamp);
                engine.block_store.delete_block(&meta)?;
                engine.time_index.remove_block(&meta.block_id);
            }
        }
    }

    Ok(())
}
