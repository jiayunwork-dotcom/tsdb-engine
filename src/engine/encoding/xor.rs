use crate::engine::encoding::{BitWriter, BitReader};

pub fn encode_floats(values: &[f64]) -> Vec<u8> {
    if values.is_empty() {
        return Vec::new();
    }

    let mut writer = BitWriter::new();
    let first_bits = values[0].to_bits();
    writer.write_bits(first_bits, 64);

    if values.len() == 1 {
        return writer.into_bytes();
    }

    let mut prev_bits = first_bits;
    let mut prev_leading = 64u8;
    let mut prev_trailing = 64u8;

    for &val in &values[1..] {
        let cur_bits = val.to_bits();
        let xor = prev_bits ^ cur_bits;

        if xor == 0 {
            writer.write_bit(false);
        } else {
            writer.write_bit(true);

            let leading = xor.leading_zeros() as u8;
            let trailing = xor.trailing_zeros() as u8;
            let significant_bits = 64 - leading - trailing;

            if leading >= prev_leading && trailing >= prev_trailing && prev_leading + prev_trailing < 64 {
                writer.write_bit(false);
                writer.write_bits(xor >> prev_trailing, (64 - prev_leading - prev_trailing) as u8);
            } else {
                writer.write_bit(true);
                writer.write_bits(leading as u64, 6);
                let sig_bits = if significant_bits == 0 { 64 } else { significant_bits };
                writer.write_bits((sig_bits - 1) as u64, 6);
                writer.write_bits(xor >> trailing, sig_bits as u8);
                prev_leading = leading;
                prev_trailing = trailing;
            }
        }

        prev_bits = cur_bits;
    }

    writer.into_bytes()
}

pub fn decode_floats(data: &[u8], count: usize) -> Vec<f64> {
    if count == 0 || data.is_empty() {
        return Vec::new();
    }

    let mut reader = BitReader::new(data);
    let mut result = Vec::with_capacity(count);

    let first_bits = reader.read_bits(64).unwrap();
    result.push(f64::from_bits(first_bits));

    let mut prev_bits = first_bits;
    let mut prev_leading: u8 = 64;
    let mut prev_trailing: u8 = 64;

    for _ in 1..count {
        let is_zero = !reader.read_bit().unwrap();
        if is_zero {
            result.push(f64::from_bits(prev_bits));
        } else {
            let reuse_prev = !reader.read_bit().unwrap();
            if reuse_prev {
                let significant_bits = 64 - prev_leading - prev_trailing;
                let significant = reader.read_bits(significant_bits as u8).unwrap();
                let xor = significant << prev_trailing;
                let cur_bits = prev_bits ^ xor;
                result.push(f64::from_bits(cur_bits));
                prev_bits = cur_bits;
            } else {
                let leading = reader.read_bits(6).unwrap() as u8;
                let sig_bits = reader.read_bits(6).unwrap() as u8 + 1;
                let trailing = 64 - leading - sig_bits;
                let significant = reader.read_bits(sig_bits as u8).unwrap();
                let xor = significant << trailing;
                let cur_bits = prev_bits ^ xor;
                result.push(f64::from_bits(cur_bits));
                prev_bits = cur_bits;
                prev_leading = leading;
                prev_trailing = trailing;
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_decode_floats() {
        let values = vec![1.0, 1.0, 1.001, 1.002, 1.001, 1.0];
        let encoded = encode_floats(&values);
        let decoded = decode_floats(&encoded, values.len());
        for (a, b) in values.iter().zip(decoded.iter()) {
            assert!((a - b).abs() < 1e-10);
        }
    }

    #[test]
    fn test_constant_floats() {
        let values = vec![3.14, 3.14, 3.14, 3.14];
        let encoded = encode_floats(&values);
        let decoded = decode_floats(&encoded, values.len());
        assert_eq!(decoded, values);
    }

    #[test]
    fn test_single_float() {
        let values = vec![42.0];
        let encoded = encode_floats(&values);
        let decoded = decode_floats(&encoded, 1);
        assert_eq!(decoded, values);
    }
}
