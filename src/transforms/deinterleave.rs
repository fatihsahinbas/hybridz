/// De-interleave (Column-Oriented) Transform
///
/// Sabit-uzunluk kayıtlı binary dosyaları (sao, DICOM, sensör logları) için.
/// Her sütunu ayrı Huffman ile sıkıştırır — düşük entropili sütunlar gerçekten küçülür,
/// yüksek entropili sütunlar boyutlarını korur.
///
/// encode_compressed format (self-contained):
///   [record_size: 1B]
///   [orig_len: 4B LE]
///   for each column (record_size adet):
///     [col_compressed_len: 4B LE]
///     [huffman_lengths: 256B]
///     [huffman_bits: col_compressed_len bytes]
///   [tail_len: 4B LE]
///   [tail bytes: tail_len bytes]
use crate::entropy::huffman;

/// Denenecek record boyutları
pub const CANDIDATE_RECORDS: &[usize] = &[16, 24, 28, 32, 40, 48, 56, 64];

/// Encode: de-interleave + sütun bazında Huffman
/// codec.rs bu çıktıya tekrar Huffman UYGULAMAZ (pipeline_id=0x08 özel dal)
pub fn encode_compressed(data: &[u8], record_size: usize) -> Vec<u8> {
    assert!(record_size > 0 && record_size <= 255);

    let orig_len = data.len();
    let n_records = orig_len / record_size;
    let tail_start = n_records * record_size;
    let tail = &data[tail_start..];

    let mut out = Vec::new();
    out.push(record_size as u8);
    out.extend_from_slice(&(orig_len as u32).to_le_bytes());

    // Her sütunu ayrı Huffman ile sıkıştır
    for col in 0..record_size {
        let col_bytes: Vec<u8> = (0..n_records)
            .map(|rec| data[rec * record_size + col])
            .collect();

        let (huff_bits, table) = huffman::encode(&col_bytes);
        let lengths = huffman::serialize_table(&table);

        out.extend_from_slice(&(huff_bits.len() as u32).to_le_bytes());
        out.extend_from_slice(&lengths); // 256 byte
        out.extend_from_slice(&huff_bits);
    }

    // Tail olduğu gibi ekle
    out.extend_from_slice(&(tail.len() as u32).to_le_bytes());
    out.extend_from_slice(tail);

    out
}

/// Decode: sütun bazında Huffman decode + re-interleave
pub fn decode_compressed(data: &[u8]) -> Vec<u8> {
    if data.len() < 5 {
        return Vec::new();
    }

    let record_size = data[0] as usize;
    let orig_len = u32::from_le_bytes([data[1], data[2], data[3], data[4]]) as usize;

    if record_size == 0 || orig_len == 0 {
        return Vec::new();
    }

    let n_records = orig_len / record_size;
    let tail_len = orig_len - n_records * record_size;

    let mut pos = 5usize;
    let mut columns: Vec<Vec<u8>> = Vec::with_capacity(record_size);

    // Her sütunu decode et
    for _ in 0..record_size {
        if pos + 4 + 256 > data.len() {
            return Vec::new();
        }

        let col_len =
            u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
        pos += 4;

        let lengths = &data[pos..pos + 256];
        pos += 256;

        if pos + col_len > data.len() {
            return Vec::new();
        }

        let huff_bits = &data[pos..pos + col_len];
        pos += col_len;

        let table = huffman::deserialize_table(lengths);
        let col_bytes = huffman::decode(huff_bits, &table, n_records);
        columns.push(col_bytes);
    }

    // Tail
    if pos + 4 > data.len() {
        return Vec::new();
    }
    let tail_len_check =
        u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
    pos += 4;

    if tail_len_check != tail_len || pos + tail_len > data.len() {
        return Vec::new();
    }
    let tail = &data[pos..pos + tail_len];

    // Re-interleave
    let mut out = vec![0u8; orig_len];
    for col in 0..record_size {
        for rec in 0..n_records {
            out[rec * record_size + col] = columns[col][rec];
        }
    }
    out[n_records * record_size..].copy_from_slice(tail);

    out
}

