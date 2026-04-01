//! Huffman entropy coding with Package-Merge length-limited code assignment.
//!
//! Guarantees tree depth <= MAX_CODE_LEN (16), preserving Kraft inequality.


// ── constants ────────────────────────────────────────────────────────────────

const MAX_CODE_LEN: usize = 16;
const ALPHABET:    usize  = 256;

// ── public types ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct HuffmanTable {
    /// (code, bit_length) per symbol; bit_length==0 means symbol unused.
    pub codes: Vec<(u32, u8)>,
    /// Flat canonical table used by the decoder.
    pub decode_table: Vec<u8>,
}

// ── encode ───────────────────────────────────────────────────────────────────

pub fn encode(data: &[u8]) -> (Vec<u8>, HuffmanTable) {
    // 1. frequency count
    let mut freq = [0u64; ALPHABET];
    for &b in data {
        freq[b as usize] += 1;
    }

    // 2. length-limited code lengths via Package-Merge
    let lengths = package_merge(&freq, MAX_CODE_LEN);

    // 3. canonical codes from lengths
    let table = build_canonical_table(&lengths);

    // 4. bit-pack the payload
    let bits = encode_bits(data, &table);

    (bits, table)
}

pub fn decode(encoded: &[u8], table: &HuffmanTable, original_len: usize) -> Vec<u8> {
    decode_bits(encoded, table, original_len)
}

// ── Package-Merge (Larmore & Hirschberg) ─────────────────────────────────────
//
// Returns lengths[0..256]: code length for each symbol.
// Symbols with freq==0 get length 0 (unused).
//
// Algorithm sketch:
//   For L levels (L = max_len), build a "coin collector" structure.
//   At each level l (from max_len down to 1):
//     - Start with 2*ALPHABET "coins" — one per symbol, weight = freq[sym]
//     - Package pairs from the previous level, add them as coins here
//     - Sort all coins by weight, take the 2*(ALPHABET-1) lightest
//   The number of times symbol s appears across all levels = its code length.

fn package_merge(freq: &[u64; ALPHABET], max_len: usize) -> [u8; ALPHABET] {
    let syms: Vec<usize> = (0..ALPHABET).filter(|&i| freq[i] > 0).collect();

    if syms.is_empty() {
        return [0u8; ALPHABET];
    }
    if syms.len() == 1 {
        let mut out = [0u8; ALPHABET];
        out[syms[0]] = 1;
        return out;
    }

    let n = syms.len();

    // Package-Merge: her item sadece weight taşır.
    // Symbol membership'i AYRI bir Vec<Vec<u32>> ile takip et:
    // sym_counts[level][sym_idx] = o level'da o sembol kaç item'da seçildi.
    //
    // Ama bu pahalı. Daha basit: canonical Package-Merge'de
    // her level'da items listesi SADECE yapraklardan + önceki level paketlerinden oluşur.
    // "Paket" = önceki level'dan alınan 2 item'ın birleşimi.
    // Seçilen item sayısı = 2*(n-1).
    // Her seçilen item için, içindeki yaprakların (orijinal semboller) listesini tut.
    // Bir level'da sembol s'nin kaç seçili item'da geçtiği = o level'ın s'ye katkısı.
    // Toplam katkı = code length.
    //
    // Kritik: bir paket içinde aynı sembol en fazla 1 kez geçmeli.
    // Bu zaten garantili çünkü paket = 2 ayrı item'ın birleşimi,
    // ve yapraklar tekil semboller.
    // AMA: büyük paketler çok sembol içerebilir, ve bir level'da
    // aynı sembol birden fazla seçili item'da geçebilir — bu normaldir ve
    // counts bu şekilde birikir.

    // Her item'ı (weight, leaf_set) olarak tut.
    // leaf_set: syms index'leri kümesi — bool vec yerine sorted vec kullan,
    // duplicate kontrolü için.

    // Daha verimli: item'ları sadece weight ile tut,
    // ve "hangi yaprakları içeriyor" bilgisini bit array ile sakla.
    // n <= 256 olduğu için u128×2 = 256 bit yeterli, ama basitlik için
    // [u64; 4] kullanalım (256 bit).

    #[derive(Clone)]
    struct Item {
        weight: u64,
        bits: [u64; 4], // 256-bit membership, bit i = syms[i] dahil
    }

    impl Item {
        fn leaf(weight: u64, sym_idx: usize) -> Self {
            let mut bits = [0u64; 4];
            bits[sym_idx / 64] |= 1u64 << (sym_idx % 64);
            Item { weight, bits }
        }

        fn merge(a: &Item, b: &Item) -> Self {
            Item {
                weight: a.weight + b.weight,
                bits: [
                    a.bits[0] | b.bits[0],
                    a.bits[1] | b.bits[1],
                    a.bits[2] | b.bits[2],
                    a.bits[3] | b.bits[3],
                ],
            }
        }
    }

    let mut prev: Vec<Item> = Vec::new();
    let mut counts = vec![0u32; n];

    for _level in (1..=max_len).rev() {
        let mut items: Vec<Item> = Vec::new();

        // Yapraklar
        for (i, &s) in syms.iter().enumerate() {
            items.push(Item::leaf(freq[s], i));
        }

        // Önceki level'dan paketler
        let mut i = 0;
        while i + 1 < prev.len() {
            items.push(Item::merge(&prev[i], &prev[i + 1]));
            i += 2;
        }

        // Weight'e göre sırala, en hafif 2*(n-1) seç
        items.sort_by_key(|x| x.weight);
        let take = 2 * (n - 1);
        items.truncate(take);

        // Bu level'ın katkısını say
        // Bu level'da seçilen sembolleri bul (her sembol max 1 kez sayılır)
        let mut level_bits = [0u64; 4];
        for item in &items {
            level_bits[0] |= item.bits[0];
            level_bits[1] |= item.bits[1];
            level_bits[2] |= item.bits[2];
            level_bits[3] |= item.bits[3];
        }
        for word in 0..4 {
            let mut w = level_bits[word];
            while w != 0 {
                let bit = w.trailing_zeros() as usize;
                let sym_idx = word * 64 + bit;
                if sym_idx < n {
                    counts[sym_idx] += 1;
                }
                w &= w - 1;
            }
        }

        prev = items;
    }

    let mut lengths = [0u8; ALPHABET];
    for (i, &s) in syms.iter().enumerate() {
        lengths[s] = counts[i] as u8;
    }
    lengths
}

