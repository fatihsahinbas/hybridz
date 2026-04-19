/// Content Analyzer
///
/// Bir veri bloğunu hızlıca analiz ederek hangi transform pipeline'ının
/// en iyi sonucu vereceğine karar verir.
///
/// Analoji: Bir şef malzemeye bakıp "bunu haşlayalım mı, kızartalım mı?"
/// diye karar verir. Biz de veriye bakıp "BWT mi, Delta mı?" deriz.

use crate::transforms::{delta, rle, bwt, mtf,bcj};

/// Veri tipi tahmini
#[derive(Debug, Clone, PartialEq)]
pub enum ContentType {
    Text,           // UTF-8 metin, log, XML, JSON
    Numeric,        // Sayısal seriler, sensör verisi, fiyat/stok
    Binary,         // Genel binary, EXE, ZIP içeriği
    Repetitive,     // Çok tekrarlı (null buffer, sabit değer)
    Unknown,
}

/// Seçilen transform pipeline
#[derive(Debug, Clone)]
pub enum TransformPipeline {
    BwtMtf,         // Metin için: BWT → MTF
    BcjBwtMtf,      // EXE/PE için: BCJ → BWT → MTF
    Delta,          // Sayısal için: Delta
    Rle,            // Çok tekrarlı için: RLE
    DeltaRle,       // Sayısal + tekrarlı: Delta → RLE
    None,           // Transform faydasız, direkt LZ'ye gönder
}

/// Analiz sonucu
pub struct AnalysisResult {
    pub content_type: ContentType,
    pub pipeline: TransformPipeline,
    pub entropy: f64,           // 0.0 (düşük) → 8.0 (maksimum)
    pub compressibility: f64,   // 0.0 (sıkışmaz) → 1.0 (çok sıkışır)
}

/// Veri bloğunu analiz et ve transform kararı ver
pub fn analyze(data: &[u8]) -> AnalysisResult {
    if data.is_empty() {
        return AnalysisResult {
            content_type: ContentType::Unknown,
            pipeline: TransformPipeline::None,
            entropy: 0.0,
            compressibility: 0.0,
        };
    }

    let entropy = calculate_entropy(data);
    let delta_score = delta::suitability_score(data);
    let rle_score = rle::suitability_score(data);
    let bwt_score = bwt::suitability_score(data);
    let text_ratio = text_byte_ratio(data);

    // İçerik tipi tahmini
    let content_type = if rle_score > 0.7 {
        ContentType::Repetitive
    } else if text_ratio > 0.85 {
        ContentType::Text
    } else if delta_score > 0.5 {
        ContentType::Numeric
    } else {
        ContentType::Binary
    };

    // Pipeline kararı
    let pipeline = match &content_type {
        ContentType::Repetitive => TransformPipeline::Rle,
        ContentType::Text => TransformPipeline::BwtMtf,
        ContentType::Numeric => {
            if rle_score > 0.3 {
                TransformPipeline::DeltaRle
            } else {
                TransformPipeline::Delta
            }
        }
        ContentType::Binary | ContentType::Unknown => {
            if entropy > 7.5 {
                TransformPipeline::None
            } else {
                let bcj_score = bcj::suitability_score(data);
                if bcj_score > 0.05 {
                    TransformPipeline::BcjBwtMtf
                } else {
                    TransformPipeline::BwtMtf
                }
            }
        }
    };

    // ← DEBUG SATIRI — buraya ekle

    let compressibility = 1.0 - (entropy / 8.0);

    AnalysisResult {
        content_type,
        pipeline,
        entropy,
        compressibility,
    }
}

/// Seçilen pipeline'ı veriye uygula
/// Döndürür: (transformed_data, pipeline_id_byte)
/// pipeline_id_byte header'a yazılır, decode'da ne yapılacağını söyler
pub fn apply_pipeline(data: &[u8], pipeline: &TransformPipeline) -> (Vec<u8>, u8) {
    match pipeline {
        TransformPipeline::None => (data.to_vec(), 0x00),

        TransformPipeline::Delta => (delta::encode(data), 0x01),

        TransformPipeline::Rle => (rle::encode(data), 0x02),

        TransformPipeline::DeltaRle => {
            let d = delta::encode(data);
            (rle::encode(&d), 0x03)
        }

        TransformPipeline::BcjBwtMtf => {
        let bcj_data = bcj::encode(data);
        let bwt_result = bwt::encode(&bcj_data);
        let mut out = Vec::new();
        let idx = bwt_result.original_index as u32;
        out.extend_from_slice(&idx.to_le_bytes());
        out.extend(mtf::encode(&bwt_result.transformed));
        (out, 0x06)
        }

        TransformPipeline::BwtMtf => {
            let bwt_result = bwt::encode(data);
            let mut out = Vec::new();
            let idx = bwt_result.original_index as u32;
            out.extend_from_slice(&idx.to_le_bytes());
            out.extend(mtf::encode(&bwt_result.transformed));
            (out, 0x04)
        }
    }
}