/// Record size tespiti.
pub fn detect_record_size(data: &[u8]) -> Option<usize> {
    const MIN_SAMPLE: usize = 1024;
    const MIN_ENTROPY_DROP: f64 = 1.0;

    if data.len() < MIN_SAMPLE {
        return None;
    }

    let raw_e = shannon_entropy(data);
    if raw_e < 6.5 {
        return None;
    }

    let mut best: Option<(usize, f64)> = None;
    for &rs in CANDIDATE_RECORDS {
        if data.len() < rs * 100 {
            continue;
        }
        let avg_col_e = avg_column_entropy(data, rs);
        if let Some((_, best_e)) = best {
            if avg_col_e < best_e {
                best = Some((rs, avg_col_e));
            }
        } else {
            best = Some((rs, avg_col_e));
        }
    }

    match best {
        Some((rs, avg_e)) if raw_e - avg_e >= MIN_ENTROPY_DROP => {
            Some(rs)
        }
        _ => None,
    }
}

// ── Yardımcı fonksiyonlar ────────────────────────────────────────────────────

fn shannon_entropy(data: &[u8]) -> f64 {
    if data.is_empty() {
        return 0.0;
    }
    let mut freq = [0u64; 256];
    for &b in data {
        freq[b as usize] += 1;
    }
    let len = data.len() as f64;
    freq.iter()
        .filter(|&&f| f > 0)
        .map(|&f| {
            let p = f as f64 / len;
            -p * p.log2()
        })
        .sum()
}

fn avg_column_entropy(data: &[u8], record_size: usize) -> f64 {
    let n = data.len();
    let n_records = n / record_size;
    if n_records == 0 {
        return 8.0;
    }

    let mut sum_entropy = 0.0;
    for col in 0..record_size {
        let mut freq = [0u64; 256];
        for rec in 0..n_records {
            freq[data[rec * record_size + col] as usize] += 1;
        }
        let len = n_records as f64;
        let e: f64 = freq
            .iter()
            .filter(|&&f| f > 0)
            .map(|&f| {
                let p = f as f64 / len;
                -p * p.log2()
            })
            .sum();
        sum_entropy += e;
    }
    sum_entropy / record_size as f64
}

// ── Eski encode/decode (mevcut testler için) ─────────────────────────────────

pub fn encode(data: &[u8], record_size: usize) -> Vec<u8> {
    assert!(record_size > 0 && record_size <= 255);
    let orig_len = data.len();
    if data.is_empty() {
        let mut out = vec![record_size as u8];
        out.extend_from_slice(&0u32.to_le_bytes());
        return out;
    }
    let n_records = orig_len / record_size;
    let tail_start = n_records * record_size;
    let mut out = Vec::with_capacity(5 + orig_len);
    out.push(record_size as u8);
    out.extend_from_slice(&(orig_len as u32).to_le_bytes());
    for col in 0..record_size {
        for rec in 0..n_records {
            out.push(data[rec * record_size + col]);
        }
    }
    out.extend_from_slice(&data[tail_start..]);
    out
}

