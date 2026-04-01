/// Move-To-Front (MTF) Transform
///
/// BWT çıktısını entropy coder için daha elverişli hale getirir.
/// BWT sonrası aynı karakterler gruplanmıştır; MTF bu grupları
/// küçük sayılara (çoğunlukla 0) dönüştürür.
///
/// Analoji: Çok kullandığın araçları masanın önüne taşımak.
/// Sık gelen karakter → düşük index → entropi coder'a kolay iş.
///
/// Örnek:
///   Alfabe: [a,b,c,d,e,...]
///   Giriş:  [a, a, a, b, b]
///   Çıktı:  [0, 0, 0, 1, 0]  ← çoğunlukla 0 ve 1 → mükemmel entropi

/// MTF encode
/// alphabet: başlangıç alfabe durumu (genellikle [0u8..=255])
pub fn encode(data: &[u8]) -> Vec<u8> {
    // Standart: 0..=255 arası tüm byte değerleri başlangıç alfabesi
    let mut alphabet: Vec<u8> = (0u8..=255).collect();
    let mut result = Vec::with_capacity(data.len());

    for &byte in data {
        // Bu byte alfabe içinde hangi index'te?
        let pos = alphabet.iter().position(|&x| x == byte).unwrap();
        result.push(pos as u8);

        // Bu elemanı öne taşı
        alphabet.remove(pos);
        alphabet.insert(0, byte);
    }

    result
}

/// MTF decode
pub fn decode(data: &[u8]) -> Vec<u8> {
    let mut alphabet: Vec<u8> = (0u8..=255).collect();
    let mut result = Vec::with_capacity(data.len());

    for &index in data {
        let byte = alphabet[index as usize];
        result.push(byte);

        // Bu elemanı öne taşı
        alphabet.remove(index as usize);
        alphabet.insert(0, byte);
    }

    result
}

/// MTF uygunluk skoru
/// Encode sonrası düşük değerlerin (0,1,2) yoğunluğunu ölçer.
/// BWT sonrası uygulandığında bu oran genellikle çok yüksektir.
pub fn suitability_score_after_encode(data: &[u8]) -> f64 {
    if data.is_empty() {
        return 0.0;
    }
    let encoded = encode(data);
    // 0,1,2 değerlerinin toplam içindeki oranı
    let low_count = encoded.iter().filter(|&&x| x <= 2).count();
    low_count as f64 / encoded.len() as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_roundtrip() {
        let original = b"hello world";
        let encoded = encode(original);
        let decoded = decode(&encoded);
        assert_eq!(original.to_vec(), decoded);
    }

    #[test]
    fn test_repetitive_produces_zeros() {
        // Tekrarlı veri → MTF sonrası çoğunlukla 0
        let input = b"aaaabbbbcccc";
        let encoded = encode(input);
        println!("MTF('aaaabbbbcccc') = {:?}", encoded);
        // İkinci ve sonraki a'lar → 0 olmalı
        assert_eq!(encoded[1], 0); // ikinci 'a' → 0
        assert_eq!(encoded[2], 0); // üçüncü 'a' → 0
        assert_eq!(encoded[3], 0); // dördüncü 'a' → 0
    }

    #[test]
    fn test_bwt_plus_mtf_pipeline() {
        // BWT + MTF birlikte kullanım testi
        use crate::transforms::bwt;

        let original = b"abracadabra";
        let bwt_result = bwt::encode(original);
        println!(
            "BWT output: {:?}",
            std::str::from_utf8(&bwt_result.transformed)
        );

        let mtf_encoded = encode(&bwt_result.transformed);
        println!("MTF output: {:?}", mtf_encoded);

        // Decode pipeline (ters sıra)
        let mtf_decoded = decode(&mtf_encoded);
        let final_decoded = bwt::decode(&mtf_decoded, bwt_result.original_index);

        assert_eq!(original.to_vec(), final_decoded);
        println!("BWT+MTF roundtrip: ✓");
    }

    #[test]
    fn test_all_same_bytes() {
        // Tümü aynı byte → ilk hariç hepsi 0 olmalı
        let input = vec![0xAAu8; 50];
        let encoded = encode(&input);
        assert_eq!(encoded[0], 0xAA); // ilk kez görülüyor, alfabe[170] = 0xAA
        for i in 1..encoded.len() {
            assert_eq!(encoded[i], 0, "Tekrarlı byte'lar 0 olmalı");
        }
        let decoded = decode(&encoded);
        assert_eq!(input, decoded);
    }

    #[test]
    fn test_suitability_score() {
        let repetitive = b"aaabbbcccaaabbb";
        let score = suitability_score_after_encode(repetitive);
        println!("MTF suitability score: {:.3}", score);
        // Tekrarlı veri → yüksek skor beklenir
        assert!(score > 0.4);
    }
}
