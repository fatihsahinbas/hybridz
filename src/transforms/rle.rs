/// Run-Length Encoding (RLE) Transform
///
/// Ardışık tekrar eden byte'ları (count, value) çiftlerine dönüştürür.
/// Binary image, sıfır dolu buffer'lar, boşluk karakterleri için idealdir.
///
/// Format: [MARKER, count, value] → tekrar eden run
///         [byte]                 → tekil byte (marker değilse)
///
/// MARKER = 0xFE (254) — nadir kullanılan bir byte değeri seçildi
/// Eğer veri içinde 0xFE geçiyorsa: [MARKER, 1, 0xFE] olarak saklanır

const RLE_MARKER: u8 = 0xFE;
const MIN_RUN_LENGTH: usize = 3; // 3'ten kısa run'ları encode etme (overhead olur)

/// RLE encode
pub fn encode(data: &[u8]) -> Vec<u8> {
    if data.is_empty() {
        return Vec::new();
    }

    let mut result = Vec::with_capacity(data.len());
    let mut i = 0;

    while i < data.len() {
        let current = data[i];

        // Bu byte'tan kaç tane ardışık var?
        let mut run_len = 1;
        while i + run_len < data.len()
            && data[i + run_len] == current
            && run_len < 255
        {
            run_len += 1;
        }

        if run_len >= MIN_RUN_LENGTH || current == RLE_MARKER {
            // Run encode et: [MARKER, count, value]
            result.push(RLE_MARKER);
            result.push(run_len as u8);
            result.push(current);
        } else {
            // Tekil byte'ları olduğu gibi yaz
            for j in 0..run_len {
                result.push(data[i + j]);
            }
        }

        i += run_len;
    }

    result
}

/// RLE decode
pub fn decode(data: &[u8]) -> Vec<u8> {
    if data.is_empty() {
        return Vec::new();
    }

    let mut result = Vec::new();
    let mut i = 0;

    while i < data.len() {
        if data[i] == RLE_MARKER {
            // [MARKER, count, value] formatı
            if i + 2 >= data.len() {
                // Bozuk veri — kalan byte'ları olduğu gibi al
                result.extend_from_slice(&data[i..]);
                break;
            }
            let count = data[i + 1] as usize;
            let value = data[i + 2];
            for _ in 0..count {
                result.push(value);
            }
            i += 3;
        } else {
            result.push(data[i]);
            i += 1;
        }
    }

    result
}

/// RLE uygunluk skoru
/// Veri içindeki run oranını ölçer: ne kadar çok tekrar varsa o kadar iyi
pub fn suitability_score(data: &[u8]) -> f64 {
    if data.len() < MIN_RUN_LENGTH {
        return 0.0;
    }

    let mut total_run_bytes = 0usize;
    let mut i = 0;

    while i < data.len() {
        let current = data[i];
        let mut run_len = 1;
        while i + run_len < data.len() && data[i + run_len] == current {
            run_len += 1;
        }
        if run_len >= MIN_RUN_LENGTH {
            total_run_bytes += run_len;
        }
        i += run_len;
    }

    total_run_bytes as f64 / data.len() as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_roundtrip() {
        let original = vec![
            0u8, 0, 0, 0, 1, 2, 2, 2, 2, 3, 3, 255, 255, 255, 255, 255,
        ];
        let encoded = encode(&original);
        let decoded = decode(&encoded);
        assert_eq!(original, decoded);
    }

    #[test]
    fn test_no_run_data() {
        // Hiç run yok, her byte farklı
        let original = vec![1u8, 2, 3, 4, 5, 6, 7, 8];
        let encoded = encode(&original);
        let decoded = decode(&encoded);
        assert_eq!(original, decoded);
    }

    #[test]
    fn test_marker_byte_in_data() {
        // Veri içinde RLE_MARKER (0xFE) geçiyor
        let original = vec![RLE_MARKER, RLE_MARKER, RLE_MARKER, 1u8, 2, 3];
        let encoded = encode(&original);
        let decoded = decode(&encoded);
        assert_eq!(original, decoded);
    }

    #[test]
    fn test_all_same_bytes() {
        let original = vec![0xAAu8; 200];
        let encoded = encode(&original);
        println!(
            "200 same bytes: {} → {} bytes (ratio: {:.2}x)",
            original.len(),
            encoded.len(),
            original.len() as f64 / encoded.len() as f64
        );
        let decoded = decode(&encoded);
        assert_eq!(original, decoded);
        // 200 byte → 3 byte olmalı [MARKER, 200, 0xAA]... ama max run=255 yeterli
        assert!(encoded.len() < 10, "Çok tekrarlı veri küçülmeli");
    }

    #[test]
    fn test_suitability_score() {
        let high_run = vec![0u8; 100]; // Tümü sıfır
        let low_run: Vec<u8> = (0u8..100).collect(); // Hepsi farklı
        assert!(suitability_score(&high_run) > 0.9);
        assert!(suitability_score(&low_run) < 0.1);
    }
}
