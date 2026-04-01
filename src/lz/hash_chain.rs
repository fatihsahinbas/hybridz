/// Hash Chain
///
/// Sliding window içinde aynı 4-byte prefix'e sahip pozisyonları zincirler.
/// Analoji: Telefon rehberi — "abc" ile başlayan tüm isimler aynı sayfada.
///
/// KRİTİK: Mutlak u32 pozisyon sakla, window-relative u16 değil!
/// u16 + !WINDOW_MASK → usize overflow hatası (önceki dersten)

use crate::lz::token::WINDOW_SIZE;

const HASH_SIZE: usize = 65536; // 2^16
const HASH_MASK: usize = HASH_SIZE - 1;
const NIL: u32 = u32::MAX;

pub struct HashChain {
    /// head[hash] = bu hash'e sahip son pozisyon (mutlak)
    head: Vec<u32>,
    /// prev[pos % WINDOW_SIZE] = aynı hash'in bir önceki pozisyonu (mutlak)
    prev: Vec<u32>,
    /// Toplam işlenen byte sayısı
    pub total: usize,
}

impl HashChain {
    pub fn new() -> Self {
        HashChain {
            head: vec![NIL; HASH_SIZE],
            prev: vec![NIL; WINDOW_SIZE],
            total: 0,
        }
    }

    /// 4-byte'tan hash üret
    fn hash4(data: &[u8], pos: usize) -> usize {
        if pos + 3 >= data.len() { return 0; }
        let v = u32::from_le_bytes([
            data[pos], data[pos+1], data[pos+2], data[pos+3]
        ]);
        // Fibonacci hashing
        let h = v.wrapping_mul(0x9E3779B9);
        (h >> 16) as usize & HASH_MASK
    }

    /// Pozisyonu zincire ekle
    pub fn insert(&mut self, data: &[u8], pos: usize) {
        let h = Self::hash4(data, pos);
        let slot = pos % WINDOW_SIZE;
        self.prev[slot] = self.head[h];
        self.head[h] = pos as u32;
        self.total = self.total.max(pos + 1);
    }

    /// En iyi match'i bul
    /// Döner: Option<(offset, length)>
    pub fn find_match(
        &self,
        data: &[u8],
        pos: usize,
        best_len: usize,
    ) -> Option<(usize, usize)> {
        if pos + MIN_MATCH_LOCAL > data.len() { return None; }

        let h = Self::hash4(data, pos);
        let mut candidate = self.head[h];
        let mut best_len = best_len.max(MIN_MATCH_LOCAL - 1);
        let mut best_match: Option<(usize, usize)> = None;
        let mut chain_limit = 128; // max chain derinliği

        while candidate != NIL && chain_limit > 0 {
            chain_limit -= 1;
            let cand = candidate as usize;

            // Window sınırı kontrolü
            if cand + WINDOW_SIZE <= pos { break; }
            if cand >= pos { 
                // Zincirde ilerle
                let slot = cand % WINDOW_SIZE;
                candidate = self.prev[slot];
                continue;
            }

            let offset = pos - cand;
            if offset == 0 || offset > WINDOW_SIZE { 
                let slot = cand % WINDOW_SIZE;
                candidate = self.prev[slot];
                continue;
            }

            // Match uzunluğunu ölç
            let max_len = (data.len() - pos).min(crate::lz::token::MAX_MATCH);
            let mut len = 0;
            while len < max_len && data[pos + len] == data[cand + len] {
                len += 1;
            }

            if len > best_len {
                best_len = len;
                best_match = Some((offset, len));
                if len >= crate::lz::token::MAX_MATCH { break; }
            }

            let slot = cand % WINDOW_SIZE;
            candidate = self.prev[slot];
        }

        best_match
    }
}

const MIN_MATCH_LOCAL: usize = crate::lz::token::MIN_MATCH;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_and_find() {
        let data = b"abcdabcd";
        let mut chain = HashChain::new();

        // İlk 4 byte'ı ekle
        for i in 0..4 {
            chain.insert(data, i);
        }

        // pos=4'te match ara — "abcd" tekrar ediyor
        let result = chain.find_match(data, 4, 0);
        assert!(result.is_some());
        let (offset, length) = result.unwrap();
        assert_eq!(offset, 4);
        assert!(length >= 4);
    }

    #[test]
    fn test_no_match_unique_data() {
        let data = b"abcdefgh";
        let mut chain = HashChain::new();
        for i in 0..4 {
            chain.insert(data, i);
        }
        // pos=4: "efgh" — daha önce görülmedi
        let result = chain.find_match(data, 4, 0);
        assert!(result.is_none());
    }

    #[test]
    fn test_find_before_insert() {
        // KRİTİK: önce find, sonra insert (self-match önlenir)
        let data = b"abcdabcd";
        let mut chain = HashChain::new();

        for pos in 0..8 {
            let _m = chain.find_match(data, pos, 0);
            chain.insert(data, pos);
        }
        // En az bir match bulunmalı
    }

    #[test]
    fn test_absolute_position() {
        // u32 mutlak pozisyon — overflow olmamalı
        let data: Vec<u8> = (0u8..=255).cycle().take(1000).collect();
        let mut chain = HashChain::new();
        for pos in 0..data.len().saturating_sub(4) {
            let _ = chain.find_match(&data, pos, 0);
            chain.insert(&data, pos);
        }
        // Panic olmadan tamamlanmalı
    }
}