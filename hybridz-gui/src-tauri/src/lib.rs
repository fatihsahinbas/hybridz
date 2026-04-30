use serde::Serialize;
use std::fs;
use std::time::Instant;

#[derive(Serialize)]
pub struct CompressResult {
    pub original_size: usize,
    pub output_size: usize,
    pub savings_pct: f64,
    pub speed_mb: f64,
    pub pipeline_id: u8,
    pub output_bytes: Vec<u8>,
    pub original_ext: String,
}

#[derive(Serialize)]
pub struct BenchResult {
    pub original_size: usize,
    pub compressed_size: usize,
    pub savings_pct: f64,
    pub bits_per_byte: f64,
    pub compress_ms: f64,
    pub compress_mb: f64,
    pub decompress_ms: f64,
    pub decompress_mb: f64,
    pub roundtrip_ok: bool,
    pub pipeline_id: u8,
}

#[derive(Serialize)]
pub struct FileStats {
    pub filename: String,
    pub original_size: usize,
    pub compressed_size: usize,
    pub roundtrip_ok: bool,
}

#[derive(Serialize)]
pub struct ArchiveEntry {
    pub name: String,
    pub size: u64,
    pub is_dir: bool,
}

// PBKDF2-SHA256 ile 32 byte anahtar turet
fn derive_key(password: &[u8], salt: &[u8]) -> [u8; 32] {
    use hmac::Hmac;
    use pbkdf2::pbkdf2;
    use sha2::Sha256;
    let mut key = [0u8; 32];
    pbkdf2::<Hmac<Sha256>>(password, salt, 100_000, &mut key).expect("PBKDF2 hatasi");
    key
}
// .hz wrapper formatı v2:
// [4 byte magic: 0x48 0x5A 0x32 0x00] [1 byte ext_len] [ext_len byte UTF-8 uzantı] [compress() çıktısı]
// Eski format (magic yok): direkt compress() çıktısı — geriye dönük uyumlu

const HZ_MAGIC: [u8; 4] = [0x48, 0x5A, 0x32, 0x00];

fn wrap_with_ext(compressed: Vec<u8>, ext: &str) -> Vec<u8> {
    let ext_bytes = ext.as_bytes();
    let ext_len = ext_bytes.len().min(255) as u8;
    let mut out = Vec::with_capacity(4 + 1 + ext_len as usize + compressed.len());
    out.extend_from_slice(&HZ_MAGIC);
    out.push(ext_len);
    out.extend_from_slice(&ext_bytes[..ext_len as usize]);
    out.extend(compressed);
    out
}

fn unwrap_ext(data: &[u8]) -> (Vec<u8>, String) {
    if data.len() >= 5 && data[..4] == HZ_MAGIC {
        let ext_len = data[4] as usize;
        let ext_end = 5 + ext_len;
        if data.len() >= ext_end {
            let ext = String::from_utf8_lossy(&data[5..ext_end]).to_string();
            let payload = data[ext_end..].to_vec();
            return (payload, ext);
        }
    }
    // Eski format — uzantı yok
    (data.to_vec(), String::new())
}
pub mod cmds {
    use super::*;
    use std::io::Read;

    #[tauri::command]
    pub fn compress_file(path: String) -> Result<CompressResult, String> {
        let p = std::path::Path::new(&path);
        let ext = p
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_string();
        let data = fs::read(&path).map_err(|e| e.to_string())?;
        let original_size = data.len();
        let t0 = Instant::now();
        let compressed = hybridz::compress(&data).map_err(|e| e.to_string())?;
        let elapsed = t0.elapsed();
        let pipeline_id = if compressed.is_empty() {
            0
        } else {
            compressed[0]
        };
        let wrapped = wrap_with_ext(compressed, &ext);
        let output_size = wrapped.len();
        let savings_pct = (1.0 - output_size as f64 / original_size as f64) * 100.0;
        let speed_mb = original_size as f64 / 1_048_576.0 / elapsed.as_secs_f64();
        Ok(CompressResult {
            original_size,
            output_size,
            savings_pct,
            speed_mb,
            pipeline_id,
            output_bytes: wrapped,
            original_ext: ext,
        })
    }

    #[tauri::command]
    pub fn decompress_file(path: String) -> Result<CompressResult, String> {
        let raw = fs::read(&path).map_err(|e| e.to_string())?;
        let original_size = raw.len();
        let (payload, ext) = unwrap_ext(&raw);
        let pipeline_id = if payload.is_empty() { 0 } else { payload[0] };
        let t0 = Instant::now();
        let decompressed = hybridz::decompress(&payload).map_err(|e| e.to_string())?;
        let elapsed = t0.elapsed();
        let output_size = decompressed.len();
        let savings_pct = (1.0 - original_size as f64 / output_size as f64) * 100.0;
        let speed_mb = output_size as f64 / 1_048_576.0 / elapsed.as_secs_f64();
        Ok(CompressResult {
            original_size,
            output_size,
            savings_pct,
            speed_mb,
            pipeline_id,
            output_bytes: decompressed,
            original_ext: ext,
        })
    }

