# HybridZ

**HybridZ**, Rust ile yazılmış içerik-adaptif hibrit bir sıkıştırma kütüphanesidir. Veriye bakar, en uygun transform + entropy coding kombinasyonunu seçer ve uygular.

> ⚠️ **Alpha — API ve binary format değişebilir.** Üretimde kullanmayın.

🔄 [English README](README.en.md)

## Neden?

Klasik sıkıştırıcılar (gzip, bzip2, zstd) tek bir stratejiye sabitlenir. HybridZ, veri tipini (metin, sayısal, tekrarlı, executable, binary, sabit-kayıt) önce **analiz eder**, sonra ona uygun bir **transform pipeline** + **entropy coder** seçer. Farklı veri tiplerinde farklı referans sıkıştırıcıları geçmeyi hedefler.

## Benchmark — Canterbury Corpus

| Dosya          | Boyut   | HybridZ    | gzip*  | bzip2* |
| -------------- | ------: | ---------: | -----: | -----: |
| alice29.txt    |  148 KB | **65.7%**  |  64.4% |  71.6% |
| asyoulik.txt   |  122 KB | **62.6%**  |  59.4% |  68.4% |
| cp.html        |   24 KB | **65.9%**  |  68.9% |  75.9% |
| fields.c       |   11 KB | **67.8%**  |  72.1% |  75.9% |
| kennedy.xls    | 1006 KB | **89.0%**  |  41.0% |  47.0% |
| lcet10.txt     |  417 KB | **69.0%**  |  63.4% |  72.3% |
| plrabn12.txt   |  471 KB | **63.9%**  |  59.0% |  69.0% |
| ptt5           |  501 KB | **86.3%**  |  91.0% |  93.0% |
| sum            |   37 KB | **62.5%**  |  64.1% |  67.9% |
| xargs.1        |    4 KB | **52.4%**  |  63.5% |  69.8% |
| **TOPLAM**     |  2.7 MB | **78.0%**  |  ~62%  |  ~68%  |

\* gzip/bzip2 referans değerleri Canterbury Corpus sonuçlarından alınmıştır; yüzde = tasarruf oranı (yüksek iyidir).

**Öne çıkan:** `kennedy.xls` üzerinde %89.0 tasarrufla bzip2'yi **42 puan** geride bırakır.

## Benchmark — Silesia Corpus (202 MB)

| Dosya      | Boyut    | HybridZ     | bzip2* | Δ         |
| ---------- | -------: | ----------: | -----: | --------: |
| dickens    |  9.7 MB  | **69.1%**   |   73%  |   -3.9    |
| mozilla    | 48.8 MB  | **61.6%**   |   63%  |   -1.4    |
| mr         |  9.5 MB  | **73.2%** ✅ |   75%  |   -1.8    |
| nci        | 32.0 MB  | **94.4%** 🎯 |   85%  | **+9.4**  |
| ooffice    |  5.9 MB  | **55.5%**   |   59%  |   -3.5    |
| osdb       |  9.6 MB  | **71.1%** ✅ |   64%  | **+7.1**  |
| reymont    |  6.3 MB  | **79.5%** 🎯 |   75%  | **+4.5**  |
| samba      | 20.6 MB  | **75.8%** ✅ |   73%  | **+2.8**  |
| sao        |  6.9 MB  | **29.3%**   |   35%  |   -5.7    |
| webster    | 39.5 MB  | **78.0%** ✅ |   73%  | **+5.0**  |
| x-ray      |  8.1 MB  | **45.6%** ✅ |   35%  | **+10.6** |
| xml        |  5.1 MB  | **89.8%** 🎯 |   75%  | **+14.8** |
| **TOPLAM** | 202 MB   | **72.2%**   |  ~72%  |  **+0.2** |

\* bzip2 Silesia referans değerleri; yüzde = tasarruf oranı.

**Öne çıkanlar:** `nci` +9.4, `xml` +14.8, `x-ray` +10.6, `webster` +5.0 puan bzip2'yi geçer.

## Hız (release build, single-core)

| Dosya        | Compress         | Decompress        |
| ------------ | ---------------: | ----------------: |
| alice29.txt  | ~4.6 MB/s        | ~23.5 MB/s        |
| kennedy.xls  | ~5.5 MB/s        | ~24.6 MB/s        |
| ptt5         | ~89.6 MB/s       | ~221.8 MB/s       |
| notepad.exe  | ~5.9 MB/s        | ~18.4 MB/s        |

## Nasıl Çalışır?
Veri
│
▼
┌──────────────────────────┐
│ Content Analyzer         │
│ entropy / text_ratio /   │
│ delta / rle / bcj /      │
│ deinterleave score       │
└──────────────────────────┘
│
▼
┌──────────────────────────┐
│ Transform Pipeline       │
│ BWT → MTF → RLE          │
│ BWT → MTF (± BCJ)        │
│ Delta, RLE, DeltaRle     │
│ DeIlv (sütun-bazlı)      │
│ (veya transform yok)     │
└──────────────────────────┘
│
▼
┌──────────────────────────┐
│ Entropy Coder            │
│ Huffman ⚡ ANS            │
│ (küçük olan kazanır)     │
└──────────────────────────┘
│
▼
Sıkıştırılmış veri
`analyzer.rs`, Shannon entropisi + bigram tekrar oranı + text byte oranı + delta/rle/bcj/deinterleave uygunluk skorlarına bakarak veriyi kategorize eder. BWT+MTF ve BCJ+BWT+MTF dallarında hem Huffman hem ANS denenir, çıktısı küçük olan seçilir. Sabit-kayıt binary dosyalar (sao, DICOM, sensör logları) için sütun-bazlı Huffman (DeIlv) uygulanır.

