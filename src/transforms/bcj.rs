/// BCJ (Branch-Call-Jump) Transform — x86/x64
///
/// EXE/DLL dosyalarındaki relative call/jump offsetlerini
/// absolute adreslere çevirir.
///
/// Analoji: Şehir haritasında "100m ileriye dön" yerine
/// "Koordinat: 40.123, 29.456" yazmak — her yerden bakınca aynı nokta.
///
/// E8 XX XX XX XX → CALL rel32  (5 byte)
/// E9 XX XX XX XX → JMP  rel32  (5 byte)
///
/// Transform: offset_field += instruction_position
/// Inverse:   offset_field -= instruction_position

/// BCJ encode: relative adresleri absolute'a çevir
pub fn encode(data: &[u8]) -> Vec<u8> {
    let mut out = data.to_vec();
    let len = out.len();
    let mut i = 0;

    while i + 4 < len {
        let opcode = out[i];
        if opcode == 0xE8 || opcode == 0xE9 {
            // 4 byte little-endian relative offset oku
            let rel = i32::from_le_bytes([
                out[i + 1], out[i + 2], out[i + 3], out[i + 4],
            ]);
            // Absolute adrese çevir: abs = rel + (i + 5)
            // (i + 5) = instruction'dan sonraki adres = return address
            let abs = rel.wrapping_add((i as i32).wrapping_add(5));
            let abs_bytes = abs.to_le_bytes();
            out[i + 1] = abs_bytes[0];
            out[i + 2] = abs_bytes[1];
            out[i + 3] = abs_bytes[2];
            out[i + 4] = abs_bytes[3];
            i += 5; // instruction tamamlandı, atla
        } else {
            i += 1;
        }
    }

    out
}

/// BCJ decode: absolute adresleri relative'e geri çevir
pub fn decode(data: &[u8]) -> Vec<u8> {
    let mut out = data.to_vec();
    let len = out.len();
    let mut i = 0;

    while i + 4 < len {
        let opcode = out[i];
        if opcode == 0xE8 || opcode == 0xE9 {
            let abs = i32::from_le_bytes([
                out[i + 1], out[i + 2], out[i + 3], out[i + 4],
            ]);
            // Relative'e geri çevir: rel = abs - (i + 5)
            let rel = abs.wrapping_sub((i as i32).wrapping_add(5));
            let rel_bytes = rel.to_le_bytes();
            out[i + 1] = rel_bytes[0];
            out[i + 2] = rel_bytes[1];
            out[i + 3] = rel_bytes[2];
            out[i + 4] = rel_bytes[3];
            i += 5;
        } else {
            i += 1;
        }
    }

    out
}

/// BCJ uygunluk skoru
/// E8/E9 opcode yoğunluğunu ölçer — exe/dll için yüksek olur
pub fn suitability_score(data: &[u8]) -> f64 {
    if data.len() < 16 {
        return 0.0;
    }
    // Hızlı örnekleme: ilk 64KB'ı tara
    let sample = &data[..data.len().min(65536)];
    let mut i = 0;
    let mut call_jmp_count = 0usize;

    while i + 4 < sample.len() {
        let b = sample[i];
        if b == 0xE8 || b == 0xE9 {
            // Offset alanının son byte'ı 0x00 veya 0xFF ise gerçek call/jmp olabilir
            // (basit heuristic — false positive'leri azaltır)
            let high = sample[i + 4];
            if high == 0x00 || high == 0xFF {
                call_jmp_count += 1;
                i += 5;
                continue;
            }
        }
        i += 1;
    }

    // 64KB'da kaç call/jmp var?
    // Tipik exe: her ~50-100 byte'ta bir call → skor 0.5+
    let density = call_jmp_count as f64 / (sample.len() as f64 / 100.0);
    (density / 10.0).min(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip_empty() {
        assert_eq!(encode(&[]), vec![]);
        assert_eq!(decode(&[]), vec![]);
    }

    #[test]
    fn test_roundtrip_no_calls() {
        let data = vec![0x90u8; 100]; // NOP sled
        let encoded = encode(&data);
        let decoded = decode(&encoded);
        assert_eq!(data, decoded);
        assert_eq!(data, encoded); // NOP'lar değişmemeli
    }

    #[test]
    fn test_single_call() {
        // E8 00 00 00 00 → CALL +5 (kendini çağır, pratik değil ama test için)
        let mut data = vec![0x90u8; 20];
        data[5] = 0xE8;
        data[6] = 0x0A; // rel = 10
        data[7] = 0x00;
        data[8] = 0x00;
        data[9] = 0x00;

        let encoded = encode(&data);
        let decoded = decode(&encoded);
        assert_eq!(data, decoded, "BCJ roundtrip başarısız");

        // Encoded'da offset absolute olmalı: 10 + (5+5) = 20
        let abs = i32::from_le_bytes([encoded[6], encoded[7], encoded[8], encoded[9]]);
        assert_eq!(abs, 20, "Absolute adres yanlış");
    }

    #[test]
    fn test_same_target_different_positions() {
        // Aynı hedefe iki farklı pozisyondan CALL
        // Hedef: byte 100
        // Call 1: pos=10, rel = 100 - (10+5) = 85
        // Call 2: pos=50, rel = 100 - (50+5) = 45
        // BCJ sonrası ikisi de abs=100 olmalı

        let mut data = vec![0x90u8; 120];

        // Call 1 at pos 10
        data[10] = 0xE8;
        let rel1 = (100i32 - (10 + 5)).to_le_bytes();
        data[11..15].copy_from_slice(&rel1);

        // Call 2 at pos 50
        data[50] = 0xE8;
        let rel2 = (100i32 - (50 + 5)).to_le_bytes();
        data[51..55].copy_from_slice(&rel2);

        let encoded = encode(&data);

        // Her iki call da absolute=100 üretmeli
        let abs1 = i32::from_le_bytes([encoded[11], encoded[12], encoded[13], encoded[14]]);
        let abs2 = i32::from_le_bytes([encoded[51], encoded[52], encoded[53], encoded[54]]);
        assert_eq!(abs1, 100, "Call 1 absolute yanlış");
        assert_eq!(abs2, 100, "Call 2 absolute yanlış");

        // Roundtrip
        let decoded = decode(&encoded);
        assert_eq!(data, decoded);
    }

    #[test]
    fn test_notepad_roundtrip() {
        let data = std::fs::read("C:/Windows/System32/notepad.exe")
            .unwrap_or_else(|_| {
                // notepad yoksa küçük bir binary simüle et
                let mut d = vec![0u8; 1000];
                d[0] = 0x4D; d[1] = 0x5A; // MZ header
                d[100] = 0xE8;
                d[101..105].copy_from_slice(&50i32.to_le_bytes());
                d
            });

        let encoded = encode(&data);
        let decoded = decode(&encoded);
        assert_eq!(data, decoded, "notepad.exe BCJ roundtrip başarısız");
    }

    #[test]
    fn test_suitability_score_exe() {
        let data = std::fs::read("C:/Windows/System32/notepad.exe")
            .unwrap_or_else(|_| {
                // Simüle et: her 50 byte'ta bir E8
                let mut d = vec![0x90u8; 5000];
                for i in (0..4990).step_by(50) {
                    d[i] = 0xE8;
                    d[i+4] = 0x00;
                }
                d
            });
        let score = suitability_score(&data);
        eprintln!("notepad.exe BCJ suitability: {:.3}", score);
        assert!(score > 0.0);
    }
}