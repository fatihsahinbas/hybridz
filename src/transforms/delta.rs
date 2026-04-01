/// Delta Encoding Transform
///
/// Her byte'ı bir öncekiyle farkı olarak saklar.
/// Sayısal seriler ve binary dosyalar için idealdir.
///
/// Örnek:
///   encode([10, 12, 11, 14]) → [10, 2, 255, 3]
///   (255 = -1 in u8 wrapping arithmetic)

/// Delta encode: her eleman öncekiyle farkına dönüşür
pub fn encode(data: &[u8]) -> Vec<u8> {
    if data.is_empty() {
        return Vec::new();
    }

    let mut result = Vec::with_capacity(data.len());
    result.push(data[0]); // İlk byte olduğu gibi saklanır (referans noktası)

    for i in 1..data.len() {
        // wrapping_sub: u8 overflow'u güvenli şekilde yönetir
        // 10u8 - 12u8 = 254u8 (wrap around) → decode'da wrapping_add ile geri alınır
        result.push(data[i].wrapping_sub(data[i - 1]));
    }

    result
}

/// Delta decode: farkları toplayarak orijinal veriyi geri üretir
pub fn decode(data: &[u8]) -> Vec<u8> {
    if data.is_empty() {
        return Vec::new();
    }

    let mut result = Vec::with_capacity(data.len());
    result.push(data[0]); // İlk byte referans

    for i in 1..data.len() {
        // Önceki değer + delta = orijinal değer
        let prev = result[i - 1];
        result.push(prev.wrapping_add(data[i]));
    }

    result
}

/// Verinin delta encoding'e ne kadar uygun olduğunu ölçer.
/// Döndürülen skor: 0.0 (hiç uygun değil) → 1.0 (mükemmel uygun)
/// Karar mantığı: delta sonrası ortalama mutlak değer düşükse → uygun
pub fn suitability_score(data: &[u8]) -> f64 {
    if data.len() < 2 {
        return 0.0;
    }

    // Ham verinin ortalama mutlak değeri
    let raw_avg: f64 = data.iter().map(|&b| b as f64).sum::<f64>() / data.len() as f64;

    // Delta sonrası ortalama mutlak delta değeri
    let delta_sum: f64 = data
        .windows(2)
        .map(|w| (w[1] as i16 - w[0] as i16).unsigned_abs() as f64)
        .sum();
    let delta_avg = delta_sum / (data.len() - 1) as f64;

    // Skor: delta ortalama, ham ortalamadan ne kadar küçük?
    // 1.0'a yakınsa delta encoding çok faydalı demektir
    if raw_avg == 0.0 {
        return 0.0;
    }
    (1.0 - (delta_avg / raw_avg)).max(0.0).min(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_decode_roundtrip() {
        let original = vec![10u8, 12, 11, 14, 13, 15, 200, 201, 199];
        let encoded = encode(&original);
        let decoded = decode(&encoded);
        assert_eq!(original, decoded, "Roundtrip başarısız!");
    }

    #[test]
    fn test_empty_input() {
        assert_eq!(encode(&[]), Vec::<u8>::new());
        assert_eq!(decode(&[]), Vec::<u8>::new());
    }

    #[test]
    fn test_overflow_wrapping() {
        // 5 - 200 = overflow → wrapping ile güvenli
        let original = vec![200u8, 5, 200];
        let encoded = encode(&original);
        let decoded = decode(&encoded);
        assert_eq!(original, decoded);
    }

    #[test]
    fn test_suitability_score_high_for_incremental() {
        // Kademeli artan veri → delta'ya çok uygun
        let data: Vec<u8> = (0u8..=100).collect();
        let score = suitability_score(&data);
        println!("Incremental data score: {:.3}", score);
        assert!(score > 0.5, "Kademeli veri için skor yüksek olmalı");
    }

    #[test]
    fn test_suitability_score_low_for_random() {
        // Sabit değer veri → delta pek faydalı değil (tümü sıfır olur, aslında iyi ama farklı bir case)
        let data = vec![128u8; 100];
        let score = suitability_score(&data);
        println!("Constant data score: {:.3}", score);
        // Sabit veri için delta = 0, ham = 128 → skor = 1.0 (bu da aslında iyi bir durum)
        // Rastgele veri testi için farklı bir yaklaşım:
        let random_like = vec![
            12u8, 200, 45, 178, 93, 11, 234, 67, 155, 88, 32, 210, 5, 189, 76,
        ];
        let random_score = suitability_score(&random_like);
        println!("Random-like data score: {:.3}", random_score);
        // Rastgele veride skor düşük olmalı
        assert!(random_score < 0.6);
    }
}
