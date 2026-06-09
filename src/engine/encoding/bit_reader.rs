pub struct BitReader<'a> {
    buffer: &'a [u8],
    byte_pos: usize,
    bit_pos: u8,
}

impl<'a> BitReader<'a> {
    pub fn new(buffer: &'a [u8]) -> Self {
        Self {
            buffer,
            byte_pos: 0,
            bit_pos: 0,
        }
    }

    pub fn read_bit(&mut self) -> Option<bool> {
        if self.byte_pos >= self.buffer.len() {
            return None;
        }
        let bit = (self.buffer[self.byte_pos] >> (7 - self.bit_pos)) & 1 == 1;
        self.bit_pos += 1;
        if self.bit_pos == 8 {
            self.byte_pos += 1;
            self.bit_pos = 0;
        }
        Some(bit)
    }

    pub fn read_bits(&mut self, num_bits: u8) -> Option<u64> {
        let mut value: u64 = 0;
        for _ in 0..num_bits {
            value = (value << 1) | (self.read_bit()? as u64);
        }
        Some(value)
    }

    pub fn has_more(&self) -> bool {
        self.byte_pos < self.buffer.len()
    }
}
