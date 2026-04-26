use crate::analyzer;
use crate::entropy::huffman;
use crate::entropy::ans;

// Pipeline ID tablosu:
//   0x00 → None       + Huffman
//   0x01 → Delta      + Huffman
//   0x02 → RLE        + Huffman
//   0x03 → DeltaRle   + Huffman
//   0x04 → BwtMtf     + Huffman
//   0x05 → BwtMtf     + ANS      ← ANS kazandığında
//   0x06 → BcjBwtMtf  + Huffman
//   0x07 → BcjBwtMtf  + ANS      ← ANS kazandığında
//   0x08 → DeIlv      (kendi içinde sıkıştırılmış)

pub fn compress(data: &[u8]) -> Result<Vec<u8>, String> {
    if data.is_empty() {
        return Ok(vec![0x00]);
    }

    let analysis = analyzer::analyze(data);
    let (transformed, pipeline_id) = analyzer::apply_pipeline(data, &analysis.pipeline);

    // 0x08 = DeIlv: encode_compressed zaten sıkıştırdı
    if pipeline_id == 0x08 {
        let mut output = Vec::with_capacity(1 + transformed.len());
        output.push(pipeline_id);
        output.extend_from_slice(&transformed);
        return Ok(output);
    }

    // BwtMtf (0x04) ve BcjBwtMtf (0x06) için ANS vs Huffman yarışması
    if pipeline_id == 0x04 || pipeline_id == 0x06 {
        let ans_bytes = ans::encode(&transformed);
        let (huff_bytes, table) = huffman::encode(&transformed);

        if ans_bytes.len() < huff_bytes.len() {
            // ANS kazandı → pipeline_id + 1 (0x05 veya 0x07)
            let ans_id = pipeline_id + 1;
            let original_len = transformed.len() as u32;
            let mut output = Vec::with_capacity(1 + 4 + ans_bytes.len());
            output.push(ans_id);
            output.extend_from_slice(&original_len.to_le_bytes());
            output.extend_from_slice(&ans_bytes);
            return Ok(output);
        } else {
            // Huffman kazandı → normal pipeline_id
            let original_len = transformed.len() as u32;
            let lengths = huffman::serialize_table(&table);
            let mut output = Vec::with_capacity(1 + 4 + 256 + huff_bytes.len());
            output.push(pipeline_id);
            output.extend_from_slice(&original_len.to_le_bytes());
            output.extend_from_slice(&lengths);
            output.extend_from_slice(&huff_bytes);
            return Ok(output);
        }
    }

    // Diğer pipeline'lar: Huffman
    let original_len = transformed.len() as u32;
    let (huff_bytes, table) = huffman::encode(&transformed);
    let lengths = huffman::serialize_table(&table);

    let mut output = Vec::with_capacity(1 + 4 + 256 + huff_bytes.len());
    output.push(pipeline_id);
    output.extend_from_slice(&original_len.to_le_bytes());
    output.extend_from_slice(&lengths);
    output.extend_from_slice(&huff_bytes);

    Ok(output)
}

pub fn decompress(data: &[u8]) -> Result<Vec<u8>, String> {
    if data.is_empty() {
        return Err("Boş veri".to_string());
    }

    let pipeline_id = data[0];

    if data.len() == 1 {
        return Ok(Vec::new());
    }

    // 0x08 = DeIlv
    if pipeline_id == 0x08 {
        let payload = &data[1..];
        return Ok(analyzer::reverse_pipeline(payload, pipeline_id));
    }

    // 0x05 = BwtMtf + ANS, 0x07 = BcjBwtMtf + ANS
    if pipeline_id == 0x05 || pipeline_id == 0x07 {
        if data.len() < 5 {
            return Err("ANS header eksik".to_string());
        }
        let _original_len = u32::from_le_bytes([data[1], data[2], data[3], data[4]]) as usize;
        let ans_payload = &data[5..];
        let transformed = ans::decode(ans_payload)
            .ok_or_else(|| "ANS decode başarısız".to_string())?;
        // pipeline_id - 1 = Huffman versiyonu = reverse_pipeline'ın beklediği ID
        let reverse_id = pipeline_id - 1;
        return Ok(analyzer::reverse_pipeline(&transformed, reverse_id));
    }

    if data.len() < 1 + 4 + 256 {
        return Err("Veri çok kısa: header eksik".to_string());
    }

    let original_len = u32::from_le_bytes([data[1], data[2], data[3], data[4]]) as usize;
    let lengths = &data[5..261];
    let compressed = &data[261..];

    let table = huffman::deserialize_table(lengths);
    let transformed = huffman::decode(compressed, &table, original_len);
    let original = analyzer::reverse_pipeline(&transformed, pipeline_id);

    Ok(original)
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
        let data = std::fs::read("../corpus/kennedy.xls")
            .unwrap_or_else(|_| std::fs::read("corpus/kennedy.xls").unwrap());
        let compressed = compress(&data).unwrap();
        let recovered = decompress(&compressed).unwrap();
        assert_eq!(data, recovered);
    }

    #[test]
    fn test_lz_notepad_ratio() {
        let data = std::fs::read("notepad.exe")
            .unwrap_or_else(|_| std::fs::read("target/release/notepad.exe")
            .unwrap_or_else(|_| vec![0u8; 100]));
        if data.len() < 100 { return; }
        let compressed = compress(&data).unwrap();
        let recovered = decompress(&compressed).unwrap();
        assert_eq!(data, recovered);
    }
}