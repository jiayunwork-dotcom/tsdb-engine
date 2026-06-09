use crate::engine::encoding::{BitWriter, BitReader};

pub fn encode_timestamps(timestamps: &[i64]) -> Vec<u8> {
    if timestamps.is_empty() {
        return Vec::new();
    }

    let mut writer = BitWriter::new();

    let first = timestamps[0] as u64;
    writer.write_bits(first, 64);

    if timestamps.len() == 1 {
        return writer.into_bytes();
    }

    let prev_delta = timestamps[1] - timestamps[0];
    let zigzag_prev = super::zigzag::encode(prev_delta);
    writer.write_bits(zigzag_prev as u64, 64);

    let mut prev_ts = timestamps[1];
    let mut prev_d = prev_delta;

    for &ts in &timestamps[2..] {
        let delta = ts - prev_ts;
        let delta_of_delta = delta - prev_d;

        if delta_of_delta == 0 {
            writer.write_bit(false);
        } else {
            let dod_zigzag = super::zigzag::encode(delta_of_delta);
            match dod_zigzag {
                0..=0xFF => {
                    writer.write_bits(0b10, 2);
                    writer.write_bits(dod_zigzag as u64, 8);
                }
                0x100..=0xFFFF => {
                    writer.write_bits(0b110, 3);
                    writer.write_bits(dod_zigzag as u64, 16);
                }
                0x10000..=0xFFFFFFFF => {
                    writer.write_bits(0b1110, 4);
                    writer.write_bits(dod_zigzag as u64, 32);
                }
                _ => {
                    writer.write_bits(0b1111, 4);
                    writer.write_bits(dod_zigzag as u64, 64);
                }
            }
        }

        prev_ts = ts;
        prev_d = delta;
    }

    writer.into_bytes()
}

pub fn decode_timestamps(data: &[u8], count: usize) -> Vec<i64> {
    if count == 0 || data.is_empty() {
        return Vec::new();
    }

    let mut reader = BitReader::new(data);
    let mut result = Vec::with_capacity(count);

    let first = reader.read_bits(64).unwrap() as i64;
    result.push(first);

    if count == 1 {
        return result;
    }

    let zigzag_prev = reader.read_bits(64).unwrap() as u64;
    let prev_delta = super::zigzag::decode(zigzag_prev);
    let mut prev_ts = first + prev_delta;
    let mut prev_d = prev_delta;
    result.push(prev_ts);

    for _ in 2..count {
        let first_bit = reader.read_bit().unwrap();
        if !first_bit {
            let ts = prev_ts + prev_d;
            result.push(ts);
            prev_ts = ts;
        } else {
            let second_bit = reader.read_bit().unwrap();
            if !second_bit {
                let dod_zigzag = reader.read_bits(8).unwrap();
                let dod = super::zigzag::decode(dod_zigzag);
                let delta = prev_d + dod;
                let ts = prev_ts + delta;
                result.push(ts);
                prev_ts = ts;
                prev_d = delta;
            } else {
                let third_bit = reader.read_bit().unwrap();
                if !third_bit {
                    let dod_zigzag = reader.read_bits(16).unwrap();
                    let dod = super::zigzag::decode(dod_zigzag);
                    let delta = prev_d + dod;
                    let ts = prev_ts + delta;
                    result.push(ts);
                    prev_ts = ts;
                    prev_d = delta;
                } else {
                    let fourth_bit = reader.read_bit().unwrap();
                    if !fourth_bit {
                        let dod_zigzag = reader.read_bits(32).unwrap();
                        let dod = super::zigzag::decode(dod_zigzag);
                        let delta = prev_d + dod;
                        let ts = prev_ts + delta;
                        result.push(ts);
                        prev_ts = ts;
                        prev_d = delta;
                    } else {
                        let dod_zigzag = reader.read_bits(64).unwrap();
                        let dod = super::zigzag::decode(dod_zigzag);
                        let delta = prev_d + dod;
                        let ts = prev_ts + delta;
                        result.push(ts);
                        prev_ts = ts;
                        prev_d = delta;
                    }
                }
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_decode_timestamps() {
        let timestamps: Vec<i64> = vec![1609459200000000000, 1609459201000000000, 1609459202000000000, 1609459203000000000];
        let encoded = encode_timestamps(&timestamps);
        let decoded = decode_timestamps(&encoded, timestamps.len());
        assert_eq!(decoded, timestamps);
    }

    #[test]
    fn test_varying_timestamps() {
        let timestamps: Vec<i64> = vec![1000, 1050, 1120, 1150, 1300, 2000];
        let encoded = encode_timestamps(&timestamps);
        let decoded = decode_timestamps(&encoded, timestamps.len());
        assert_eq!(decoded, timestamps);
    }

    #[test]
    fn test_single_timestamp() {
        let timestamps: Vec<i64> = vec![12345];
        let encoded = encode_timestamps(&timestamps);
        let decoded = decode_timestamps(&encoded, 1);
        assert_eq!(decoded, timestamps);
    }
}
