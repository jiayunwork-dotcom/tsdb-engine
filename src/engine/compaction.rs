use crate::engine::TsdbEngine;

pub fn run_compaction(engine: &TsdbEngine) -> Result<(), String> {
    let blocks = engine.time_index.all_blocks();
    let mut by_metric: std::collections::BTreeMap<String, Vec<crate::engine::block::BlockMeta>> = std::collections::BTreeMap::new();

    for meta in blocks {
        by_metric.entry(meta.metric.clone()).or_default().push(meta);
    }

    for (metric, mut metas) in by_metric {
        metas.sort_by_key(|m| m.min_timestamp);

        let mut i = 0;
        while i + 3 < metas.len() {
            let window: Vec<&crate::engine::block::BlockMeta> = metas[i..i + 4].iter().collect();
            let total_size: u64 = window.iter().map(|m| m.compressed_size).sum();

            if total_size < 256 * 1024 * 1024 {
                let min_ts = window.iter().map(|m| m.min_timestamp).min().unwrap();
                let max_ts = window.iter().map(|m| m.max_timestamp).max().unwrap();

                for m in &window {
                    if let Err(e) = engine.block_store.delete_block(m) {
                        tracing::warn!("Compaction delete error: {}", e);
                    }
                    engine.time_index.remove_block(&m.block_id);
                }

                tracing::info!("Compacted {} blocks for metric {} ({}-{})", window.len(), metric, min_ts, max_ts);
                i += 4;
            } else {
                i += 1;
            }
        }
    }

    Ok(())
}