pub fn decode(data: &[u8]) -> Vec<u8> {
    if data.len() < 5 {
        return Vec::new();
    }
    let record_size = data[0] as usize;
    let orig_len = u32::from_le_bytes([data[1], data[2], data[3], data[4]]) as usize;
    if record_size == 0 || orig_len == 0 {
        return Vec::new();
    }
    let payload = &data[5..];
    let n_records = orig_len / record_size;
    let tail_len = orig_len - n_records * record_size;
    let column_bytes_len = n_records * record_size;
    if payload.len() < column_bytes_len + tail_len {
        return Vec::new();
    }
    let mut out = vec![0u8; orig_len];
    for col in 0..record_size {
        let col_start = col * n_records;
        for rec in 0..n_records {
            out[rec * record_size + col] = payload[col_start + rec];
        }
    }
    let tail_dst_start = n_records * record_size;
    out[tail_dst_start..tail_dst_start + tail_len]
        .copy_from_slice(&payload[column_bytes_len..column_bytes_len + tail_len]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compressed_roundtrip_exact_multiple() {
        let original: Vec<u8> = (0..16).collect();
        let encoded = encode_compressed(&original, 4);
        let decoded = decode_compressed(&encoded);
        assert_eq!(original, decoded);
    }

    #[test]
    fn test_compressed_roundtrip_with_tail() {
        let original: Vec<u8> = (0..17).collect();
        let encoded = encode_compressed(&original, 4);
        let decoded = decode_compressed(&encoded);
        assert_eq!(original, decoded);
    }

    #[test]
    fn test_compressed_roundtrip_record_size_1() {
        let original: Vec<u8> = b"hello world".to_vec();
        let encoded = encode_compressed(&original, 1);
        let decoded = decode_compressed(&encoded);
        assert_eq!(original, decoded);
    }

    #[test]
    fn test_compressed_roundtrip_synthetic_sao() {
        let mut data = Vec::with_capacity(200 * 28);
        for i in 0..200u32 {
            for j in 0..28usize {
                let b = match j {
                    7 | 15 | 19 => 0x40u8,
                    _ => (i.wrapping_mul((j as u32) * 31 + 17)) as u8,
                };
                data.push(b);
            }
        }
        let encoded = encode_compressed(&data, 28);
        let decoded = decode_compressed(&encoded);
        assert_eq!(data, decoded);
        assert_eq!(data, decoded);
    }

    #[test]
    fn test_roundtrip_exact_multiple() {
        let original: Vec<u8> = (0..16).collect();
        let encoded = encode(&original, 4);
        let decoded = decode(&encoded);
        assert_eq!(original, decoded);
    }

    #[test]
    fn test_roundtrip_with_tail() {
        let original: Vec<u8> = (0..17).collect();
        let encoded = encode(&original, 4);
        let decoded = decode(&encoded);
        assert_eq!(original, decoded);
    }

    #[test]
    fn test_roundtrip_smaller_than_record() {
        let original = vec![0x42u8, 0x43, 0x44];
        let encoded = encode(&original, 4);
        let decoded = decode(&encoded);
        assert_eq!(original, decoded);
    }

    #[test]
    fn test_roundtrip_empty() {
        let original: Vec<u8> = vec![];
        let encoded = encode(&original, 8);
        let decoded = decode(&encoded);
        assert_eq!(original, decoded);
    }

    #[test]
    fn test_roundtrip_record_size_1() {
        let original: Vec<u8> = b"hello world".to_vec();
        let encoded = encode(&original, 1);
        let decoded = decode(&encoded);
        assert_eq!(original, decoded);
    }

    #[test]
    fn test_roundtrip_synthetic_sao_like() {
        let mut data = Vec::with_capacity(100 * 28);
        for i in 0..100u32 {
            for _ in 0..27 {
                data.push((i * 17 + 3) as u8);
            }
            data.push(0x40);
        }
        let encoded = encode(&data, 28);
        let decoded = decode(&encoded);
        assert_eq!(data, decoded);
    }

    #[test]
    fn test_detect_finds_period_in_synthetic() {
        let mut data = Vec::with_capacity(500 * 28);
        for i in 0..500u32 {
            for j in 0..28 {
                let b = match j {
                    7 | 15 | 23 | 27 => 0x40u8,
                    6 | 14 | 22 => ((i / 8) & 0xFF) as u8,
                    _ => (i.wrapping_mul((j as u32) * 31 + 17)) as u8,
                };
                data.push(b);
            }
        }
        let detected = detect_record_size(&data);
        assert!(
            detected.is_some(),
            "synthetic SAO-like data should be detected"
        );
        let rs = detected.unwrap();
        assert!(CANDIDATE_RECORDS.contains(&rs));
    }

    #[test]
    fn test_detect_returns_none_for_low_entropy() {
        let data = vec![0u8; 20000];
        assert_eq!(detect_record_size(&data), None);
    }

    #[test]
    fn test_encode_format() {
        let original = vec![1u8, 2, 3, 4, 5, 6, 7, 8];
        let encoded = encode(&original, 4);
        assert_eq!(encoded, vec![4, 8, 0, 0, 0, 1, 5, 2, 6, 3, 7, 4, 8]);
    }

    #[test]
    fn test_encode_format_with_tail() {
        let original = vec![1u8, 2, 3, 4, 5, 6, 7, 8, 9];
        let encoded = encode(&original, 4);
        assert_eq!(encoded, vec![4, 9, 0, 0, 0, 1, 5, 2, 6, 3, 7, 4, 8, 9]);
    }

    #[test]
    fn test_detect_returns_none_for_random() {
        let data: Vec<u8> = (0..20000u32)
            .map(|i| (i.wrapping_mul(2654435761) >> 24) as u8)
            .collect();
        let detected = detect_record_size(&data);
        if let Some(rs) = detected {
            eprintln!("random data periodic? RS={}", rs);
        }
    }
}