    #[tauri::command]
    pub fn bench_file(path: String) -> Result<BenchResult, String> {
        let data = fs::read(&path).map_err(|e| e.to_string())?;
        let original_size = data.len();
        let t0 = Instant::now();
        let compressed = hybridz::compress(&data).map_err(|e| e.to_string())?;
        let compress_ms = t0.elapsed().as_secs_f64() * 1000.0;
        let compressed_size = compressed.len();
        let pipeline_id = if compressed.is_empty() {
            0
        } else {
            compressed[0]
        };
        let savings_pct = (1.0 - compressed_size as f64 / original_size as f64) * 100.0;
        let bits_per_byte = compressed_size as f64 * 8.0 / original_size as f64;
        let compress_mb = original_size as f64 / 1_048_576.0 / (compress_ms / 1000.0);
        let t1 = Instant::now();
        let decompressed = hybridz::decompress(&compressed).map_err(|e| e.to_string())?;
        let decompress_ms = t1.elapsed().as_secs_f64() * 1000.0;
        let decompress_mb = decompressed.len() as f64 / 1_048_576.0 / (decompress_ms / 1000.0);
        let roundtrip_ok = decompressed == data;
        Ok(BenchResult {
            original_size,
            compressed_size,
            savings_pct,
            bits_per_byte,
            compress_ms,
            compress_mb,
            decompress_ms,
            decompress_mb,
            roundtrip_ok,
            pipeline_id,
        })
    }