## Pipeline ID'leri

Her sıkıştırılmış blok, header'ın ilk byte'ında bir pipeline ID taşır. Decoder buna bakıp doğru ters dönüşümü uygular.

| ID   | Transform             | Entropy               |
| :--- | :-------------------- | :-------------------- |
| 0x00 | (yok)                 | Huffman               |
| 0x01 | Delta                 | Huffman               |
| 0x02 | RLE                   | Huffman               |
| 0x03 | Delta + RLE           | Huffman               |
| 0x04 | BWT + MTF             | Huffman               |
| 0x05 | BWT + MTF             | ANS                   |
| 0x06 | BCJ + BWT + MTF       | Huffman               |
| 0x07 | BCJ + BWT + MTF       | ANS                   |
| 0x08 | DeIlv (sütun-bazlı)   | Huffman (per-column)  |
| 0x09 | BWT + MTF + RLE       | Huffman / ANS         |

## Kullanım (CLI)

```powershell
# Build
cargo build --release

# Tek dosya sıkıştır/aç
.\target\release\hybridz.exe compress   input.txt compressed.hz
.\target\release\hybridz.exe decompress compressed.hz restored.txt

# Benchmark (sıkıştır, aç, roundtrip doğrula, rapor et)
.\target\release\hybridz.exe bench      input.txt

# Tüm dizini sıkıştır, corpus tablosu üret
.\target\release\hybridz.exe corpus     .\corpus\
```

## Kütüphane Olarak

```rust
use hybridz::{compress, decompress};

// Tek seferlik (in-memory)
let original = b"the cat sat on the mat the cat sat on the mat";
let encoded  = compress(original)?;
let decoded  = decompress(&encoded)?;
assert_eq!(original.to_vec(), decoded);

// Paralel (büyük dosyalar için)
use hybridz::parallel;
let encoded = parallel::compress(&data)?;
let decoded = parallel::decompress(&encoded)?;

// Stream API (Read/Write trait'leri)
use hybridz::stream::{CompressWriter, DecompressReader};
use std::io::{Write, Read};

let mut writer = CompressWriter::new(output_file);
writer.write_all(&data)?;
let output = writer.finish()?;
```

## Algoritmalar

- **BWT** — Burrows-Wheeler Transform. O(n²) suffix sort (büyük dosyalarda SA-IS'e geçilebilir). Rotasyonel sıralama klasik BWT ile birebir uyumlu.
- **MTF** — Move-to-Front, BWT çıktısını düşük-indeksli (çoğunlukla 0/1) dizilere yığar.
- **RLE** — Run-Length Encoding, `0xFE` marker ile kaçış; `MIN_RUN_LENGTH = 3`. MTF sonrası sıfır dizilerini sıkıştırır (bzip2-style).
- **Delta** — Ardışık byte farkı (u8 wrapping). Sayısal seriler için.
- **BCJ** — x86 E8/E9 (CALL/JMP) instruction'larının relatif adreslerini mutlak adrese çevirir; binary'lerde entropy coder'a daha öngörülebilir bir akış sağlar.
- **DeIlv** — Sabit-kayıt binary dosyaları sütun-bazlı yeniden düzenler; her sütun ayrı Huffman ile sıkıştırılır. Periyodik veri (DICOM, sao, sensör logları) için.
- **Huffman** — Length-limited canonical Huffman, **Package-Merge + Kraft rebalancing**. Maks. kod uzunluğu 16 bit.
- **ANS** — Canonical rANS (Fabian Giesen referansı), 4096 slotlu frekans tablosu. BWT+MTF çıktısında Huffman'dan genellikle daha küçük çıktı üretir.

## Test

```powershell
cargo test
```

111 test: transform roundtrip'leri, entropy coder roundtrip'leri, Canterbury corpus dosyalarında end-to-end testler, paralel/stream API testleri, property tabanlı kontroller (Kraft eşitsizliği, BWT klasik referans uyumu, DeIlv sütun roundtrip).

## Durum

**Tamamlanan:**
- BWT, MTF, RLE, Delta, BCJ, DeIlv transform'ları
- Huffman (length-limited, Package-Merge), rANS entropy coder
- İçerik-adaptif analyzer + 10 pipeline (0x00–0x09)
- ANS vs Huffman adaptif seçimi (BwtMtf ve BcjBwtMtf dallarında)
- BwtMtfRle pipeline (MTF sonrası RLE, bzip2-style)
- Paralel sıkıştırma (rayon, blok-bazlı)
- Stream API (CompressWriter, DecompressReader)
- Canterbury corpus: **%78.0** toplam tasarruf (gzip ~%62, bzip2 ~%68)
- Silesia corpus: **%72.2** toplam tasarruf; nci, xml, x-ray, webster, reymont, samba, osdb'de bzip2'yi geçer
- 111/111 test yeşil

**Yol haritasında:**
- crates.io yayınlama
- SA-IS ile O(n) BWT (büyük dosyalarda hız iyileştirmesi)
- Adaptif blok boyutu (içerik tipine göre)

## Lisans

MIT **veya** Apache-2.0 arasında seçim sizindir.

- [MIT](LICENSE-MIT)
- [Apache License 2.0](LICENSE-APACHE)

## Referanslar

- Burrows & Wheeler (1994), *A Block-sorting Lossless Data Compression Algorithm*
- Nong, Zhang & Chan (2009), *Linear Suffix Array Construction by Almost Pure Induced-Sorting*
- Fabian Giesen, *ryg_rans* (rANS reference implementation)
- Seward (2000), *bzip2 and libbzip2 manual*