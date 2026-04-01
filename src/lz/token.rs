/// LZ Token
///
/// LZ engine'in ürettiği token stream.
/// Her token ya bir literal byte ya da bir (offset, length) match'tir.
///
/// Analoji: Metin editöründe "bul-değiştir" gibi.
/// "Bu kelimeyi 50 karakter önce gördük, 8 karakter uzunluğunda" → Match
/// Görülmemiş karakter → Literal

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Literal(u8),
    Match { offset: u32, length: u32 },
}

pub const MIN_MATCH: usize = 4;
pub const MAX_MATCH: usize = 258;
pub const WINDOW_SIZE: usize = 131072; // 128KB

impl Token {
    /// Token'ı byte dizisine serialize et
    /// Literal:  [0x00, byte]                              → 2 byte
    /// Match:    [0x01, off0, off1, off2, off3, len0, len1, len2, len3] → 9 byte
    pub fn serialize(&self, out: &mut Vec<u8>) {
        match self {
            Token::Literal(b) => {
                out.push(0x00);
                out.push(*b);
            }
            Token::Match { offset, length } => {
                out.push(0x01);
                // offset: 3 byte (24 bit, max 16MB — 128KB için yeterli)
                out.push((offset & 0xFF) as u8);
                out.push(((offset >> 8) & 0xFF) as u8);
                out.push(((offset >> 16) & 0xFF) as u8);
                // length: 2 byte (max 65535 — MAX_MATCH=258 için yeterli)
                out.push((length & 0xFF) as u8);
                out.push(((length >> 8) & 0xFF) as u8);
            }
        }
    }

    /// Byte dizisinden token stream deserialize et
    pub fn deserialize_all(data: &[u8]) -> Vec<Token> {
        let mut tokens = Vec::new();
        let mut i = 0;
        while i < data.len() {
            match data[i] {
                0x00 => {
                    if i + 1 < data.len() {
                        tokens.push(Token::Literal(data[i + 1]));
                        i += 2;
                    } else { break; }
                }
                0x01 => {
                    if i + 5 < data.len() {
                        let offset = (data[i + 1] as u32)
                            | ((data[i + 2] as u32) << 8)
                            | ((data[i + 3] as u32) << 16);
                        let length = (data[i + 4] as u32)
                            | ((data[i + 5] as u32) << 8);
                        tokens.push(Token::Match { offset, length });
                        i += 6;
                    } else { break; }
                }
                _ => { i += 1; }
            }
        }
        tokens
    }

    /// Token stream'i tek Vec<u8>'e serialize et
    pub fn serialize_all(tokens: &[Token]) -> Vec<u8> {
        let mut out = Vec::new();
        for t in tokens {
            t.serialize(&mut out);
        }
        out
    }

    /// Token stream'den orijinal veriyi geri üret
    pub fn decode(tokens: &[Token]) -> Vec<u8> {
        let mut output: Vec<u8> = Vec::new();
        for token in tokens {
            match token {
                Token::Literal(b) => output.push(*b),
                Token::Match { offset, length } => {
                    let start = output.len().saturating_sub(*offset as usize);
                    for j in 0..*length as usize {
                        let byte = output[start + j];
                        output.push(byte);
                    }
                }
            }
        }
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_literal_serialize() {
        let t = Token::Literal(0xAB);
        let mut out = Vec::new();
        t.serialize(&mut out);
        assert_eq!(out, vec![0x00, 0xAB]);
    }

    #[test]
    fn test_match_serialize() {
        let t = Token::Match { offset: 10, length: 5 };
        let mut out = Vec::new();
        t.serialize(&mut out);
        assert_eq!(out, vec![0x01, 10, 0, 0, 5, 0]);  // 6 byte
    }

    #[test]
    fn test_serialize_deserialize_roundtrip() {
        let tokens = vec![
            Token::Literal(b'h'),
            Token::Literal(b'e'),
            Token::Literal(b'l'),
            Token::Literal(b'l'),
            Token::Literal(b'o'),
            Token::Match { offset: 5, length: 4 },
        ];
        let bytes = Token::serialize_all(&tokens);
        let recovered = Token::deserialize_all(&bytes);
        assert_eq!(tokens, recovered);
    }

    #[test]
    fn test_decode_with_match() {
        let tokens = vec![
            Token::Literal(b'a'),
            Token::Literal(b'b'),
            Token::Literal(b'c'),
            Token::Literal(b'd'),
            Token::Match { offset: 4, length: 4 },
        ];
        let decoded = Token::decode(&tokens);
        assert_eq!(decoded, b"abcdabcd");
    }

    #[test]
    fn test_decode_overlapping_match() {
        let tokens = vec![
            Token::Literal(b'a'),
            Token::Match { offset: 1, length: 3 },
        ];
        let decoded = Token::decode(&tokens);
        assert_eq!(decoded, b"aaaa");
    }
}