    #[tauri::command]
    pub fn scan_corpus(dir: String) -> Result<Vec<FileStats>, String> {
        let entries = fs::read_dir(&dir).map_err(|e| e.to_string())?;
        let mut files: Vec<std::path::PathBuf> = entries
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.is_file())
            .collect();
        files.sort();
        let mut results = Vec::new();
        for path in files {
            let filename = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let data = match fs::read(&path) {
                Ok(d) if !d.is_empty() => d,
                _ => continue,
            };
            let original_size = data.len();
            let compressed = match hybridz::compress(&data) {
                Ok(c) => c,
                Err(_) => continue,
            };
            let compressed_size = compressed.len();
            let decompressed = match hybridz::decompress(&compressed) {
                Ok(d) => d,
                Err(_) => {
                    results.push(FileStats {
                        filename,
                        original_size,
                        compressed_size,
                        roundtrip_ok: false,
                    });
                    continue;
                }
            };
            results.push(FileStats {
                filename,
                original_size,
                compressed_size,
                roundtrip_ok: decompressed == data,
            });
        }
        Ok(results)
    }

    #[tauri::command]
    pub fn compress_file_bytes(data: Vec<u8>) -> Result<CompressResult, String> {
        let original_size = data.len();
        let t0 = Instant::now();
        let compressed = hybridz::compress(&data).map_err(|e| e.to_string())?;
        let elapsed = t0.elapsed();
        let output_size = compressed.len();
        let savings_pct = (1.0 - output_size as f64 / original_size as f64) * 100.0;
        let speed_mb = original_size as f64 / 1_048_576.0 / elapsed.as_secs_f64();
        let pipeline_id = if compressed.is_empty() {
            0
        } else {
            compressed[0]
        };
        Ok(CompressResult {
            original_size,
            output_size,
            savings_pct,
            speed_mb,
            pipeline_id,
            output_bytes: compressed,
            original_ext: String::new(),
        })
    }

    #[tauri::command]
    pub fn save_bytes(path: String, bytes: Vec<u8>) -> Result<(), String> {
        fs::write(&path, &bytes).map_err(|e| e.to_string())
    }
    #[tauri::command]
    pub fn decompress_to_file(src: String, dest: String) -> Result<(), String> {
        let raw = fs::read(&src).map_err(|e| e.to_string())?;
        let (payload, _ext) = unwrap_ext(&raw);
        let decompressed = hybridz::decompress(&payload).map_err(|e| e.to_string())?;
        fs::write(&dest, &decompressed).map_err(|e| e.to_string())
    }

    #[tauri::command]
    pub fn decrypt_to_file(src: String, dest: String, password: String) -> Result<(), String> {
        let data = fs::read(&src).map_err(|e| e.to_string())?;
        if data.len() < 61 {
            return Err("Gecersiz sifreli dosya".to_string());
        }
        let algo = data[0];
        let salt = &data[1..33];
        let nonce = &data[33..45];
        let ct = &data[45..];
        let key = derive_key(password.as_bytes(), salt);
        let compressed = match algo {
            0x01 => {
                use aes_gcm::{aead::Aead, Aes256Gcm, KeyInit, Nonce};
                let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| e.to_string())?;
                cipher
                    .decrypt(Nonce::from_slice(nonce), ct)
                    .map_err(|_| "Yanlis sifre".to_string())?
            }
            0x02 => {
                use chacha20poly1305::{aead::Aead, ChaCha20Poly1305, KeyInit, Nonce};
                let cipher = ChaCha20Poly1305::new_from_slice(&key).map_err(|e| e.to_string())?;
                cipher
                    .decrypt(Nonce::from_slice(nonce), ct)
                    .map_err(|_| "Yanlis sifre".to_string())?
            }
            _ => return Err(format!("Bilinmeyen algoritma: {}", algo)),
        };
        let decompressed = hybridz::decompress(&compressed).map_err(|e| e.to_string())?;
        fs::write(&dest, &decompressed).map_err(|e| e.to_string())
    }

    /// Dosyanin sifreli olup olmadigini kontrol et
    #[tauri::command]
    pub fn is_encrypted(path: String) -> Result<bool, String> {
        let data = fs::read(&path).map_err(|e| e.to_string())?;
        if data.is_empty() {
            return Ok(false);
        }
        Ok(data[0] == 0x01 || data[0] == 0x02)
    }
    // --- Sifreleme ---
    // Dosya formati: [1 byte algo] [32 byte salt] [12 byte nonce] [sifreli+sikistirilmis veri]
    // algo: 0x01 = AES-256-GCM, 0x02 = ChaCha20-Poly1305

    #[tauri::command]
    pub fn encrypt_file(
        path: String,
        password: String,
        algo: u8,
    ) -> Result<CompressResult, String> {
        let data = fs::read(&path).map_err(|e| e.to_string())?;
        let original_size = data.len();
        let t0 = Instant::now();

        let compressed = hybridz::compress(&data).map_err(|e| e.to_string())?;
        let salt: [u8; 32] = rand::random();
        let key = derive_key(password.as_bytes(), &salt);

        let output_bytes = match algo {
            0x01 => {
                use aes_gcm::{
                    aead::{Aead, AeadCore, OsRng},
                    Aes256Gcm, KeyInit,
                };
                let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| e.to_string())?;
                let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
                let ct = cipher
                    .encrypt(&nonce, compressed.as_ref())
                    .map_err(|e| e.to_string())?;
                let mut out = vec![0x01u8];
                out.extend_from_slice(&salt);
                out.extend_from_slice(&nonce);
                out.extend(ct);
                out
            }
            0x02 => {
                use chacha20poly1305::{
                    aead::{Aead, AeadCore, OsRng},
                    ChaCha20Poly1305, KeyInit,
                };
                let cipher = ChaCha20Poly1305::new_from_slice(&key).map_err(|e| e.to_string())?;
                let nonce = ChaCha20Poly1305::generate_nonce(&mut OsRng);
                let ct = cipher
                    .encrypt(&nonce, compressed.as_ref())
                    .map_err(|e| e.to_string())?;
                let mut out = vec![0x02u8];
                out.extend_from_slice(&salt);
                out.extend_from_slice(&nonce);
                out.extend(ct);
                out
            }
            _ => return Err(format!("Bilinmeyen algoritma: {}", algo)),
        };

        let elapsed = t0.elapsed();
        let output_size = output_bytes.len();
        let savings_pct = (1.0 - output_size as f64 / original_size as f64) * 100.0;
        let speed_mb = original_size as f64 / 1_048_576.0 / elapsed.as_secs_f64();
        Ok(CompressResult {
            original_size,
            output_size,
            savings_pct,
            speed_mb,
            pipeline_id: algo,
            output_bytes,
            original_ext: String::new(),
        })
    }

    #[tauri::command]
    pub fn decrypt_file(path: String, password: String) -> Result<CompressResult, String> {
        let data = fs::read(&path).map_err(|e| e.to_string())?;
        // minimum: 1 + 32 + 12 + 16 (tag) = 61 byte
        if data.len() < 61 {
            return Err("Gecersiz sifreli dosya".to_string());
        }
        let original_size = data.len();
        let t0 = Instant::now();

        let algo = data[0];
        let salt = &data[1..33];
        let nonce = &data[33..45];
        let ct = &data[45..];
        let key = derive_key(password.as_bytes(), salt);

        let compressed = match algo {
            0x01 => {
                use aes_gcm::{aead::Aead, Aes256Gcm, KeyInit, Nonce};
                let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| e.to_string())?;
                cipher
                    .decrypt(Nonce::from_slice(nonce), ct)
                    .map_err(|_| "Sifre cozme hatasi: yanlis sifre veya bozuk dosya".to_string())?
            }
            0x02 => {
                use chacha20poly1305::{aead::Aead, ChaCha20Poly1305, KeyInit, Nonce};
                let cipher = ChaCha20Poly1305::new_from_slice(&key).map_err(|e| e.to_string())?;
                cipher
                    .decrypt(Nonce::from_slice(nonce), ct)
                    .map_err(|_| "Sifre cozme hatasi: yanlis sifre veya bozuk dosya".to_string())?
            }
            _ => return Err(format!("Bilinmeyen algoritma: {}", algo)),
        };

        let decompressed = hybridz::decompress(&compressed).map_err(|e| e.to_string())?;
        let elapsed = t0.elapsed();
        let output_size = decompressed.len();
        let savings_pct = (1.0 - original_size as f64 / output_size as f64) * 100.0;
        let speed_mb = output_size as f64 / 1_048_576.0 / elapsed.as_secs_f64();
        Ok(CompressResult {
            original_size,
            output_size,
            savings_pct,
            speed_mb,
            pipeline_id: algo,
            output_bytes: decompressed,
            original_ext: String::new(),
        })
    }

    // --- Arsiv destegi ---

    #[tauri::command]
    pub fn list_archive(path: String) -> Result<Vec<ArchiveEntry>, String> {
        let p = std::path::Path::new(&path);
        let ext = p
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        match ext.as_str() {
            "zip" => list_zip(&path),
            "7z" => list_7z(&path),
            _ => Err(format!("Desteklenmeyen format: .{}", ext)),
        }
    }

    #[tauri::command]
    pub fn extract_entry(path: String, entry_name: String) -> Result<Vec<u8>, String> {
        let p = std::path::Path::new(&path);
        let ext = p
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        match ext.as_str() {
            "zip" => extract_zip(&path, &entry_name),
            "7z" => extract_7z(&path, &entry_name),
            _ => Err(format!("Desteklenmeyen format: .{}", ext)),
        }
    }

    fn list_zip(path: &str) -> Result<Vec<ArchiveEntry>, String> {
        let file = fs::File::open(path).map_err(|e| e.to_string())?;
        let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
        let mut entries = Vec::new();
        for i in 0..archive.len() {
            let entry = archive.by_index(i).map_err(|e| e.to_string())?;
            entries.push(ArchiveEntry {
                name: entry.name().to_string(),
                size: entry.size(),
                is_dir: entry.is_dir(),
            });
        }
        Ok(entries)
    }

    fn extract_zip(path: &str, entry_name: &str) -> Result<Vec<u8>, String> {
        let file = fs::File::open(path).map_err(|e| e.to_string())?;
        let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
        let mut entry = archive.by_name(entry_name).map_err(|e| e.to_string())?;
        let mut buf = Vec::new();
        entry.read_to_end(&mut buf).map_err(|e| e.to_string())?;
        Ok(buf)
    }

    fn list_7z(path: &str) -> Result<Vec<ArchiveEntry>, String> {
        use sevenz_rust2::{Password, SevenZReader};
        use std::io::{Seek, SeekFrom};
        let mut file = fs::File::open(path).map_err(|e| e.to_string())?;
        let len = file.seek(SeekFrom::End(0)).map_err(|e| e.to_string())?;
        file.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
        let mut reader =
            SevenZReader::new(file, len, Password::empty()).map_err(|e| e.to_string())?;
        let mut entries = Vec::new();
        reader
            .for_each_entries(|entry, _reader| {
                entries.push(ArchiveEntry {
                    name: entry.name().to_string(),
                    size: entry.size(),
                    is_dir: entry.is_directory(),
                });
                Ok(true)
            })
            .map_err(|e| e.to_string())?;
        Ok(entries)
    }

    fn extract_7z(path: &str, entry_name: &str) -> Result<Vec<u8>, String> {
        use sevenz_rust2::Error as SzError;
        let mut result: Option<Vec<u8>> = None;
        sevenz_rust2::decompress_file_with_extract_fn(path, "", |entry, reader, _dest| {
            if entry.name() == entry_name && !entry.is_directory() {
                let mut buf = Vec::new();
                std::io::copy(reader, &mut buf).map_err(SzError::io)?;
                result = Some(buf);
            }
            Ok(true)
        })
        .map_err(|e| e.to_string())?;
        result.ok_or_else(|| format!("Dosya bulunamadi: {}", entry_name))
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            cmds::compress_file,
            cmds::decompress_file,
            cmds::bench_file,
            cmds::scan_corpus,
            cmds::save_bytes,
            cmds::list_archive,
            cmds::extract_entry,
            cmds::compress_file_bytes,
            cmds::encrypt_file,
            cmds::decrypt_file,
            cmds::is_encrypted,
            cmds::decompress_to_file,
            cmds::decrypt_to_file,
        ])
        .run(tauri::generate_context!())
        .expect("Tauri baslatılamadı");
}
