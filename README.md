# HybridZ

**HybridZ**, Rust ile yazılmış içerik-adaptif hibrit bir sıkıştırma kütüphanesidir. Veriye bakar, en uygun transform + entropy coding kombinasyonunu seçer ve uygular.

> ⚠️ **Alpha — API ve binary format değişebilir.** Üretimde kullanmayın.

🔄 [English README](README.en.md)

## Neden?

Klasik sıkıştırıcılar (gzip, bzip2, zstd) tek bir stratejiye sabitlenir. HybridZ, veri tipini (metin, sayısal, tekrarlı, executable, binary, sabit-kayıt) önce **analiz eder**, sonra ona uygun bir **transform pipeline** + **entropy coder** seçer. Farklı veri tiplerinde farklı referans sıkıştırıcıları geçmeyi hedefler.

## Benchmark — Canterbury Corpus

| Dosya          | Boyut   | HybridZ    | gzip*  | bzip2* |
| -------------- | ------: | ---------: | -----: | -----: |
| alice29.txt    |  148 KB | **67.5%**  |  64.4% |  71.6% |
| asyoulik.txt   |  122 KB | **63.7%**  |  59.4% |  68.4% |
| cp.html        |   24 KB | **64.2%**  |  68.9% |  75.9% |
| fields.c       |   11 KB | **68.4%**  |  72.1% |  75.9% |
| kennedy.xls    | 1006 KB | **74.4%**  |  41.0% |  47.0% |
| lcet10.txt     |  417 KB | **70.2%**  |  63.4% |  72.3% |
| plrabn12.txt   |  471 KB | **64.6%**  |  59.0% |  69.0% |
| ptt5           |  501 KB | **86.3%**  |  91.0% |  93.0% |
| sum            |   37 KB | **62.2%**  |  64.1% |  67.9% |
| xargs.1        |    4 KB | **53.9%**  |  63.5% |  69.8% |
| **TOPLAM**     |  2.7 MB | **73.1%**  |  ~62%  |  ~68%  |

\* gzip/bzip2 referans değerleri Canterbury Corpus sonuçlarından alınmıştır; yüzde = tasarruf oranı (yüksek iyidir).

**Öne çıkan:** `kennedy.xls` üzerinde %74.4 tasarrufla bzip2'yi **27 puan** geride bırakır.

## Benchmark — Silesia Corpus (202 MB)

| Dosya      | Boyut    | HybridZ    | bzip2* | Δ       |
| ---------- | -------: | ---------: | -----: | ------: |
| dickens    |  9.7 MB  | **69.8%**  |   73%  |   -3.2  |
| mozilla    | 48.8 MB  | **57.4%**  |   63%  |   -5.6  |
| mr         |  9.5 MB  | **65.0%**  |   75%  |  -10.0  |
| nci        | 32.0 MB  | **92.8%** 🎯|   85%  |  **+7.8** |
| ooffice    |  5.9 MB  | **55.5%**  |   59%  |   -3.5  |
| osdb       |  9.6 MB  | **63.4%**  |   64%  |   -0.6  |
| reymont    |  6.3 MB  | **76.0%** ✅|   75%  |  **+1.0** |
| samba      | 20.6 MB  | **73.2%** ✅|   73%  |  **+0.2** |
| sao        |  6.9 MB  | **29.3%**  |   35%  |   -5.7  |
| webster    | 39.5 MB  | **76.6%** ✅|   73%  |  **+3.6** |
| x-ray      |  8.1 MB  | **39.6%** ✅|   35%  |  **+4.6** |
| xml        |  5.1 MB  | **88.3%** 🎯|   75%  | **+13.3** |
| **TOPLAM** | 202 MB   | **69.2%**  |  ~72%  |   -2.8  |

\* bzip2 Silesia referans değerleri; yüzde = tasarruf oranı.

**Öne çıkanlar:** `nci` +7.8, `xml` +13.3, `webster` +3.6 puan bzip2'yi geçer.

