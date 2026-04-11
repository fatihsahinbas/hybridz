use crate::analyzer;
use crate::entropy::{huffman, ans};

// Format:
//   0x00..0x04 : [pipeline_id: 1B][original_len: 4B LE][lengths: 256B][huffman_payload]
//   0x05       : [pipeline_id: 1B][original_len: 4B LE][ans_payload]
//   (0x05 = BwtMtf + ANS)

pub fn compress(data: &[u8]) -> Result<Vec<u8>, String> {
    if data.is_empty() {
        return Ok(vec![0x00]);
    }

    let analysis = analyzer::analyze(data);
    let (transformed, pipeline_id) = analyzer::apply_pipeline(data, &analysis.pipeline);

    // BwtMtf için ANS ve Huffman'ı yarıştır, küçüğünü seç
    if pipeline_id == 0x04 {
        let ans_out  = encode_with_ans(&transformed);
        let huff_out = encode_with_huffman(&transformed, 0x04);
        return Ok(if ans_out.len() < huff_out.len() { ans_out } else { huff_out });
    }

    Ok(encode_with_huffman(&transformed, pipeline_id))
}

pub fn decompress(data: &[u8]) -> Result<Vec<u8>, String> {
    if data.is_empty() {
        return Err("Boş veri".to_string());
    }

    let pipeline_id = data[0];

    if data.len() == 1 {
        return Ok(Vec::new());
    }

    if pipeline_id == 0x05 {
        // ANS decode: [0x05][original_len: 4B][ans_payload]
        if data.len() < 5 {
            return Err("ANS header eksik".to_string());
        }
        let original_len = u32::from_le_bytes([data[1], data[2], data[3], data[4]]) as usize;
        let ans_payload  = &data[5..];
        let transformed  = ans::decode(ans_payload)
            .ok_or_else(|| "ANS decode hatası".to_string())?;
        if transformed.len() != original_len {
            return Err(format!(
                "ANS uzunluk uyuşmazlığı: beklenen={} alınan={}",
                original_len, transformed.len()
            ));
        }
        return Ok(analyzer::reverse_pipeline(&transformed, 0x04));
    }

    // Huffman decode: [pipeline_id][original_len: 4B][lengths: 256B][payload]
    if data.len() < 1 + 4 + 256 {
        return Err("Huffman header eksik".to_string());
    }
    let original_len = u32::from_le_bytes([data[1], data[2], data[3], data[4]]) as usize;
    let lengths      = &data[5..261];
    let compressed   = &data[261..];

    let table       = huffman::deserialize_table(lengths);
    let transformed = huffman::decode(compressed, &table, original_len);
    let original    = analyzer::reverse_pipeline(&transformed, pipeline_id);

    Ok(original)
}

// ── helpers ──────────────────────────────────────────────────────────────────

fn encode_with_huffman(transformed: &[u8], pipeline_id: u8) -> Vec<u8> {
    let (huff_bytes, table) = huffman::encode(transformed);
    let lengths      = huffman::serialize_table(&table);
    let original_len = transformed.len() as u32;

    let mut out = Vec::with_capacity(1 + 4 + 256 + huff_bytes.len());
    out.push(pipeline_id);
    out.extend_from_slice(&original_len.to_le_bytes());
    out.extend_from_slice(&lengths);
    out.extend_from_slice(&huff_bytes);
    out
}

