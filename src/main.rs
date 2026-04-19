use std::fs;
use std::time::Instant;
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        print_usage();
        return;
    }

    match args[1].as_str() {
        "compress" => {
            if args.len() < 4 {
                eprintln!("Kullanım: hybridz compress <girdi> <çıktı>");
                return;
            }
            cmd_compress(&args[2], &args[3]);
        }
        "decompress" => {
            if args.len() < 4 {
                eprintln!("Kullanım: hybridz decompress <girdi> <çıktı>");
                return;
            }
            cmd_decompress(&args[2], &args[3]);
        }
        "bench" => {
            if args.len() < 3 {
                eprintln!("Kullanım: hybridz bench <girdi|dizin>");
                return;
            }
            cmd_bench(&args[2]);
        }
        "corpus" => {
            if args.len() < 3 {
                eprintln!("Kullanım: hybridz corpus <dizin>");
                return;
            }
            cmd_corpus(&args[2]);
        }
        _ => {
            eprintln!("Bilinmeyen komut: {}", args[1]);
            print_usage();
        }
    }
}

fn print_usage() {
    println!("HybridZ Sıkıştırma Kütüphanesi");
    println!();
    println!("Kullanım:");
    println!("  hybridz compress   <girdi> <çıktı>");
    println!("  hybridz decompress <girdi> <çıktı>");
    println!("  hybridz bench      <dosya>");
    println!("  hybridz corpus     <dizin>   ← Canterbury corpus karşılaştırması");
}

// ─── bench: tek dosya ────────────────────────────────────────────────────────

fn cmd_bench(input_path: &str) {
    let data = fs::read(input_path).unwrap_or_else(|e| {
        eprintln!("Dosya okunamadı '{}': {}", input_path, e);
        std::process::exit(1);
    });

    println!("=== HybridZ Benchmark ===");
    println!("Dosya       : {}", input_path);
    println!("Girdi boyutu: {} byte ({:.1} KB)", data.len(), data.len() as f64 / 1024.0);
    println!();

    let t0 = Instant::now();
    let compressed = hybridz::compress(&data).unwrap_or_else(|e| {
        eprintln!("Sıkıştırma hatası: {}", e);
        std::process::exit(1);
    });
    let compress_time = t0.elapsed();

    let ratio         = compressed.len() as f64 / data.len() as f64 * 100.0;
    let saved         = 100.0 - ratio;
    let bits_per_byte = compressed.len() as f64 * 8.0 / data.len() as f64;
    let speed_mb      = data.len() as f64 / 1_048_576.0 / compress_time.as_secs_f64();

    println!("[ Sıkıştırma ]");
    println!("  Çıktı   : {} byte ({:.1} KB)", compressed.len(), compressed.len() as f64 / 1024.0);
    println!("  Oran    : {:.1}%  ({:.1}% tasarruf)", ratio, saved);
    println!("  Bit/byte: {:.3}", bits_per_byte);
    println!("  Süre    : {:.1?}", compress_time);
    println!("  Hız     : {:.2} MB/s", speed_mb);
    println!();

    let t1 = Instant::now();
    let decompressed = hybridz::decompress(&compressed).unwrap_or_else(|e| {
        eprintln!("Açma hatası: {}", e);
        std::process::exit(1);
    });
    let decompress_time = t1.elapsed();
    let dspeed_mb = decompressed.len() as f64 / 1_048_576.0 / decompress_time.as_secs_f64();

    println!("[ Açma ]");
    println!("  Çıktı   : {} byte", decompressed.len());
    println!("  Süre    : {:.1?}", decompress_time);
    println!("  Hız     : {:.2} MB/s", dspeed_mb);
    println!();

    if decompressed == data {
        println!("[ Roundtrip ] ✅ Veri bütünlüğü doğrulandı");
    } else {
        println!("[ Roundtrip ] ❌ HATA!");
        for (i, (a, b)) in data.iter().zip(decompressed.iter()).enumerate() {
            if a != b {
                println!("  İlk fark: byte {} → orijinal=0x{:02X}, açılan=0x{:02X}", i, a, b);
                break;
            }
        }
        if decompressed.len() != data.len() {
            println!("  Boyut farkı: orijinal={}, açılan={}", data.len(), decompressed.len());
        }
    }
}

