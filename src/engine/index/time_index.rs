use std::collections::BTreeMap;
use crate::engine::block::BlockMeta;

pub struct TimeIndex {
    blocks: parking_lot::RwLock<BTreeMap<String, BlockMeta>>,
    metric_blocks: parking_lot::RwLock<BTreeMap<String, Vec<String>>>,
}

impl TimeIndex {
    pub fn new() -> Self {
        Self {
            blocks: parking_lot::RwLock::new(BTreeMap::new()),
            metric_blocks: parking_lot::RwLock::new(BTreeMap::new()),
        }
    }

    pub fn add_block(&self, meta: BlockMeta) {
        let block_id = meta.block_id.clone();
        let metric = meta.metric.clone();

        if let Some(existing) = self.blocks.read().get(&block_id) {
            if existing.min_timestamp == meta.min_timestamp && existing.max_timestamp == meta.max_timestamp {
                return;
            }
        }

        self.blocks.write().insert(block_id.clone(), meta);

        let mut mb = self.metric_blocks.write();
        mb.entry(metric).or_insert_with(Vec::new).push(block_id);
    }

    pub fn find_blocks(&self, metric: &str, start: i64, end: i64) -> Vec<BlockMeta> {
        let blocks = self.blocks.read();
        let mb = self.metric_blocks.read();

        let block_ids = match mb.get(metric) {
            Some(ids) => ids,
            None => return Vec::new(),
        };

        block_ids.iter()
            .filter_map(|id| blocks.get(id))
            .filter(|meta| meta.max_timestamp >= start && meta.min_timestamp < end)
            .cloned()
            .collect()
    }

    pub fn remove_block(&self, block_id: &str) {
        if let Some(meta) = self.blocks.write().remove(block_id) {
            let mut mb = self.metric_blocks.write();
            if let Some(ids) = mb.get_mut(&meta.metric) {
                ids.retain(|id| id != block_id);
            }
        }
    }

    pub fn block_count(&self) -> usize {
        self.blocks.read().len()
    }

    pub fn all_blocks(&self) -> Vec<BlockMeta> {
        self.blocks.read().values().cloned().collect()
    }

    pub fn get_block(&self, block_id: &str) -> Option<BlockMeta> {
        self.blocks.read().get(block_id).cloned()
    }
}
