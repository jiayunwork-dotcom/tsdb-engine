use std::path::PathBuf;
use std::collections::BTreeMap;
use std::fs;
use std::io::{Write, Read};
use crate::engine::active_block::{FlushData, FlushSeries};
use crate::engine::encoding::{delta_of_delta, xor, varint};
use crate::engine::index::GlobalDictionary;
use crate::model::FieldValue;

const BLOCK_MAGIC: &[u8; 4] = b"BLK\0";
const BLOCK_HEADER_SIZE: usize = 4 + 8 + 8 + 4 + 8 + 8 + 4;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BlockMeta {
    pub block_id: String,
    pub min_timestamp: i64,
    pub max_timestamp: i64,
    pub series_count: u32,
    pub compressed_size: u64,
    pub original_size: u64,
    pub crc32: u32,
    pub file_path: String,
    pub metric: String,
}

pub struct BlockStore {
    dir: PathBuf,
}

impl BlockStore {
    pub fn new(dir: PathBuf) -> Self {
        fs::create_dir_all(&dir).ok();
        Self { dir }
    }

    pub fn write_block(&self, data: &FlushData, dict: &GlobalDictionary) -> Result<BlockMeta, String> {
        let block_id = uuid::Uuid::new_v4().to_string();
        let file_path = self.dir.join(format!("{}.blk", block_id));

        let mut compressed = Vec::new();
        let mut original_size: u64 = 0;

        let metric = data.series.first().map(|s| s.metric.clone()).unwrap_or_default();

        for series in &data.series {
            let series_header = encode_series_header(series, dict);
            let timestamps = series.points.iter().map(|p| p.timestamp).collect::<Vec<_>>();
            let ts_encoded = delta_of_delta::encode_timestamps(&timestamps);

            let mut fields_encoded = BTreeMap::new();
            for (field_name, _) in &series.points.first().unwrap().fields {
                let float_values: Vec<f64> = series.points.iter()
                    .filter_map(|p| p.fields.get(field_name).and_then(|v| v.as_f64()))
                    .collect();
                let int_values: Vec<i64> = series.points.iter()
                    .filter_map(|p| match p.fields.get(field_name) {
                        Some(FieldValue::Integer(v)) => Some(*v),
                        _ => None,
                    })
                    .collect();

                let has_floats = !float_values.is_empty();
                let has_ints = !int_values.is_empty();

                let mut field_data = Vec::new();
                if has_floats {
                    field_data.push(1u8);
                    field_data.extend_from_slice(&xor::encode_floats(&float_values));
                } else if has_ints {
                    field_data.push(2u8);
                    let mut buf = Vec::new();
                    for v in &int_values {
                        varint::encode_signed_varint(*v, &mut buf);
                    }
                    varint::encode_varint(buf.len() as u64, &mut field_data);
                    field_data.extend_from_slice(&buf);
                } else {
                    field_data.push(0u8);
                }

                fields_encoded.insert(field_name.clone(), field_data);
            }

            original_size += series_header.len() as u64 + ts_encoded.len() as u64;

            varint::encode_varint(series_header.len() as u64, &mut compressed);
            compressed.extend_from_slice(&series_header);
            varint::encode_varint(ts_encoded.len() as u64, &mut compressed);
            compressed.extend_from_slice(&ts_encoded);
            varint::encode_varint(fields_encoded.len() as u64, &mut compressed);
            for (name, data) in &fields_encoded {
                let name_bytes = name.as_bytes();
                varint::encode_varint(name_bytes.len() as u64, &mut compressed);
                compressed.extend_from_slice(name_bytes);
                varint::encode_varint(data.len() as u64, &mut compressed);
                compressed.extend_from_slice(data);
            }
        }

        let mut hasher = crc32fast::Hasher::new();
        hasher.update(&compressed);
        let crc = hasher.finalize();

        let mut file = fs::File::create(&file_path).map_err(|e| e.to_string())?;

        let mut header = Vec::new();
        header.extend_from_slice(BLOCK_MAGIC);
        header.extend_from_slice(&data.min_timestamp.to_be_bytes());
        header.extend_from_slice(&data.max_timestamp.to_be_bytes());
        header.extend_from_slice(&(data.series_count as u32).to_be_bytes());
        header.extend_from_slice(&original_size.to_be_bytes());
        header.extend_from_slice(&(compressed.len() as u64).to_be_bytes());
        header.extend_from_slice(&crc.to_be_bytes());

        file.write_all(&header).map_err(|e| e.to_string())?;
        file.write_all(&compressed).map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;

        Ok(BlockMeta {
            block_id,
            min_timestamp: data.min_timestamp,
            max_timestamp: data.max_timestamp,
            series_count: data.series_count as u32,
            compressed_size: compressed.len() as u64,
            original_size,
            crc32: crc,
            file_path: file_path.to_string_lossy().to_string(),
            metric,
        })
    }

