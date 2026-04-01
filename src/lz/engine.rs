/// LZ Match Engine
///
/// Lazy matching ile sliding window LZ sıkıştırma.
/// Analoji: Bir kelimeyi okurken bir sonrakine bakarsın —
/// eğer sonraki daha iyi match veriyorsa, şimdikini literal olarak geç.
///
/// Lazy matching: pos'ta match bulduktan sonra pos+1'e de bak.
/// Eğer pos+1 daha uzun match veriyorsa, pos'u literal yaz.

use crate::lz::token::{Token, MIN_MATCH, MAX_MATCH, WINDOW_SIZE};
use crate::lz::hash_chain::HashChain;

pub struct LzEngine {
    lazy_threshold: usize,
}

impl LzEngine {
    pub fn new() -> Self {
        LzEngine { lazy_threshold: 8 }
    }

    pub fn compress(&self, data: &[u8]) -> Vec<Token> {
        let mut tokens = Vec::new();
        let mut chain = HashChain::new();
        let mut pos = 0;

        while pos < data.len() {
            if pos + MIN_MATCH > data.len() {
                tokens.push(Token::Literal(data[pos]));
                pos += 1;
                continue;
            }

            let current_match = chain.find_match(data, pos, 0);
            chain.insert(data, pos);

            match current_match {
                None => {
                    tokens.push(Token::Literal(data[pos]));
                    pos += 1;
                }
                Some((offset, length)) => {
                    if length < MIN_MATCH {
                        tokens.push(Token::Literal(data[pos]));
                        pos += 1;
                        continue;
                    }

                    let use_current = if pos + 1 + MIN_MATCH <= data.len() {
                        let next_match = chain.find_match(data, pos + 1, 0);
                        chain.insert(data, pos + 1);

                        match next_match {
                            Some((_, next_len)) if next_len > length + self.lazy_threshold => {
                                tokens.push(Token::Literal(data[pos]));
                                pos += 1;
                                false
                            }
                            _ => true,
                        }
                    } else {
                        true
                    };

                    if use_current {
                        tokens.push(Token::Match {
                            offset: offset as u32,  // u16 → u32
                            length: length as u32,  // u16 → u32
                        });
                        let skip_start = if pos + 1 + MIN_MATCH <= data.len() { 2 } else { 1 };
                        for i in skip_start..length {
                            if pos + i < data.len() {
                                chain.insert(data, pos + i);
                            }
                        }
                        pos += length;
                    }
                }
            }
        }

        tokens
    }

    pub fn compress_to_bytes(&self, data: &[u8]) -> Vec<u8> {
        let tokens = self.compress(data);
        Token::serialize_all(&tokens)
    }

    pub fn decompress_from_bytes(&self, data: &[u8]) -> Vec<u8> {
        let tokens = Token::deserialize_all(data);
        Token::decode(&tokens)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip_simple() {
        let engine = LzEngine::new();
        let data = b"abcdabcdabcd";
        let compressed = engine.compress_to_bytes(data);
        let decompressed = engine.decompress_from_bytes(&compressed);
        assert_eq!(data.to_vec(), decompressed);
    }

    #[test]
    fn test_roundtrip_text() {
        let engine = LzEngine::new();
        let data = b"the quick brown fox jumps over the lazy dog the quick brown fox";
        let compressed = engine.compress_to_bytes(data);
        let decompressed = engine.decompress_from_bytes(&compressed);
        assert_eq!(data.to_vec(), decompressed);
    }

    #[test]
    fn test_roundtrip_repetitive() {
        let engine = LzEngine::new();
        let data = vec![0xAAu8; 1000];
        let compressed = engine.compress_to_bytes(&data);
        let decompressed = engine.decompress_from_bytes(&compressed);
        assert_eq!(data, decompressed);
    }

    #[test]
    fn test_roundtrip_binary() {
        let engine = LzEngine::new();
        let data: Vec<u8> = (0u8..=255).cycle().take(512).collect();
        let compressed = engine.compress_to_bytes(&data);
        let decompressed = engine.decompress_from_bytes(&compressed);
        assert_eq!(data, decompressed);
    }

    #[test]
    fn test_compression_ratio_repetitive() {
        let engine = LzEngine::new();
        let data = vec![0xAAu8; 1000];
        let compressed = engine.compress_to_bytes(&data);
        println!("Repetitive: {} → {} bytes", data.len(), compressed.len());
        assert!(compressed.len() < data.len() / 2);
    }

    #[test]
    fn test_empty_input() {
        let engine = LzEngine::new();
        let compressed = engine.compress_to_bytes(&[]);
        let decompressed = engine.decompress_from_bytes(&compressed);
        assert_eq!(decompressed, Vec::<u8>::new());
    }

    #[test]
    fn test_single_byte() {
        let engine = LzEngine::new();
        let data = vec![42u8];
        let compressed = engine.compress_to_bytes(&data);
        let decompressed = engine.decompress_from_bytes(&compressed);
        assert_eq!(data, decompressed);
    }
}