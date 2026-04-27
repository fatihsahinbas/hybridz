/// Stream API
///
/// Rust std::io::Write ve Read trait'lerini implemente eder.
///
/// Analoji: Konveyör bant — veri geldiği anda işle, 512KB dolunca
/// bloğu sıkıştır ve yaz. Tüm veriyi bellekte tutmak gerekmez.
///
/// CompressWriter<W>: Write impl — gelen veriyi bloklar halinde sıkıştırır
/// DecompressReader<R>: Read impl — sıkıştırılmış kaynaktan okuyup açar

use std::io::{self, Read, Write};
use crate::codec;

const BLOCK_SIZE: usize = 512 * 1024; // 512 KB
const MAGIC: u32 = 0x485A5042;        // "HZPB" — parallel.rs ile aynı format

// ── CompressWriter ────────────────────────────────────────────────────────────

pub struct CompressWriter<W: Write> {
    inner: W,
    buf: Vec<u8>,           // henüz sıkıştırılmamış giriş tamponu
    block_sizes: Vec<u32>,  // her bloğun sıkıştırılmış boyutu
    blocks: Vec<Vec<u8>>,   // sıkıştırılmış bloklar (finish'e kadar birikir)
    finished: bool,
}

impl<W: Write> CompressWriter<W> {
    pub fn new(inner: W) -> Self {
        CompressWriter {
            inner,
            buf: Vec::with_capacity(BLOCK_SIZE),
            block_sizes: Vec::new(),
            blocks: Vec::new(),
            finished: false,
        }
    }

    /// Tampondaki veriyi sıkıştırıp blok listesine ekle
    fn flush_block(&mut self) -> io::Result<()> {
        if self.buf.is_empty() {
            return Ok(());
        }
        let compressed = codec::compress(&self.buf)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        self.block_sizes.push(compressed.len() as u32);
        self.blocks.push(compressed);
        self.buf.clear();
        Ok(())
    }

    /// Tüm veriyi yaz ve stream'i kapat
    /// finish() çağrılmazsa header yazılmaz — eksik dosya oluşur!
    pub fn finish(mut self) -> io::Result<W> {
        if self.finished {
            return Ok(self.inner);
        }
        // Kalan tamponu flush et
        self.flush_block()?;

        let block_count = self.blocks.len() as u32;

        // Header: MAGIC + block_count
        self.inner.write_all(&MAGIC.to_le_bytes())?;
        self.inner.write_all(&block_count.to_le_bytes())?;

        // Block size tablosu
        for &size in &self.block_sizes {
            self.inner.write_all(&size.to_le_bytes())?;
        }

        // Blok verileri
        for block in &self.blocks {
            self.inner.write_all(block)?;
        }

        self.finished = true;
        Ok(self.inner)
    }
}

impl<W: Write> Write for CompressWriter<W> {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        let mut written = 0;
        let mut remaining = data;

        while !remaining.is_empty() {
            let space = BLOCK_SIZE - self.buf.len();
            let take = remaining.len().min(space);
            self.buf.extend_from_slice(&remaining[..take]);
            remaining = &remaining[take..];
            written += take;

            if self.buf.len() == BLOCK_SIZE {
                self.flush_block()?;
            }
        }
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

// ── DecompressReader ──────────────────────────────────────────────────────────

pub struct DecompressReader<R: Read> {
    inner: R,
    /// Açılmış ama henüz okunmamış veri
    output_buf: Vec<u8>,
    output_pos: usize,
    /// Henüz okunmamış sıkıştırılmış blok boyutları
    block_sizes: Vec<u32>,
    block_index: usize,
    header_read: bool,
}

impl<R: Read> DecompressReader<R> {
    pub fn new(inner: R) -> Self {
        DecompressReader {
            inner,
            output_buf: Vec::new(),
            output_pos: 0,
            block_sizes: Vec::new(),
            block_index: 0,
            header_read: false,
        }
    }

    /// Header'ı oku: MAGIC + block_count + block_sizes
    fn read_header(&mut self) -> io::Result<()> {
        let mut buf8 = [0u8; 8];
        self.inner.read_exact(&mut buf8)?;

        let magic = u32::from_le_bytes([buf8[0], buf8[1], buf8[2], buf8[3]]);
        if magic != MAGIC {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Geçersiz magic: 0x{:08X}", magic),
            ));
        }

        let block_count = u32::from_le_bytes([buf8[4], buf8[5], buf8[6], buf8[7]]) as usize;

        let mut sizes = vec![0u32; block_count];
        for s in sizes.iter_mut() {
            let mut b4 = [0u8; 4];
            self.inner.read_exact(&mut b4)?;
            *s = u32::from_le_bytes(b4);
        }

