/// Burrows-Wheeler Transform (BWT)
///
/// Veriyi, aynı karakterleri bir araya toplayacak şekilde yeniden düzenler.
/// Referans: Burrows & Wheeler (1994) "A Block-sorting Lossless Data Compression Algorithm"
///
/// Encode: data + data üzerinde SA-IS kurar, rotasyon SA çıkarır
/// Decode: L kullanarak orijinali geri üret (LF-mapping, sondan başa)

pub struct BwtResult {
    pub transformed: Vec<u8>,  // L = last column
    pub original_index: usize, // Orijinal stringin sıralı rotasyonlar içindeki yeri
}

/// BWT encode — data + data üzerinde SA-IS, rotasyon SA çıkar
pub fn encode(data: &[u8]) -> BwtResult {
    let n = data.len();
    if n == 0 {
        return BwtResult { transformed: Vec::new(), original_index: 0 };
    }
    if n == 1 {
        return BwtResult { transformed: data.to_vec(), original_index: 0 };
    }

    // s = data + data + sentinel, uzunluk 2n+1
    // Byte değerleri +1 kaydırılır, sentinel = 0 (en küçük)
    let mut s: Vec<u32> = Vec::with_capacity(2 * n + 1);
    for _ in 0..2 {
        for &b in data {
            s.push(b as u32 + 1);
        }
    }
    s.push(0);

    let sa = suffix_array(&s, 258);

    // SA'dan rotasyon SA çıkar: sadece sai < n olanlar (sıra korunur)
    // BWT[i] = data[(sai - 1 + n) % n]
    let mut transformed = Vec::with_capacity(n);
    let mut original_index = 0usize;
    for &sai in &sa {
        if sai >= n {
            continue;
        }
        if sai == 0 {
            original_index = transformed.len();
        }
        let prev = if sai == 0 { n - 1 } else { sai - 1 };
        transformed.push(data[prev]);
    }

    BwtResult { transformed, original_index }
}

// ── SA-IS ────────────────────────────────────────────────────────────────────
//
// Nong/Zhang/Chan 2009. Input: s[n-1] sentinel olmalı (en küçük değer).
// Python referansının bire bir Rust çevirisi.

