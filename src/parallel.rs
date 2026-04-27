/// Paralel Sıkıştırma
///
/// Büyük veriyi bloklara böler, her bloğu rayon ile ayrı thread'de sıkıştırır.
///
/// Analoji: Bir kitabı 4 kişiye böl, herkes kendi bölümünü çevirsin, sonra birleştir.
///
/// Format:
///   [MAGIC: 4B] [block_count: 4B LE] [block_sizes: block_count × 4B LE] [block_data...]
///
/// MAGIC = 0x485A5042 ("HZPB" — HybridZ Parallel Block)

use rayon::prelude::*;
use crate::codec;

const MAGIC: u32 = 0x485A5042;
const BLOCK_SIZE: usize = 512 * 1024; // 512 KB

pub fn compress(data: &[u8]) -> Result<Vec<u8>, String> {
    if data.is_empty() {
        return Ok(Vec::new());
    }

    // Veriyi bloklara böl
    let blocks: Vec<&[u8]> = data.chunks(BLOCK_SIZE).collect();
    let block_count = blocks.len() as u32;

    // Her bloğu paralel sıkıştır
    let compressed_blocks: Vec<Vec<u8>> = blocks
        .par_iter()
        .map(|block| codec::compress(block).unwrap_or_default())
        .collect();

    // Çıktıyı birleştir
    let mut output = Vec::new();
    output.extend_from_slice(&MAGIC.to_le_bytes());
    output.extend_from_slice(&block_count.to_le_bytes());

    // Her bloğun boyutunu yaz
    for cb in &compressed_blocks {
        output.extend_from_slice(&(cb.len() as u32).to_le_bytes());
    }

    // Blok verilerini yaz
    for cb in &compressed_blocks {
        output.extend_from_slice(cb);
    }

    Ok(output)
}

pub fn decompress(data: &[u8]) -> Result<Vec<u8>, String> {
    if data.is_empty() {
        return Ok(Vec::new());
    }

    if data.len() < 8 {
        return Err("Header eksik".to_string());
    }

    let magic = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    if magic != MAGIC {
        return Err(format!("Geçersiz magic: 0x{:08X}", magic));
    }

    let block_count = u32::from_le_bytes([data[4], data[5], data[6], data[7]]) as usize;

    if data.len() < 8 + block_count * 4 {
        return Err("Block size tablosu eksik".to_string());
    }

    // Blok boyutlarını oku
    let mut block_sizes = Vec::with_capacity(block_count);
    for i in 0..block_count {
        let off = 8 + i * 4;
        let size = u32::from_le_bytes([data[off], data[off+1], data[off+2], data[off+3]]) as usize;
        block_sizes.push(size);
    }

    // Blok verilerini ayır
    let mut offset = 8 + block_count * 4;
    let mut block_slices = Vec::with_capacity(block_count);
    for &size in &block_sizes {
        if offset + size > data.len() {
            return Err("Veri kesik".to_string());
        }
        block_slices.push(&data[offset..offset + size]);
        offset += size;
    }

    // Her bloğu paralel aç
    let results: Vec<Result<Vec<u8>, String>> = block_slices
        .par_iter()
        .map(|block| codec::decompress(block))
        .collect();

    // Hata kontrolü ve birleştirme
    let mut output = Vec::new();
    for result in results {
        output.extend(result?);
    }

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parallel_roundtrip_small() {
        let data = b"the cat sat on the mat the cat sat on the mat";
        let compressed = compress(data).unwrap();
        let recovered = decompress(&compressed).unwrap();
        assert_eq!(data.to_vec(), recovered);
    }

    #[test]
    fn test_parallel_roundtrip_large() {
        // 2 MB — birden fazla blok
        let data: Vec<u8> = (0u8..=255).cycle().take(2 * 1024 * 1024).collect();
        let compressed = compress(&data).unwrap();
        let recovered = decompress(&compressed).unwrap();
        assert_eq!(data, recovered);
    }

    #[test]
    fn test_parallel_roundtrip_text() {
        let data = std::fs::read("../corpus/alice29.txt")
            .unwrap_or_else(|_| std::fs::read("corpus/alice29.txt").unwrap());
        let compressed = compress(&data).unwrap();
        let recovered = decompress(&compressed).unwrap();
        assert_eq!(data, recovered);
    }

    #[test]
    fn test_parallel_empty() {
        let compressed = compress(&[]).unwrap();
        let recovered = decompress(&compressed).unwrap();
        assert_eq!(recovered, Vec::<u8>::new());
    }

    #[test]
    fn test_parallel_single_block() {
        // 100 byte — tek blok
        let data = vec![0x42u8; 100];
        let compressed = compress(&data).unwrap();
        let recovered = decompress(&compressed).unwrap();
        assert_eq!(data, recovered);
    }

    #[test]
    fn test_parallel_block_boundary() {
        // Tam 512 KB + 1 byte — iki blok sınırı
        let data = vec![0xAAu8; BLOCK_SIZE + 1];
        let compressed = compress(&data).unwrap();
        let recovered = decompress(&compressed).unwrap();
        assert_eq!(data, recovered);
    }
}