/// Uygulanan pipeline'ı geri al (decode)
pub fn reverse_pipeline(data: &[u8], pipeline_id: u8) -> Vec<u8> {
    match pipeline_id {
        0x00 => data.to_vec(),
        0x01 => delta::decode(data),
        0x02 => rle::decode(data),
        0x03 => {
            let rle_decoded = rle::decode(data);
            delta::decode(&rle_decoded)
        }
        0x04 => {
            if data.len() < 4 {
                return data.to_vec();
            }
            let idx = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
            let mtf_data = &data[4..];
            let bwt_data = mtf::decode(mtf_data);
            bwt::decode(&bwt_data, idx)
        }

        0x06 => {
            if data.len() < 4 {
                return data.to_vec();
            }
            let idx = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
            let mtf_data = &data[4..];
            let bwt_data = mtf::decode(mtf_data);
            let bcj_data = bwt::decode(&bwt_data, idx);
            bcj::decode(&bcj_data)
        }
        _ => data.to_vec(),
    }
}

/// Shannon entropy hesabı: 0.0 → 8.0
fn calculate_entropy(data: &[u8]) -> f64 {
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

/// ASCII printable + boşluk oranı → metin mi değil mi?
fn text_byte_ratio(data: &[u8]) -> f64 {
    let text_count = data
        .iter()
        .filter(|&&b| b.is_ascii_alphanumeric() || b.is_ascii_punctuation() || b == b' ' || b == b'\n' || b == b'\r' || b == b'\t')
        .count();
    text_count as f64 / data.len() as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_detection() {
        let text = b"Hello World! This is a log file with repeated entries.\n";
        let result = analyze(text);
        println!("Text analysis: {:?}, entropy: {:.2}, compressibility: {:.2}",
            result.content_type, result.entropy, result.compressibility);
        assert_eq!(result.content_type, ContentType::Text);
    }

    #[test]
    fn test_numeric_detection() {
        // Kademeli artan sayısal veri
        let data: Vec<u8> = (0u8..=200).collect();
        let result = analyze(&data);
        println!("Numeric analysis: {:?}", result.content_type);
        assert!(matches!(result.content_type, ContentType::Numeric | ContentType::Binary));
    }

    #[test]
    fn test_repetitive_detection() {
        let data = vec![0u8; 200];
        let result = analyze(&data);
        println!("Repetitive analysis: {:?}", result.content_type);
        assert_eq!(result.content_type, ContentType::Repetitive);
    }

    #[test]
    fn test_full_pipeline_text() {
        let original = b"the cat sat on the mat the cat sat on the mat";
        let result = analyze(original);
        let (transformed, pipeline_id) = apply_pipeline(original, &result.pipeline);
        let recovered = reverse_pipeline(&transformed, pipeline_id);
        println!("Pipeline: {:?}, id: 0x{:02X}", result.pipeline, pipeline_id);
        println!("Original: {} bytes, Transformed: {} bytes", original.len(), transformed.len());
        assert_eq!(original.to_vec(), recovered, "Pipeline roundtrip başarısız!");
    }

    #[test]
    fn test_full_pipeline_numeric() {
        let data: Vec<u8> = (0u8..=200).collect();
        let result = analyze(&data);
        let (transformed, pipeline_id) = apply_pipeline(&data, &result.pipeline);
        let recovered = reverse_pipeline(&transformed, pipeline_id);
        assert_eq!(data, recovered);
    }

    #[test]
    fn test_entropy_calculation() {
        // Tümü aynı byte → entropy = 0
        let same = vec![0u8; 100];
        let result = analyze(&same);
        assert!(result.entropy < 0.01);

        // Uniform dağılım → entropy yüksek
        let uniform: Vec<u8> = (0u8..=255).collect();
        let result2 = analyze(&uniform);
        println!("Uniform entropy: {:.2}", result2.entropy);
        assert!(result2.entropy > 7.9);
    }
}
