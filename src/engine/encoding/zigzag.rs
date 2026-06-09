pub fn encode(n: i64) -> u64 {
    ((n << 1) ^ (n >> 63)) as u64
}

pub fn decode(n: u64) -> i64 {
    ((n >> 1) as i64) ^ -((n & 1) as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zigzag() {
        assert_eq!(encode(0), 0);
        assert_eq!(encode(-1), 1);
        assert_eq!(encode(1), 2);
        assert_eq!(encode(-2), 3);
        assert_eq!(encode(2147483647), 4294967294);
        assert_eq!(encode(-2147483648), 4294967295);

        for v in [-1000i64, -1, 0, 1, 1000, i64::MAX, i64::MIN] {
            assert_eq!(decode(encode(v)), v);
        }
    }
}
