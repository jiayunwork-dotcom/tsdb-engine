pub mod encoding;
pub mod wal;
pub mod block;
pub mod index;
pub mod query;
pub mod active_block;
pub mod compaction;
pub mod downsample;
pub mod ttl;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use parking_lot::{RwLock, Mutex};
use dashmap::DashMap;
use crate::config::EngineConfig;

pub struct SeriesInfo {
    pub metric: String,
    pub tags: BTreeMap<String, String>,
}

pub struct TsdbEngine {
    pub config: EngineConfig,
    pub wal: Arc<wal::WalManager>,
    pub active_block: Arc<RwLock<active_block::ActiveBlock>>,
    pub block_store: Arc<block::BlockStore>,
    pub inverted_index: Arc<index::InvertedIndex>,
    pub time_index: Arc<index::TimeIndex>,
    pub dict: Arc<index::GlobalDictionary>,
    pub series_registry: Arc<DashMap<u64, SeriesInfo>>,
    pub series_count_per_metric: Arc<DashMap<String, usize>>,
    pub stats: Arc<Mutex<EngineStats>>,
}

#[derive(Default, Clone, serde::Serialize)]
pub struct EngineStats {
    pub write_qps: f64,
    pub query_latency_p99_us: u64,
    pub wal_size_bytes: u64,
    pub block_count: usize,
    pub memory_usage_bytes: u64,
    pub total_points_written: u64,
    pub total_queries: u64,
    pub wal_recovered_records: u64,
    pub wal_recovery_time_ms: u64,
}

impl TsdbEngine {
    pub fn new(config: EngineConfig) -> Result<Self, String> {
        let data_dir = PathBuf::from(&config.data_dir);
        let wal_dir = data_dir.join("wal");
        let block_dir = data_dir.join("blocks");

        std::fs::create_dir_all(&wal_dir).map_err(|e| e.to_string())?;
        std::fs::create_dir_all(&block_dir).map_err(|e| e.to_string())?;

        let dict = Arc::new(index::GlobalDictionary::new());
        let inverted_index = Arc::new(index::InvertedIndex::new());
        let time_index = Arc::new(index::TimeIndex::new());
        let block_store = Arc::new(block::BlockStore::new(block_dir));
        let series_registry = Arc::new(DashMap::new());
        let series_count_per_metric = Arc::new(DashMap::new());
        let stats = Arc::new(Mutex::new(EngineStats::default()));

        let wal_sync_mode = config.wal_sync_mode;
        let wal_max_size = config.wal_max_size_bytes;
        let active_window = config.active_block_window_secs;

        let wal = Arc::new(wal::WalManager::new(
            wal_dir,
            wal_sync_mode,
            wal_max_size,
        )?);

        let active_block = Arc::new(RwLock::new(active_block::ActiveBlock::new(
            active_window,
        )));

        let engine = Self {
            config,
            wal,
            active_block,
            block_store,
            inverted_index,
            time_index,
            dict,
            series_registry,
            series_count_per_metric,
            stats,
        };

        engine.recover_from_wal()?;

        Ok(engine)
    }

    fn recover_from_wal(&self) -> Result<(), String> {
        let start = std::time::Instant::now();
        let entries = self.wal.recover()?;
        if entries.is_empty() {
            return Ok(());
        }

        let entry_count = entries.len() as u64;
        let mut active = self.active_block.write();
        for entry in entries {
            let series_id = crate::model::compute_series_id(&entry.metric, &entry.tags);
            active.insert(series_id, entry.metric.clone(), entry.tags.clone(), entry.fields, entry.timestamp);
        }

        let elapsed_ms = start.elapsed().as_millis() as u64;

        {
            let mut stats = self.stats.lock();
            stats.wal_recovered_records = entry_count;
            stats.wal_recovery_time_ms = elapsed_ms;
        }

        tracing::info!("Recovered {} entries from WAL in {}ms", entry_count, elapsed_ms);
        Ok(())
    }

    pub fn write(&self, points: Vec<crate::model::DataPoint>) -> Result<(usize, Vec<(usize, String)>), String> {
        let mut success = 0usize;
        let mut errors = Vec::new();

        for (i, point) in points.into_iter().enumerate() {
            let series_id = crate::model::compute_series_id(&point.metric, &point.tags);

            let metric = point.metric.clone();
            let mut count = self.series_count_per_metric.entry(metric.clone()).or_insert(0);
            let current_count = *count.value();
            let already_exists = self.series_registry.contains_key(&series_id);

            if !already_exists && current_count >= self.config.max_series_per_metric {
                errors.push((i, format!("Series limit exceeded for metric {}", metric)));
                continue;
            }

            self.wal.append(&point)?;

            {
                let mut active = self.active_block.write();
                active.insert(series_id, point.metric.clone(), point.tags.clone(), point.fields.clone(), point.timestamp);
            }

            if !already_exists {
                self.inverted_index.add_metric_series(&metric, series_id);

                self.series_registry.insert(series_id, SeriesInfo {
                    metric: point.metric.clone(),
                    tags: point.tags.clone(),
                });
                *count.value_mut() += 1;

                for (k, v) in &point.tags {
                    self.inverted_index.add(k, v, series_id);
                    self.dict.get_or_create_id(k, v);
                }
            }

            success += 1;
        }

        let mut stats = self.stats.lock();
        stats.total_points_written += success as u64;

        Ok((success, errors))
    }

