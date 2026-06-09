use std::path::{PathBuf, Path};
use std::io::{Write, Read};
use std::fs;
use parking_lot::Mutex;
use crate::model::DataPoint;
use crate::config::WalSyncMode;

const WAL_MAGIC: &[u8; 4] = b"WAL\0";
const ENTRY_MAGIC: &[u8; 2] = b"WE";
const WAL_HEADER_SIZE: usize = 4;

struct WalInner {
    current_file: Option<fs::File>,
    current_path: Option<PathBuf>,
    current_size: u64,
    sequence: u64,
}

pub struct WalManager {
    dir: PathBuf,
    sync_mode: WalSyncMode,
    max_file_size: u64,
    inner: Mutex<WalInner>,
}

impl WalManager {
    pub fn new(dir: PathBuf, sync_mode: WalSyncMode, max_file_size: u64) -> Result<Self, String> {
        fs::create_dir_all(&dir).map_err(|e| format!("Failed to create WAL dir: {}", e))?;

        let mut sequence = 0u64;
        if let Ok(entries) = fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if let Some(seq) = name_str.strip_prefix("wal_").and_then(|s| s.strip_suffix(".log")) {
                    if let Ok(s) = seq.parse::<u64>() {
                        sequence = sequence.max(s);
                    }
                }
            }
        }

        let mut inner = WalInner {
            current_file: None,
            current_path: None,
            current_size: 0,
            sequence,
        };

        rotate_inner(&dir, &sync_mode, &mut inner)?;

        Ok(Self {
            dir,
            sync_mode,
            max_file_size,
            inner: Mutex::new(inner),
        })
    }

    pub fn append(&self, point: &DataPoint) -> Result<(), String> {
        let mut inner = self.inner.lock();

        if inner.current_size >= self.max_file_size {
            rotate_inner(&self.dir, &self.sync_mode, &mut inner)?;
        }

        let json = serde_json::to_vec(point).map_err(|e| format!("WAL serialize error: {}", e))?;
        let len = json.len() as u32;

        let file = inner.current_file.as_mut().ok_or("WAL file not open")?;

        file.write_all(ENTRY_MAGIC).map_err(|e| format!("WAL write error: {}", e))?;
        file.write_all(&len.to_be_bytes()).map_err(|e| format!("WAL write error: {}", e))?;
        file.write_all(&json).map_err(|e| format!("WAL write error: {}", e))?;

        let mut hasher = crc32fast::Hasher::new();
        hasher.update(&json);
        let crc = hasher.finalize();
        file.write_all(&crc.to_be_bytes()).map_err(|e| format!("WAL write error: {}", e))?;

        if self.sync_mode == WalSyncMode::EveryWrite {
            file.sync_all().map_err(|e| format!("WAL sync error: {}", e))?;
        }

        inner.current_size += 2 + 4 + json.len() as u64 + 4;

        Ok(())
    }

    pub fn recover(&self) -> Result<Vec<DataPoint>, String> {
        let mut all_points = Vec::new();

        let mut wal_files: Vec<PathBuf> = Vec::new();
        if let Ok(entries) = fs::read_dir(&self.dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map(|e| e == "log").unwrap_or(false) {
                    wal_files.push(path);
                }
            }
        }
        wal_files.sort();

        for path in &wal_files {
            match Self::recover_file(path) {
                Ok(points) => all_points.extend(points),
                Err(e) => {
                    tracing::warn!("WAL recovery warning for {:?}: {}", path, e);
                }
            }
        }

        Ok(all_points)
    }

    fn recover_file(path: &Path) -> Result<Vec<DataPoint>, String> {
        let mut file = fs::File::open(path).map_err(|e| format!("open error: {}", e))?;
        let mut data = Vec::new();
        file.read_to_end(&mut data).map_err(|e| format!("read error: {}", e))?;

        if data.len() < WAL_HEADER_SIZE || &data[..4] != WAL_MAGIC {
            return Err("invalid WAL header".to_string());
        }

        let mut points = Vec::new();
        let mut pos = WAL_HEADER_SIZE;

        while pos + 2 + 4 < data.len() {
            if &data[pos..pos + 2] != ENTRY_MAGIC {
                pos += 1;
                continue;
            }
            pos += 2;

            let len = u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
            pos += 4;

            if pos + len + 4 > data.len() {
                tracing::warn!("WAL entry truncated at pos {}", pos);
                break;
            }

            let json_data = &data[pos..pos + len];
            pos += len;

            let stored_crc = u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
            pos += 4;

            let mut hasher = crc32fast::Hasher::new();
            hasher.update(json_data);
            let computed_crc = hasher.finalize();

            if stored_crc != computed_crc {
                tracing::warn!("WAL entry CRC mismatch, skipping entry");
                continue;
            }

            match serde_json::from_slice::<DataPoint>(json_data) {
                Ok(point) => points.push(point),
                Err(e) => {
                    tracing::warn!("WAL entry parse error: {}", e);
                }
            }
        }

        Ok(points)
    }

    pub fn truncate_before(&self, timestamp: i64) {
        let current_path = {
            let inner = self.inner.lock();
            inner.current_path.clone()
        };

        if let Ok(entries) = fs::read_dir(&self.dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if Some(&path) == current_path.as_ref() {
                    continue;
                }
                if let Ok(points) = Self::recover_file(&path) {
                    let all_before = points.iter().all(|p| p.timestamp < timestamp);
                    if all_before {
                        if let Err(e) = fs::remove_file(&path) {
                            tracing::warn!("Failed to remove old WAL file {:?}: {}", path, e);
                        }
                    }
                }
            }
        }
    }

    pub fn current_size(&self) -> u64 {
        self.inner.lock().current_size
    }
}

fn rotate_inner(dir: &PathBuf, sync_mode: &WalSyncMode, inner: &mut WalInner) -> Result<(), String> {
    inner.sequence += 1;
    let path = dir.join(format!("wal_{:08}.log", inner.sequence));
    let mut file = fs::File::create(&path).map_err(|e| format!("Failed to create WAL file: {}", e))?;
    file.write_all(WAL_MAGIC).map_err(|e| format!("WAL header write error: {}", e))?;

    if *sync_mode == WalSyncMode::EveryWrite {
        file.sync_all().map_err(|e| format!("WAL sync error: {}", e))?;
    }

    inner.current_file = Some(file);
    inner.current_path = Some(path);
    inner.current_size = WAL_HEADER_SIZE as u64;

    Ok(())
}