        self.block_sizes = sizes;
        self.header_read = true;
        Ok(())
    }

    /// Sıradaki bloğu oku ve aç, output_buf'a yaz
    fn decompress_next_block(&mut self) -> io::Result<bool> {
        if self.block_index >= self.block_sizes.len() {
            return Ok(false); // Tüm bloklar tükendi
        }

        let size = self.block_sizes[self.block_index] as usize;
        self.block_index += 1;

        let mut compressed = vec![0u8; size];
        self.inner.read_exact(&mut compressed)?;

        let decompressed = codec::decompress(&compressed)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        self.output_buf = decompressed;
        self.output_pos = 0;
        Ok(true)
    }
}

impl<R: Read> Read for DecompressReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if !self.header_read {
            self.read_header()?;
        }

        let mut total_read = 0;

        while total_read < buf.len() {
            // output_buf'ta bekleyen veri var mı?
            let available = self.output_buf.len() - self.output_pos;
            if available > 0 {
                let take = (buf.len() - total_read).min(available);
                buf[total_read..total_read + take]
                    .copy_from_slice(&self.output_buf[self.output_pos..self.output_pos + take]);
                self.output_pos += take;
                total_read += take;
            } else {
                // Yeni blok aç
                let has_more = self.decompress_next_block()?;
                if !has_more {
                    break; // EOF
                }
            }
        }

        Ok(total_read)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};

    fn compress_stream(data: &[u8]) -> Vec<u8> {
        let buf = Vec::new();
        let mut writer = CompressWriter::new(buf);
        writer.write_all(data).unwrap();
        writer.finish().unwrap()
    }

    fn decompress_stream(data: &[u8]) -> Vec<u8> {
        let mut reader = DecompressReader::new(data);
        let mut out = Vec::new();
        reader.read_to_end(&mut out).unwrap();
        out
    }

    #[test]
    fn test_stream_roundtrip_small() {
        let data = b"the cat sat on the mat the cat sat on the mat";
        let compressed = compress_stream(data);
        let recovered = decompress_stream(&compressed);
        assert_eq!(data.to_vec(), recovered);
    }

    #[test]
    fn test_stream_roundtrip_large() {
        // 2 MB — birden fazla blok
        let data: Vec<u8> = (0u8..=255).cycle().take(2 * 1024 * 1024).collect();
        let compressed = compress_stream(&data);
        let recovered = decompress_stream(&compressed);
        assert_eq!(data, recovered);
    }

    #[test]
    fn test_stream_roundtrip_text() {
        let data = std::fs::read("../corpus/alice29.txt")
            .unwrap_or_else(|_| std::fs::read("corpus/alice29.txt").unwrap());
        let compressed = compress_stream(&data);
        let recovered = decompress_stream(&compressed);
        assert_eq!(data, recovered);
    }

    #[test]
    fn test_stream_block_boundary() {
        // Tam 512 KB + 1 byte
        let data = vec![0xBBu8; BLOCK_SIZE + 1];
        let compressed = compress_stream(&data);
        let recovered = decompress_stream(&compressed);
        assert_eq!(data, recovered);
    }

    #[test]
    fn test_stream_incremental_write() {
        // Küçük küçük write — stream API'nin asıl kullanım senaryosu
        let data: Vec<u8> = (0u8..=255).cycle().take(10_000).collect();
        let buf = Vec::new();
        let mut writer = CompressWriter::new(buf);
        for chunk in data.chunks(137) {  // 137 byte'lık parçalar
            writer.write_all(chunk).unwrap();
        }
        let compressed = writer.finish().unwrap();
        let recovered = decompress_stream(&compressed);
        assert_eq!(data, recovered);
    }

    #[test]
    fn test_stream_incremental_read() {
        // Küçük küçük read
        let data: Vec<u8> = (0u8..=255).cycle().take(10_000).collect();
        let compressed = compress_stream(&data);
        let mut reader = DecompressReader::new(compressed.as_slice());
        let mut out = Vec::new();
        let mut chunk = [0u8; 300];  // 300 byte'lık parçalar
        loop {
            let n = reader.read(&mut chunk).unwrap();
            if n == 0 { break; }
            out.extend_from_slice(&chunk[..n]);
        }
        assert_eq!(data, out);
    }

    #[test]
    fn test_stream_parallel_format_compatible() {
        // Stream ve parallel aynı formatı kullanıyor — birbirinin çıktısını açabilmeli
        use crate::parallel;
        let data: Vec<u8> = (0u8..=255).cycle().take(10_000).collect();

        // parallel ile sıkıştır, stream ile aç
        let compressed = parallel::compress(&data).unwrap();
        let recovered = decompress_stream(&compressed);
        assert_eq!(data, recovered);

        // stream ile sıkıştır, parallel ile aç
        let compressed2 = compress_stream(&data);
        let recovered2 = parallel::decompress(&compressed2).unwrap();
        assert_eq!(data, recovered2);
    }
}