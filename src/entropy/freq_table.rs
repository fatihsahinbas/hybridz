pub const TABLE_SIZE: usize = 4096;

#[derive(Debug, Clone)]
pub struct FreqTable {
    pub raw_counts: [u64; 256],
    pub normalized: [u32; 256],
    pub symbol_count: usize,
}

impl FreqTable {
    pub fn from_data(data: &[u8]) -> Self {
        let mut raw_counts = [0u64; 256];
        for &b in data {
            raw_counts[b as usize] += 1;
        }
        FreqTable::from_raw(raw_counts)
    }

    pub fn from_raw(counts: [u64; 256]) -> Self {
        let mut ft = FreqTable {
            raw_counts: counts,
            normalized: [0u32; 256],
            symbol_count: 0,
        };
        ft.normalize();
        ft
    }

    fn normalize(&mut self) {
        let total: u64 = self.raw_counts.iter().sum();
        if total == 0 {
            self.symbol_count = 0;
            return;
        }
        let active: Vec<usize> = (0..256).filter(|&i| self.raw_counts[i] > 0).collect();
        self.symbol_count = active.len();

        if active.len() == 1 {
            self.normalized[active[0]] = TABLE_SIZE as u32;
            return;
        }

        let mut assigned = [0u32; 256];
        for &sym in &active {
            let proportion = (self.raw_counts[sym] as f64 / total as f64 * TABLE_SIZE as f64) as u32;
            assigned[sym] = proportion.max(1);
        }

        let sum: u32 = assigned.iter().sum();
        let diff = TABLE_SIZE as i64 - sum as i64;
        if diff != 0 {
            let biggest = active.iter().max_by_key(|&&i| self.raw_counts[i]).copied().unwrap();
            assigned[biggest] = (assigned[biggest] as i64 + diff) as u32;
        }
        self.normalized = assigned;
    }

    pub fn serialize(&self) -> Vec<u8> {
        let active: Vec<usize> = (0..256).filter(|&i| self.raw_counts[i] > 0).collect();
        let mut out = Vec::new();
        out.extend_from_slice(&(active.len() as u16).to_le_bytes());
        for sym in active {
            out.push(sym as u8);
            let c = self.raw_counts[sym].min(u32::MAX as u64) as u32;
            out.extend_from_slice(&c.to_le_bytes());
        }
        out
    }

    pub fn deserialize(data: &[u8]) -> Option<(Self, usize)> {
        if data.len() < 2 { return None; }
        let count = u16::from_le_bytes([data[0], data[1]]) as usize;
        let needed = 2 + count * 5;
        if data.len() < needed { return None; }
        let mut raw_counts = [0u64; 256];
        for i in 0..count {
            let off = 2 + i * 5;
            let sym = data[off] as usize;
            let c = u32::from_le_bytes([data[off+1], data[off+2], data[off+3], data[off+4]]) as u64;
            raw_counts[sym] = c;
        }
        Some((FreqTable::from_raw(raw_counts), needed))
    }

    pub fn is_valid_for_ans(&self) -> bool {
        let sum: u32 = self.normalized.iter().sum();
        sum == TABLE_SIZE as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_data_basic() {
        let data = b"aabbcc";
        let ft = FreqTable::from_data(data);
        assert_eq!(ft.raw_counts[b'a' as usize], 2);
        assert_eq!(ft.raw_counts[b'b' as usize], 2);
        assert_eq!(ft.raw_counts[b'c' as usize], 2);
        assert_eq!(ft.symbol_count, 3);
    }

    #[test]
    fn test_normalize_sums_to_table_size() {
        let data: Vec<u8> = (0u8..=255).collect();
        let ft = FreqTable::from_data(&data);
        let sum: u32 = ft.normalized.iter().sum();
        assert_eq!(sum, TABLE_SIZE as u32);
    }

    #[test]
    fn test_normalize_all_active_min_one() {
        let data = b"abc";
        let ft = FreqTable::from_data(data);
        for &b in b"abc" {
            assert!(ft.normalized[b as usize] >= 1);
        }
    }

    #[test]
    fn test_serialize_deserialize_roundtrip() {
        let data = b"hello world this is a test";
        let ft = FreqTable::from_data(data);
        let serialized = ft.serialize();
        let (ft2, consumed) = FreqTable::deserialize(&serialized).unwrap();
        assert_eq!(consumed, serialized.len());
        for i in 0..256 {
            assert_eq!(ft.raw_counts[i], ft2.raw_counts[i]);
        }
    }

    #[test]
    fn test_is_valid_for_ans() {
        let data: Vec<u8> = (0u8..=255).collect();
        let ft = FreqTable::from_data(&data);
        assert!(ft.is_valid_for_ans());
    }
}