// ── Canonical code table ──────────────────────────────────────────────────────

fn build_canonical_table(lengths: &[u8; ALPHABET]) -> HuffmanTable {
    // Count symbols per length
    let mut bl_count = [0u32; MAX_CODE_LEN + 1];
    for &l in lengths.iter() {
        if l > 0 {
            bl_count[l as usize] += 1;
        }
    }

    // First code for each length (canonical assignment)
    let mut next_code = [0u32; MAX_CODE_LEN + 2];
    let mut code = 0u32;
    for bits in 1..=MAX_CODE_LEN {
        code = (code + bl_count[bits - 1]) << 1;
        next_code[bits] = code;
    }

    // Assign codes
    let mut codes = vec![(0u32, 0u8); ALPHABET];
    for sym in 0..ALPHABET {
        let l = lengths[sym];
        if l > 0 {
            codes[sym] = (next_code[l as usize], l);
            next_code[l as usize] += 1;
        }
    }

    // Build decode table
    let decode_table = build_decode_table(&codes);

    HuffmanTable { codes, decode_table }
}

// ── Decode table (flat array, MSB-first) ─────────────────────────────────────
//
// decode_table[code_bits] = symbol, where code_bits is the code
// left-justified in MAX_CODE_LEN bits.
// Entry is 0xFF if unused.

fn build_decode_table(codes: &[(u32, u8)]) -> Vec<u8> {
    let table_size = 1usize << MAX_CODE_LEN;
    let mut table = vec![0xFFu8; table_size];

    for sym in 0..ALPHABET {
        let (code, len) = codes[sym];
        if len == 0 {
            continue;
        }
        // Fill all entries that share this prefix
        let pad = MAX_CODE_LEN as u32 - len as u32;
        let base = code << pad;
        let count = 1u32 << pad;
        for k in 0..count {
            table[(base + k) as usize] = sym as u8;
        }
    }

    table
}

// ── Bit writer ────────────────────────────────────────────────────────────────

struct BitWriter {
    buf: Vec<u8>,
    staging: u64,
    bits: u32,
}

impl BitWriter {
    fn new() -> Self {
        Self { buf: Vec::new(), staging: 0, bits: 0 }
    }

    #[inline]
    fn write(&mut self, code: u32, len: u8) {
        self.staging = (self.staging << len) | (code as u64);
        self.bits += len as u32;
        while self.bits >= 8 {
            self.bits -= 8;
            self.buf.push((self.staging >> self.bits) as u8);
        }
    }

    fn finish(mut self) -> Vec<u8> {
        if self.bits > 0 {
            // Pad remaining bits to the left of the last byte
            self.buf.push((self.staging << (8 - self.bits)) as u8);
        }
        self.buf
    }
}

// ── Bit reader ────────────────────────────────────────────────────────────────

struct BitReader<'a> {
    data: &'a [u8],
    byte_pos: usize,
    window: u32,   // up to 24 bits buffered, MSB-first
    bits_avail: u32,
}

impl<'a> BitReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        let mut br = Self { data, byte_pos: 0, window: 0, bits_avail: 0 };
        br.refill();
        br
    }

    #[inline]
    fn refill(&mut self) {
        while self.bits_avail <= 24 && self.byte_pos < self.data.len() {
            self.window = (self.window << 8) | self.data[self.byte_pos] as u32;
            self.bits_avail += 8;
            self.byte_pos += 1;
        }
    }

    /// Peek the top MAX_CODE_LEN bits without consuming.
    #[inline]
    fn peek(&self) -> u32 {
        if self.bits_avail >= MAX_CODE_LEN as u32 {
            self.window >> (self.bits_avail - MAX_CODE_LEN as u32)
        } else {
            // Shift what we have to the top of a 16-bit window
            self.window << (MAX_CODE_LEN as u32 - self.bits_avail)
        }
    }

    #[inline]
    fn consume(&mut self, n: u8) {
        self.bits_avail -= n as u32;
        self.window &= (1u32 << self.bits_avail) - 1;
        self.refill();
    }
}