fn suffix_array(s: &[u32], sigma: usize) -> Vec<usize> {
    let n = s.len();
    if n == 1 { return vec![0]; }
    if n == 2 { return vec![1, 0]; }

    // S/L type
    let mut t = vec![false; n];
    t[n - 1] = true;
    for i in (0..n - 1).rev() {
        t[i] = match s[i].cmp(&s[i + 1]) {
            std::cmp::Ordering::Less => true,
            std::cmp::Ordering::Greater => false,
            std::cmp::Ordering::Equal => t[i + 1],
        };
    }

    let is_lms = |i: usize, t: &[bool]| -> bool { i > 0 && t[i] && !t[i - 1] };

    // Bucket sizes
    let mut bkt = vec![0usize; sigma];
    for &c in s { bkt[c as usize] += 1; }

    let bucket_heads = |bkt: &[usize], sigma: usize| -> Vec<usize> {
        let mut heads = vec![0usize; sigma];
        let mut acc = 0;
        for i in 0..sigma { heads[i] = acc; acc += bkt[i]; }
        heads
    };
    let bucket_tails = |bkt: &[usize], sigma: usize| -> Vec<usize> {
        let mut tails = vec![0usize; sigma];
        let mut acc = 0;
        for i in 0..sigma {
            acc += bkt[i];
            tails[i] = if acc > 0 { acc - 1 } else { 0 };
        }
        tails
    };

    // induced_sort: LMS pozisyonlarını tail'e koy, L-induce, S-induce
    let induced_sort = |
        lms_sorted: &[usize],
        s: &[u32],
        t: &[bool],
        bkt: &[usize],
        n: usize,
        sigma: usize,
    | -> Vec<usize> {
        let mut sa = vec![usize::MAX; n];

        // Step 1: LMS'leri bucket tail'lerine (tersten)
        let mut tails = bucket_tails(bkt, sigma);
        for &p in lms_sorted.iter().rev() {
            let c = s[p] as usize;
            sa[tails[c]] = p;
            if tails[c] > 0 { tails[c] -= 1; }
        }

        // Step 2: L-type induce (soldan sağa)
        let mut heads = bucket_heads(bkt, sigma);
        for i in 0..n {
            if sa[i] == usize::MAX || sa[i] == 0 { continue; }
            let j = sa[i] - 1;
            if !t[j] {
                let c = s[j] as usize;
                sa[heads[c]] = j;
                heads[c] += 1;
            }
        }

        // Step 3: S-type induce (sağdan sola)
        let mut tails = bucket_tails(bkt, sigma);
        for i in (0..n).rev() {
            if sa[i] == usize::MAX || sa[i] == 0 { continue; }
            let j = sa[i] - 1;
            if t[j] {
                let c = s[j] as usize;
                sa[tails[c]] = j;
                if tails[c] > 0 { tails[c] -= 1; }
            }
        }

        sa
    };

    // İlk induced sort
    let lms_positions: Vec<usize> = (0..n).filter(|&i| is_lms(i, &t)).collect();
    let sa = induced_sort(&lms_positions, s, &t, &bkt, n, sigma);

    // LMS substring naming
    let mut name = vec![usize::MAX; n];
    let mut cur_name = 0usize;
    let mut prev: Option<usize> = None;
    for i in 0..n {
        let p = sa[i];
        if p == usize::MAX || !is_lms(p, &t) { continue; }
        if let Some(q) = prev {
            let mut diff = false;
            let mut k = 0usize;
            loop {
                let pi = q + k;
                let pj = p + k;
                if pi >= n || pj >= n { diff = true; break; }
                if s[pi] != s[pj] || t[pi] != t[pj] { diff = true; break; }
                if k > 0 {
                    let pi_lms = is_lms(pi, &t);
                    let pj_lms = is_lms(pj, &t);
                    if pi_lms && pj_lms { break; }
                    if pi_lms != pj_lms { diff = true; break; }
                }
                k += 1;
            }
            if diff { cur_name += 1; }
        }
        name[p] = cur_name;
        prev = Some(p);
    }

    // Reduced string
    let s1: Vec<u32> = lms_positions.iter().map(|&p| name[p] as u32).collect();
    let sigma1 = cur_name + 1;

    let sa1 = if sigma1 == s1.len() {
        // Tüm name'ler unique → direkt sırala
        let mut sa1 = vec![0usize; s1.len()];
        for (i, &v) in s1.iter().enumerate() { sa1[v as usize] = i; }
        sa1
    } else {
        suffix_array(&s1, sigma1)
    };

    let sorted_lms: Vec<usize> = sa1.iter().map(|&i| lms_positions[i]).collect();
    induced_sort(&sorted_lms, s, &t, &bkt, n, sigma)
}

/// BWT decode — LF-mapping, Burrows & Wheeler 1994 paper
pub fn decode(data: &[u8], original_index: usize) -> Vec<u8> {
    let n = data.len();
    if n == 0 {
        return Vec::new();
    }

    // F = first column = sorted L
    let mut first_col = data.to_vec();
    first_col.sort_unstable();

    // Her byte'ın F'de başladığı pozisyon (prefix sum)
    let mut f_start = [0usize; 256];
    {
        let mut cnt = [0usize; 256];
        for &b in first_col.iter() {
            cnt[b as usize] += 1;
        }
        let mut acc = 0usize;
        for i in 0..256 {
            f_start[i] = acc;
            acc += cnt[i];
        }
    }

    // LF mapping
    let mut lf = vec![0usize; n];
    let mut l_occ = [0usize; 256];
    for i in 0..n {
        let b = data[i] as usize;
        lf[i] = f_start[b] + l_occ[b];
        l_occ[b] += 1;
    }

    // Decode: sondan başa
    let mut result = vec![0u8; n];
    let mut current = original_index;
    for i in (0..n).rev() {
        result[i] = data[current];
        current = lf[current];
    }

    result
}

/// BWT uygunluk skoru — tekrarlı karakter dizileri ne kadar fazlaysa o kadar iyi
pub fn suitability_score(data: &[u8]) -> f64 {
    if data.len() < 4 {
        return 0.0;
    }
    let mut seen = [false; 256];
    for &b in data {
        seen[b as usize] = true;
    }
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
        println!(
            "BWT('banana') = {:?}, index = {}",
            std::str::from_utf8(&result.transformed),
            result.original_index
        );
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
        println!(
            "BWT('abcabcabcabc') = {:?}",
            std::str::from_utf8(&encoded.transformed)
        );
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