    pub fn read_block(&self, meta: &BlockMeta) -> Result<DecodedBlock, String> {
        let path = PathBuf::from(&meta.file_path);
        let mut file = fs::File::open(&path).map_err(|e| e.to_string())?;
        let mut data = Vec::new();
        file.read_to_end(&mut data).map_err(|e| e.to_string())?;

        if data.len() < BLOCK_HEADER_SIZE {
            return Err("block file too small".to_string());
        }

        let mut hasher = crc32fast::Hasher::new();
        hasher.update(&data[BLOCK_HEADER_SIZE..]);
        let computed_crc = hasher.finalize();
        if computed_crc != meta.crc32 {
            return Err("CRC mismatch".to_string());
        }

        let compressed = &data[BLOCK_HEADER_SIZE..];
        let mut series_list = Vec::new();
        let mut pos = 0usize;

        while pos < compressed.len() {
            let (header_len, n) = varint::decode_varint(&compressed[pos..]).ok_or("varint decode error")?;
            pos += n;
            let header_data = compressed[pos..pos + header_len as usize].to_vec();
            pos += header_len as usize;

            let (ts_len, n) = varint::decode_varint(&compressed[pos..]).ok_or("varint decode error")?;
            pos += n;
            let ts_data = compressed[pos..pos + ts_len as usize].to_vec();
            pos += ts_len as usize;

            let (field_count, n) = varint::decode_varint(&compressed[pos..]).ok_or("varint decode error")?;
            pos += n;

            let mut fields_data = BTreeMap::new();
            for _ in 0..field_count {
                let (name_len, n) = varint::decode_varint(&compressed[pos..]).ok_or("varint decode error")?;
                pos += n;
                let name = String::from_utf8(compressed[pos..pos + name_len as usize].to_vec()).map_err(|e| e.to_string())?;
                pos += name_len as usize;

                let (data_len, n) = varint::decode_varint(&compressed[pos..]).ok_or("varint decode error")?;
                pos += n;
                let fdata = compressed[pos..pos + data_len as usize].to_vec();
                pos += data_len as usize;

                fields_data.insert(name, fdata);
            }

            series_list.push(SeriesBlockData {
                header: header_data,
                timestamps: ts_data,
                fields: fields_data,
            });
        }

        Ok(DecodedBlock { series: series_list })
    }

    pub fn delete_block(&self, meta: &BlockMeta) -> Result<(), String> {
        let path = PathBuf::from(&meta.file_path);
        if path.exists() {
            fs::remove_file(&path).map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    pub fn list_blocks(&self) -> Vec<String> {
        let mut blocks = Vec::new();
        if let Ok(entries) = fs::read_dir(&self.dir) {
            for entry in entries.flatten() {
                if entry.path().extension().map(|e| e == "blk").unwrap_or(false) {
                    if let Some(name) = entry.path().file_stem() {
                        blocks.push(name.to_string_lossy().to_string());
                    }
                }
            }
        }
        blocks
    }
}

fn encode_series_header(series: &FlushSeries, dict: &GlobalDictionary) -> Vec<u8> {
    let mut buf = Vec::new();
    let metric_bytes = series.metric.as_bytes();
    varint::encode_varint(metric_bytes.len() as u64, &mut buf);
    buf.extend_from_slice(metric_bytes);

    varint::encode_varint(series.tags.len() as u64, &mut buf);
    for (k, v) in &series.tags {
        let kid = dict.get_key_id(k);
        let vid = dict.get_or_create_id(k, v);
        varint::encode_varint(kid, &mut buf);
        varint::encode_varint(vid, &mut buf);
    }

    varint::encode_varint(series.points.len() as u64, &mut buf);
    buf
}

pub struct DecodedBlock {
    pub series: Vec<SeriesBlockData>,
}

pub struct SeriesBlockData {
    pub header: Vec<u8>,
    pub timestamps: Vec<u8>,
    pub fields: BTreeMap<String, Vec<u8>>,
}