## Hız (release build, single-core)

| Dosya        | Compress         | Decompress        |
| ------------ | ---------------: | ----------------: |
| alice29.txt  | ~4.6 MB/s        | ~23.5 MB/s        |
| kennedy.xls  | ~5.5 MB/s        | ~24.6 MB/s        |
| ptt5         | ~89.6 MB/s       | ~221.8 MB/s       |
| notepad.exe  | ~5.9 MB/s        | ~18.4 MB/s        |

## Nasıl Çalışır?

```
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
```

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

let original = b"the cat sat on the mat the cat sat on the mat";
let encoded  = compress(original)?;
let decoded  = decompress(&encoded)?;
assert_eq!(original.to_vec(), decoded);
```

## Algoritmalar

- **BWT** — Burrows-Wheeler Transform. O(n²) suffix sort (büyük dosyalarda SA-IS'e geçilebilir). Rotasyonel sıralama klasik BWT ile birebir uyumlu.
- **MTF** — Move-to-Front, BWT çıktısını düşük-indeksli (çoğunlukla 0/1) dizilere yığar.
- **RLE** — Run-Length Encoding, `0xFE` marker ile kaçış; `MIN_RUN_LENGTH = 3`.
- **Delta** — Ardışık byte farkı (u8 wrapping). Sayısal seriler için.
- **BCJ** — x86 E8/E9 (CALL/JMP) instruction'larının relatif adreslerini mutlak adrese çevirir; binary'lerde entropy coder'a daha öngörülebilir bir akış sağlar.
- **DeIlv** — Sabit-kayıt binary dosyaları sütun-bazlı yeniden düzenler; her sütun ayrı Huffman ile sıkıştırılır. Periyodik veri (DICOM, sao, sensör logları) için.
- **Huffman** — Length-limited canonical Huffman, **Package-Merge + Kraft rebalancing**. Maks. kod uzunluğu 16 bit.
- **ANS** — Canonical rANS (Fabian Giesen referansı), 4096 slotlu frekans tablosu. BWT+MTF çıktısında Huffman'dan genellikle daha küçük çıktı üretir.

## Test

```powershell
cargo test
```

98 test: transform roundtrip'leri, entropy coder roundtrip'leri, Canterbury corpus dosyalarında end-to-end testler, property tabanlı kontroller (Kraft eşitsizliği, BWT klasik referans uyumu, DeIlv sütun roundtrip).

## Durum

**Tamamlanan:**
- BWT, MTF, RLE, Delta, BCJ, DeIlv transform'ları
- Huffman (length-limited, Package-Merge), rANS entropy coder
- İçerik-adaptif analyzer + 9 pipeline (0x00–0x08)
- ANS vs Huffman adaptif seçimi (BwtMtf ve BcjBwtMtf dallarında)
- Canterbury corpus: **%73.1** toplam tasarruf (gzip ~%62, bzip2 ~%68)
- Silesia corpus: **%69.2** toplam tasarruf; nci, xml, webster, reymont, samba, x-ray'de bzip2'yi geçer
- 98/98 test yeşil

**Yol haritasında:**
- MTF sonrası RLE (bzip2-style, mr/dickens iyileştirmesi)
- Paralel sıkıştırma (rayon)
- Stream API (Read/Write trait'leri)
- Crate olarak yayınlama (crates.io)

## Lisans

MIT **veya** Apache-2.0 arasında seçim sizindir.

- [MIT](LICENSE-MIT)
- [Apache License 2.0](LICENSE-APACHE)

## Referanslar

- Burrows & Wheeler (1994), *A Block-sorting Lossless Data Compression Algorithm*
- Nong, Zhang & Chan (2009), *Linear Suffix Array Construction by Almost Pure Induced-Sorting*
- Fabian Giesen, *ryg_rans* (rANS reference implementation)
- Seward (2000), *bzip2 and libbzip2 manual*