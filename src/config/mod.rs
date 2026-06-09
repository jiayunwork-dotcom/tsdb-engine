use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Copy)]
pub enum WalSyncMode {
    EveryWrite,
    EverySecond,
    None,
}

impl WalSyncMode {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "every_write" | "everywrite" => Some(WalSyncMode::EveryWrite),
            "every_second" | "everysecond" => Some(WalSyncMode::EverySecond),
            "none" => Some(WalSyncMode::None),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionPolicy {
    pub metric: String,
    pub ttl_days: u32,
    pub downsample_7d_interval_secs: Option<i64>,
    pub downsample_30d_interval_secs: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineConfig {
    pub data_dir: String,
    pub wal_sync_mode: WalSyncMode,
    pub wal_max_size_bytes: u64,
    pub active_block_window_secs: i64,
    pub max_series_per_metric: usize,
    pub retention_policies: Vec<RetentionPolicy>,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            data_dir: "./data".to_string(),
            wal_sync_mode: WalSyncMode::EverySecond,
            wal_max_size_bytes: 64 * 1024 * 1024,
            active_block_window_secs: 7200,
            max_series_per_metric: 1_000_000,
            retention_policies: Vec::new(),
        }
    }
}
