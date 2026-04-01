/// Burrows-Wheeler Transform (BWT)
///
/// Veriyi, aynı karakterleri bir araya toplayacak şekilde yeniden düzenler.
/// Referans: Burrows & Wheeler (1994) "A Block-sorting Lossless Data Compression Algorithm"
///
/// Encode: Tüm döngüsel rotasyonları sırala → son sütunu al (L = last column)
/// Decode: L kullanarak orijinali geri üret (LF-mapping, sondan başa)

pub struct BwtResult {
    pub transformed: Vec<u8>,   // L = last column
    pub original_index: usize,  // Orijinal stringin sıralı rotasyonlar içindeki yeri
}

/// BWT encode — suffix array tabanlı
pub fn encode(data: &[u8]) -> BwtResult {
    let n = data.len();
    if n == 0 {
        return BwtResult { transformed: Vec::new(), original_index: 0 };
    }

    // Tüm döngüsel rotasyonları alfabetik sırala
    let mut indices: Vec<usize> = (0..n).collect();
    indices.sort_by(|&a, &b| {
        for k in 0..n {
            let ca = data[(a + k) % n];
            let cb = data[(b + k) % n];
            if ca != cb { return ca.cmp(&cb); }
        }
        a.cmp(&b)
    });

    // L = last column: her rotasyonun son karakteri = bir önceki index'teki byte
    let transformed: Vec<u8> = indices.iter()
        .map(|&i| data[(i + n - 1) % n])
        .collect();

    // Orijinal string (i=0'dan başlayan rotasyon) hangi sırada?
    let original_index = indices.iter().position(|&i| i == 0).unwrap();

    BwtResult { transformed, original_index }
}

/// BWT decode — LF-mapping, Burrows & Wheeler 1994 paper
///
/// Temel fikir:
///   L[i]'nin j-inci occurrence'ı → F'deki j-inci occurrence'ı ile aynı satır
///   (LF property)
///
/// Decode: result'u SONDAN BAŞA, L[current] kullanarak doldur
pub fn decode(data: &[u8], original_index: usize) -> Vec<u8> {
    let n = data.len();
    if n == 0 { return Vec::new(); }

    // F = first column = sorted L
    let mut first_col = data.to_vec();
    first_col.sort_unstable();

    // Her byte'ın F'de başladığı pozisyon (prefix sum)
    let mut f_start = [0usize; 256];
    {
        let mut cnt = [0usize; 256];
        for &b in first_col.iter() { cnt[b as usize] += 1; }
        let mut acc = 0usize;
        for i in 0..256 {
            f_start[i] = acc;
            acc += cnt[i];
        }
    }

    // LF mapping: lf[i] = L[i]'nin F'deki karşılık satırı
    // L'yi soldan sağa tara; aynı byte için occurrence sırasını koru (stable)
    let mut lf = vec![0usize; n];
    let mut l_occ = [0usize; 256];
    for i in 0..n {
        let b = data[i] as usize;
        lf[i] = f_start[b] + l_occ[b];
        l_occ[b] += 1;
    }

    // Decode: sondan başa, L[current] kullan
    // result[n-1] = L[original_index]
    // result[n-2] = L[lf[original_index]]
    // ...
    let mut result = vec![0u8; n];
    let mut current = original_index;
    for i in (0..n).rev() {
        result[i] = data[current];   // L[current] — data = L
        current = lf[current];
    }

    result
}

/// BWT uygunluk skoru — tekrarlı karakter dizileri ne kadar fazlaysa o kadar iyi
pub fn suitability_score(data: &[u8]) -> f64 {
    if data.len() < 4 { return 0.0; }
    let mut seen = [false; 256];
    for &b in data { seen[b as usize] = true; }
    let unique_count = seen.iter().filter(|&&x| x).count();
    let bigram_repeats = data.windows(2).filter(|w| w[0] == w[1]).count();
    let bigram_ratio = bigram_repeats as f64 / (data.len() - 1) as f64;
    let diversity_score = 1.0 - (unique_count as f64 / 256.0);
    (diversity_score * 0.5 + bigram_ratio * 0.5).min(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_banana_classic() {
        let input = b"banana";
        let result = encode(input);
        println!("BWT('banana') = {:?}, index = {}", 
            std::str::from_utf8(&result.transformed), result.original_index);
        assert_eq!(result.transformed, b"nnbaaa");
        let decoded = decode(&result.transformed, result.original_index);
        assert_eq!(input.to_vec(), decoded);
    }

    #[test]
    fn test_roundtrip_text() {
        let original = b"hello world hello world";
        let encoded = encode(original);
        let decoded = decode(&encoded.transformed, encoded.original_index);
        assert_eq!(original.to_vec(), decoded);
    }

    #[test]
    fn test_roundtrip_binary() {
        let original: Vec<u8> = (0u8..=127).collect();
        let encoded = encode(&original);
        let decoded = decode(&encoded.transformed, encoded.original_index);
        assert_eq!(original, decoded);
    }

    #[test]
    fn test_single_byte() {
        let original = vec![42u8];
        let encoded = encode(&original);
        let decoded = decode(&encoded.transformed, encoded.original_index);
        assert_eq!(original, decoded);
    }

    #[test]
    fn test_repetitive_data_groups_chars() {
        let original = b"abcabcabcabc";
        let encoded = encode(original);
        println!("BWT('abcabcabcabc') = {:?}", 
            std::str::from_utf8(&encoded.transformed));
        let decoded = decode(&encoded.transformed, encoded.original_index);
        assert_eq!(original.to_vec(), decoded);
    }

    #[test]
    fn test_all_same() {
        let original = vec![0xAAu8; 20];
        let encoded = encode(&original);
        let decoded = decode(&encoded.transformed, encoded.original_index);
        assert_eq!(original, decoded);
    }

    #[test]
    fn test_suitability_score() {
        let text = b"aaabbbcccaaabbbccc";
        let score = suitability_score(text);
        println!("BWT suitability: {:.3}", score);
        assert!(score > 0.3);
    }
}
