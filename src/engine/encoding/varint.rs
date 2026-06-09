pub fn encode_varint(value: u64, buf: &mut Vec<u8>) {
    let mut v = value;
    loop {
        let mut byte = (v & 0x7F) as u8;
        v >>= 7;
        if v != 0 {
            byte |= 0x80;
        }
        buf.push(byte);
        if v == 0 {
            break;
        }
    }
}

pub fn decode_varint(data: &[u8]) -> Option<(u64, usize)> {
    let mut result: u64 = 0;
    let mut shift: u32 = 0;
    for (i, &byte) in data.iter().enumerate() {
        result |= ((byte & 0x7F) as u64) << shift;
        if byte & 0x80 == 0 {
            return Some((result, i + 1));
        }
        shift += 7;
        if shift >= 64 {
            return None;
        }
    }
    None
}

pub fn encode_signed_varint(value: i64, buf: &mut Vec<u8>) {
    let zigzag = super::zigzag::encode(value);
    encode_varint(zigzag, buf);
}

pub fn decode_signed_varint(data: &[u8]) -> Option<(i64, usize)> {
    let (zigzag, len) = decode_varint(data)?;
    Some((super::zigzag::decode(zigzag), len))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_varint_roundtrip() {
        let values = vec![0, 1, 127, 128, 255, 256, 16383, 16384, u64::MAX];
        for v in values {
            let mut buf = Vec::new();
            encode_varint(v, &mut buf);
            let (decoded, len) = decode_varint(&buf).unwrap();
            assert_eq!(decoded, v);
            assert_eq!(len, buf.len());
        }
    }

    #[test]
    fn test_signed_varint_roundtrip() {
        let values = vec![0i64, -1, 1, -128, 127, i64::MAX, i64::MIN];
        for v in values {
            let mut buf = Vec::new();
            encode_signed_varint(v, &mut buf);
            let (decoded, len) = decode_signed_varint(&buf).unwrap();
            assert_eq!(decoded, v);
            assert_eq!(len, buf.len());
        }
    }
}