fn encode_with_ans(transformed: &[u8]) -> Vec<u8> {
    let ans_bytes    = ans::encode(transformed);
    let original_len = transformed.len() as u32;

    let mut out = Vec::with_capacity(1 + 4 + ans_bytes.len());
    out.push(0x05u8);
    out.extend_from_slice(&original_len.to_le_bytes());
    out.extend_from_slice(&ans_bytes);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compress_decompress_text() {
        let original = b"the cat sat on the mat the cat sat on the mat";
        let compressed = compress(original).expect("compress başarısız");
        let recovered = decompress(&compressed).expect("decompress başarısız");
        assert_eq!(original.to_vec(), recovered);
    }

    #[test]
    fn test_compress_decompress_repetitive() {
        let original = vec![0xABu8; 200];
        let compressed = compress(&original).expect("compress başarısız");
        let recovered = decompress(&compressed).expect("decompress başarısız");
        assert_eq!(original, recovered);
    }

    #[test]
    fn test_compress_decompress_binary() {
        let original: Vec<u8> = (0u8..=255).cycle().take(512).collect();
        let compressed = compress(&original).expect("compress başarısız");
        let recovered = decompress(&compressed).expect("decompress başarısız");
        assert_eq!(original, recovered);
    }

    #[test]
    fn test_compress_empty() {
        let original = b"";
        let compressed = compress(original).expect("compress başarısız");
        assert_eq!(compressed.len(), 1);
        let recovered = decompress(&compressed).expect("decompress başarısız");
        assert_eq!(recovered, b"");
    }

    #[test]
    fn test_compression_ratio_text() {
        let original = b"the cat sat on the mat the cat sat on the mat the cat sat on the mat";
        let compressed = compress(original).expect("compress başarısız");
        let recovered = decompress(&compressed).expect("decompress başarısız");
        assert_eq!(original.to_vec(), recovered);
    }

    #[test]
    fn test_large_file_roundtrip() {
        let data = std::fs::read("../corpus/alice29.txt")
            .unwrap_or_else(|_| std::fs::read("corpus/alice29.txt").unwrap());
        let compressed = compress(&data).unwrap();
        let recovered = decompress(&compressed).unwrap();
        assert_eq!(data, recovered);
    }

    #[test]
    fn test_compression_ratio_repetitive() {
        let original = vec![0x42u8; 1024];
        let compressed = compress(&original).expect("compress başarısız");
        assert!(compressed.len() < original.len());
        let recovered = decompress(&compressed).expect("decompress başarısız");
        assert_eq!(original, recovered);
    }

    #[test]
    fn test_bwtmtf_huffman_roundtrip() {
        use crate::transforms::{bwt, mtf};

        let data = std::fs::read("../corpus/alice29.txt")
            .unwrap_or_else(|_| std::fs::read("corpus/alice29.txt").unwrap());

        let bwt_result = bwt::encode(&data);
        let idx = bwt_result.original_index as u32;
        let mut transformed = Vec::new();
        transformed.extend_from_slice(&idx.to_le_bytes());
        transformed.extend(mtf::encode(&bwt_result.transformed));

        let (huff_bytes, table) = crate::entropy::huffman::encode(&transformed);
        let decoded = crate::entropy::huffman::decode(&huff_bytes, &table, transformed.len());

        assert_eq!(transformed, decoded, "Huffman roundtrip bozuk");

        let idx_back = u32::from_le_bytes([decoded[0], decoded[1], decoded[2], decoded[3]]) as usize;
        let mtf_data = &decoded[4..];
        let bwt_data = mtf::decode(mtf_data);
        let recovered = bwt::decode(&bwt_data, idx_back);

        assert_eq!(data, recovered, "BWT/MTF reverse bozuk");
    }

    #[test]
    fn test_kennedy_bwtmtf_ans() {
        use crate::transforms::{bwt, mtf};

        let data = std::fs::read("../corpus/kennedy.xls")
            .unwrap_or_else(|_| std::fs::read("corpus/kennedy.xls").unwrap());

        // Mevcut: sadece Huffman (pipeline=None)
        let current = compress(&data).unwrap();
        eprintln!("Mevcut (Huffman only)  : {} byte ({:.1}% tasarruf)",
            current.len(), (1.0 - current.len() as f64 / data.len() as f64) * 100.0);

        // Deney: BWT+MTF+ANS zorla
        let bwt_result = bwt::encode(&data);
        let idx = bwt_result.original_index as u32;
        let mut transformed = Vec::new();
        transformed.extend_from_slice(&idx.to_le_bytes());
        transformed.extend(mtf::encode(&bwt_result.transformed));

        let ans_out  = crate::entropy::ans::encode(&transformed);
        let huff_out = {
            let (hb, t) = crate::entropy::huffman::encode(&transformed);
            hb.len() + 256 // lengths overhead dahil
        };

        let bwtmtf_ans_total  = 1 + 4 + ans_out.len();
        let bwtmtf_huff_total = 1 + 4 + 256 + huff_out;

        eprintln!("BWT+MTF+ANS            : {} byte ({:.1}% tasarruf)",
            bwtmtf_ans_total, (1.0 - bwtmtf_ans_total as f64 / data.len() as f64) * 100.0);
        eprintln!("BWT+MTF+Huffman        : {} byte ({:.1}% tasarruf)",
            bwtmtf_huff_total, (1.0 - bwtmtf_huff_total as f64 / data.len() as f64) * 100.0);
    }    
}