    pub fn check_and_flush(&self) -> Result<(), String> {
        let should_flush = {
            let active = self.active_block.read();
            active.should_flush()
        };

        if should_flush {
            let block_data = {
                let mut active = self.active_block.write();
                active.drain_for_flush()
            };

            if let Some(data) = block_data {
                let block_meta = self.block_store.write_block(&data, &self.dict)?;
                self.time_index.add_block(block_meta.clone());

                let max_ts = block_meta.max_timestamp;
                self.wal.truncate_before(max_ts);

                let mut stats = self.stats.lock();
                stats.block_count = self.time_index.block_count();
            }
        }

        Ok(())
    }

    pub fn get_stats(&self) -> EngineStats {
        self.stats.lock().clone()
    }

    pub fn delete_points(&self, metric: &str, tags: &std::collections::BTreeMap<String, String>, start_time: i64, end_time: i64) -> usize {
        let exists = self.series_count_per_metric.contains_key(metric);
        if !exists {
            return 0;
        }

        let mut active = self.active_block.write();
        active.delete_range(metric, tags, start_time, end_time)
    }

    pub fn health_check(&self) -> serde_json::Value {
        let stats = self.stats.lock();
        serde_json::json!({
            "status": "ok",
            "wal_size_bytes": stats.wal_size_bytes,
            "block_count": stats.block_count,
            "memory_usage_bytes": stats.memory_usage_bytes,
            "write_qps": stats.write_qps,
            "query_latency_p99_us": stats.query_latency_p99_us,
            "wal_recovered_records": stats.wal_recovered_records,
            "wal_recovery_time_ms": stats.wal_recovery_time_ms,
        })
    }

    pub fn list_metrics(&self) -> Vec<String> {
        self.series_count_per_metric.iter().map(|e| e.key().clone()).collect()
    }

    pub fn list_tags(&self, metric: &str) -> Vec<(String, Vec<String>)> {
        self.inverted_index.get_tags_for_metric(metric, &self.series_registry, &self.series_count_per_metric)
    }

    pub fn series_count(&self) -> usize {
        self.series_registry.len()
    }

    pub fn run_background_tasks(&self) {
        let _config = self.config.clone();

        let flush_engine = Arc::downgrade(&Arc::new(self.clone_inner()));
        let _flush_handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
            loop {
                interval.tick().await;
                if let Some(eng) = flush_engine.upgrade() {
                    if let Err(e) = eng.check_and_flush() {
                        tracing::error!("Flush error: {}", e);
                    }
                } else {
                    break;
                }
            }
        });

        let ttl_engine = Arc::downgrade(&Arc::new(self.clone_inner()));
        let retention = self.config.retention_policies.clone();
        let _ttl_handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600));
            loop {
                interval.tick().await;
                if let Some(eng) = ttl_engine.upgrade() {
                    if let Err(e) = ttl::run_ttl(&eng, &retention) {
                        tracing::error!("TTL error: {}", e);
                    }
                } else {
                    break;
                }
            }
        });

        let stats_engine = Arc::downgrade(&Arc::new(self.clone_inner()));
        let _stats_handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
            let mut prev_count: u64 = 0;
            loop {
                interval.tick().await;
                if let Some(eng) = stats_engine.upgrade() {
                    let mut stats = eng.stats.lock();
                    let current = stats.total_points_written;
                    stats.write_qps = (current - prev_count) as f64;
                    prev_count = current;
                    stats.wal_size_bytes = eng.wal.current_size();
                    stats.memory_usage_bytes = eng.active_block.read().memory_size();
                } else {
                    break;
                }
            }
        });
    }

    fn clone_inner(&self) -> Self {
        Self {
            config: self.config.clone(),
            wal: self.wal.clone(),
            active_block: self.active_block.clone(),
            block_store: self.block_store.clone(),
            inverted_index: self.inverted_index.clone(),
            time_index: self.time_index.clone(),
            dict: self.dict.clone(),
            series_registry: self.series_registry.clone(),
            series_count_per_metric: self.series_count_per_metric.clone(),
            stats: self.stats.clone(),
        }
    }
}

unsafe impl Send for TsdbEngine {}
unsafe impl Sync for TsdbEngine {}