// ─── corpus: tüm dizini tara ─────────────────────────────────────────────────

fn cmd_corpus(dir_path: &str) {
    let entries = fs::read_dir(dir_path).unwrap_or_else(|e| {
        eprintln!("Dizin açılamadı '{}': {}", dir_path, e);
        std::process::exit(1);
    });

    let mut files: Vec<std::path::PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_file())
        .collect();
    files.sort();

    println!("=== HybridZ Canterbury Corpus ===");
    println!();
    println!("{:<20} {:>8} {:>8} {:>7} {:>7} {:>6}",
        "Dosya", "Orijinal", "Sıkışık", "Oran%", "Tasarruf", "RT");
    println!("{}", "-".repeat(62));

    let mut total_orig = 0usize;
    let mut total_comp = 0usize;

    for path in &files {
        let name = path.file_name().unwrap().to_string_lossy();
        let data = match fs::read(path) {
            Ok(d) => d,
            Err(e) => {
                println!("{:<20} okuma hatası: {}", name, e);
                continue;
            }
        };
        if data.is_empty() { continue; }

        let compressed = match hybridz::compress(&data) {
            Ok(c) => c,
            Err(e) => {
                println!("{:<20} sıkıştırma hatası: {}", name, e);
                continue;
            }
        };

        let decompressed = match hybridz::decompress(&compressed) {
            Ok(d) => d,
            Err(e) => {
                println!("{:<20} açma hatası: {}", name, e);
                continue;
            }
        };

        let ratio   = compressed.len() as f64 / data.len() as f64 * 100.0;
        let saved   = 100.0 - ratio;
        let rt_mark = if decompressed == data { "✅" } else { "❌" };

        println!("{:<20} {:>8} {:>8} {:>6.1}% {:>6.1}%  {}",
            name, data.len(), compressed.len(), ratio, saved, rt_mark);

        total_orig += data.len();
        total_comp += compressed.len();
    }

    println!("{}", "-".repeat(62));
    let total_ratio = total_comp as f64 / total_orig as f64 * 100.0;
    let total_saved = 100.0 - total_ratio;
    println!("{:<20} {:>8} {:>8} {:>6.1}% {:>6.1}%",
        "TOPLAM", total_orig, total_comp, total_ratio, total_saved);
}

// ─── compress / decompress ───────────────────────────────────────────────────

fn cmd_compress(input_path: &str, output_path: &str) {
    let data = fs::read(input_path).unwrap_or_else(|e| {
        eprintln!("Dosya okunamadı '{}': {}", input_path, e);
        std::process::exit(1);
    });

    let t0 = Instant::now();
    let compressed = hybridz::compress(&data).unwrap_or_else(|e| {
        eprintln!("Sıkıştırma hatası: {}", e);
        std::process::exit(1);
    });
    let elapsed = t0.elapsed();

    fs::write(output_path, &compressed).unwrap_or_else(|e| {
        eprintln!("Dosya yazılamadı '{}': {}", output_path, e);
        std::process::exit(1);
    });

    let ratio = compressed.len() as f64 / data.len() as f64 * 100.0;
    let saved = 100.0 - ratio;
    println!("Sıkıştırıldı: {} → {} byte ({:.1}%, {:.1}% tasarruf)  [{:.1?}]",
        data.len(), compressed.len(), ratio, saved, elapsed);
}

fn cmd_decompress(input_path: &str, output_path: &str) {
    let data = fs::read(input_path).unwrap_or_else(|e| {
        eprintln!("Dosya okunamadı '{}': {}", input_path, e);
        std::process::exit(1);
    });

    let t0 = Instant::now();
    let decompressed = hybridz::decompress(&data).unwrap_or_else(|e| {
        eprintln!("Açma hatası: {}", e);
        std::process::exit(1);
    });
    let elapsed = t0.elapsed();

    fs::write(output_path, &decompressed).unwrap_or_else(|e| {
        eprintln!("Dosya yazılamadı '{}': {}", output_path, e);
        std::process::exit(1);
    });

    println!("Açıldı: {} → {} byte  [{:.1?}]",
        data.len(), decompressed.len(), elapsed);
}
