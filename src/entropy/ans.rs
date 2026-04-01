use crate::entropy::freq_table::{FreqTable, TABLE_SIZE};

const L: u64 = TABLE_SIZE as u64;
const B: u64 = 256;

#[derive(Clone)]
pub struct AnsTable {
    pub spread:  Vec<u8>,
    pub cumfreq: [u32; 257],
    pub freq:    [u32; 256],
}

impl AnsTable {
    pub fn build(ft: &FreqTable) -> Self {
        let freq = ft.normalized;
        let mut cumfreq = [0u32; 257];
        let mut acc = 0u32;
        for i in 0..256 {
            cumfreq[i] = acc;
            acc += freq[i];
        }
        cumfreq[256] = acc;

        let mut spread = vec![0u8; TABLE_SIZE];
        for s in 0usize..256 {
            let f = freq[s] as usize;
            if f == 0 { continue; }
            let start = cumfreq[s] as usize;
            for i in 0..f {
                spread[start + i] = s as u8;
            }
        }
        AnsTable { spread, cumfreq, freq }
    }

    #[inline]
    pub fn find_symbol(&self, slot: usize) -> u8 {
        self.spread[slot]
    }
}

pub fn encode(data: &[u8]) -> Vec<u8> {
    if data.is_empty() {
        let ft = FreqTable::from_data(&[0u8]);
        let mut out = ft.serialize();
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&L.to_le_bytes());
        return out;
    }

    let ft = FreqTable::from_data(data);
    let freq_bytes = ft.serialize();
    let table = AnsTable::build(&ft);

    let active_count = ft.normalized.iter().filter(|&&f| f > 0).count();
    if active_count == 1 {
        let mut out = freq_bytes;
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&L.to_le_bytes());
        return out;
    }

    let mut state: u64 = L;
    let mut norm_bytes: Vec<u8> = Vec::new();

    for &sym in data.iter().rev() {
        let freq  = table.freq[sym as usize] as u64;
        let cumul = table.cumfreq[sym as usize] as u64;

        // Ryg canonical: normalize state into [freq, freq*B)
        while state >= freq * B {
            norm_bytes.push((state % B) as u8);
            state /= B;
        }

        state = (state / freq) * L + cumul + (state % freq);
    }

    let mut out = Vec::new();
    out.extend_from_slice(&freq_bytes);
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(&state.to_le_bytes());
    out.extend_from_slice(&norm_bytes);
    out
}

pub fn decode(data: &[u8]) -> Option<Vec<u8>> {
    if data.len() < 2 { return Some(Vec::new()); }

    let (ft, consumed) = FreqTable::deserialize(data)?;

    if data.len() < consumed + 4 { return Some(Vec::new()); }
    let orig_len = u32::from_le_bytes(
        data[consumed..consumed+4].try_into().ok()?
    ) as usize;

    if orig_len == 0 { return Some(Vec::new()); }

    if data.len() < consumed + 12 { return None; }
    let final_state = u64::from_le_bytes(
        data[consumed+4..consumed+12].try_into().ok()?
    );

    let active: Vec<u8> = (0u8..=255)
        .filter(|&b| ft.normalized[b as usize] > 0)
        .collect();
    if active.len() == 1 {
        return Some(vec![active[0]; orig_len]);
    }

    let norm_bytes = &data[consumed + 12..];
    let table = AnsTable::build(&ft);

    let mut state = final_state;
    let mut norm_pos = norm_bytes.len(); // tersten oku

    let mut result = Vec::with_capacity(orig_len);

    for _ in 0..orig_len {
        let slot  = (state % L) as usize;
        let sym   = table.find_symbol(slot);
        let freq  = table.freq[sym as usize] as u64;
        let cumul = table.cumfreq[sym as usize] as u64;

        state = freq * (state / L) + slot as u64 - cumul;

        // Renorm: state'i [L, L*B) aralığına çek
        while state < L {
            if norm_pos == 0 { break; }
            norm_pos -= 1;
            state = state * B + norm_bytes[norm_pos] as u64;
        }

        result.push(sym);
    }

    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ans_roundtrip_simple() {
        let data = b"aabb";
        let encoded = encode(data);
        let decoded = decode(&encoded).unwrap();
        assert_eq!(data.to_vec(), decoded);
    }

    #[test]
    fn test_ans_roundtrip_text() {
        let data = b"the quick brown fox jumps over the lazy dog";
        let encoded = encode(data);
        let decoded = decode(&encoded).unwrap();
        assert_eq!(data.to_vec(), decoded);
    }

    #[test]
    fn test_ans_roundtrip_repetitive() {
        let data = vec![0xAAu8; 200];
        let encoded = encode(&data);
        let decoded = decode(&encoded).unwrap();
        assert_eq!(data, decoded);
    }

    #[test]
    fn test_ans_roundtrip_binary() {
        let data: Vec<u8> = (0u8..=255).cycle().take(512).collect();
        let encoded = encode(&data);
        let decoded = decode(&encoded).unwrap();
        assert_eq!(data, decoded);
    }

    #[test]
    fn test_ans_empty() {
        let data = b"";
        let encoded = encode(data);
        let decoded = decode(&encoded).unwrap();
        assert_eq!(decoded, b"");
    }

    #[test]
    fn test_ans_single_symbol() {
        let data = vec![0x42u8; 100];
        let encoded = encode(&data);
        let decoded = decode(&encoded).unwrap();
        assert_eq!(data, decoded);
    }
}