// ── encode_bits / decode_bits ─────────────────────────────────────────────────

fn encode_bits(data: &[u8], table: &HuffmanTable) -> Vec<u8> {
    let mut w = BitWriter::new();
    for &b in data {
        let (code, len) = table.codes[b as usize];
        w.write(code, len);
    }
    w.finish()
}

fn decode_bits(encoded: &[u8], table: &HuffmanTable, original_len: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(original_len);
    let mut br = BitReader::new(encoded);

    while out.len() < original_len {
        br.refill();
        let idx = br.peek() as usize;
        let sym = table.decode_table[idx];
        // Look up the actual code length for this symbol
        let len = table.codes[sym as usize].1;
        br.consume(len);
        out.push(sym);
    }

    out
}

// ── Serialization (for pipeline use) ─────────────────────────────────────────
//
// Format: 256 bytes of lengths, then payload bytes.

pub fn serialize_table(table: &HuffmanTable) -> Vec<u8> {
    let mut out = Vec::with_capacity(ALPHABET);
    for &(_, len) in &table.codes {
        out.push(len);
    }
    out
}

pub fn deserialize_table(lengths_bytes: &[u8]) -> HuffmanTable {
    assert_eq!(lengths_bytes.len(), ALPHABET);
    let mut lengths = [0u8; ALPHABET];
    lengths.copy_from_slice(lengths_bytes);
    build_canonical_table(&lengths)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip(data: &[u8]) {
        let (encoded, table) = encode(data);
        let decoded = decode(&encoded, &table, data.len());
        assert_eq!(data, decoded.as_slice(), "roundtrip failed for {} bytes", data.len());
    }

    #[test]
    fn test_empty() {
        roundtrip(&[]);
    }

    #[test]
    fn test_single_symbol_repeated() {
        roundtrip(&[42u8; 1000]);
    }

    #[test]
    fn test_two_symbols() {
        let data: Vec<u8> = (0..200).map(|i| if i % 3 == 0 { 0 } else { 1 }).collect();
        roundtrip(&data);
    }

    #[test]
    fn test_all_256_symbols() {
        let data: Vec<u8> = (0..=255u8).collect();
        roundtrip(&data);
    }

    #[test]
    fn test_small_random() {
        let data: Vec<u8> = (0..500).map(|i| (i * 7 + 13) as u8).collect();
        roundtrip(&data);
    }

    #[test]
    fn test_large_skewed() {
        // Highly skewed distribution — stress-tests depth limiting
        let mut data = Vec::with_capacity(100_000);
        for i in 0..100_000usize {
            let sym = match i % 16 {
                0  => 0u8,
                1  => 1,
                2  => 2,
                3  => 3,
                _  => (4 + (i % 252)) as u8,
            };
            data.push(sym);
        }
        roundtrip(&data);
    }

    #[test]
    fn test_serialize_roundtrip() {
        let data: Vec<u8> = (0..1000).map(|i| (i % 256) as u8).collect();
        let (encoded, table) = encode(&data);
        let lengths = serialize_table(&table);
        let table2 = deserialize_table(&lengths);
        let decoded = decode(&encoded, &table2, data.len());
        assert_eq!(data, decoded);
    }

    #[test]
    fn test_kraft_inequality() {
        let freq: [u64; 256] = {
            let mut f = [0u64; 256];
            // Worst case: many symbols with very different frequencies
            for i in 0..256usize {
                f[i] = 1u64 << (i % 8);
            }
            f
        };
        let lengths = package_merge(&freq, MAX_CODE_LEN);
        // Check Kraft: sum(2^-len) <= 1.0
        let kraft: f64 = lengths.iter()
            .filter(|&&l| l > 0)
            .map(|&l| 2f64.powi(-(l as i32)))
            .sum();
        assert!(kraft <= 1.0 + 1e-9, "Kraft inequality violated: {kraft}");
        // Check all lengths within bounds
        for &l in &lengths {
            assert!(l <= MAX_CODE_LEN as u8, "length {l} exceeds MAX_CODE_LEN");
        }
    }

    #[test]
    fn test_pm_debug() {
        let mut freq = [0u64; 256];
        for i in 0..256usize {
            freq[i] = 1u64 << (i % 8);
        }
        let lengths = package_merge(&freq, 16);
        let max_l = lengths.iter().copied().max().unwrap_or(0);
        eprintln!("max length = {}", max_l);
        let kraft: f64 = lengths.iter()
            .filter(|&&l| l > 0)
            .map(|&l| 2f64.powi(-(l as i32)))
            .sum();
        eprintln!("kraft = {}", kraft);
        assert!(max_l <= 16, "max length {} > 16", max_l);
    }
}