# HybridZ

**HybridZ**, Rust ile yazılmış içerik-adaptif hibrit bir sıkıştırma kütüphanesidir. Veriye bakar, en uygun transform + entropy coding kombinasyonunu seçer ve uygular.

> ⚠️ **Alpha — API ve binary format değişebilir.** Üretimde kullanmayın.

📄 [English README](README.en.md)

## Neden?

Klasik sıkıştırıcılar (gzip, bzip2, zstd) tek bir stratejiye sabitlenir. HybridZ, veri tipini (metin, sayısal, tekrarlı, executable, binary) önce **analiz eder**, sonra ona uygun bir **transform pipeline** + **entropy coder** seçer. Farklı veri tiplerinde farklı referans sıkıştırıcıları geçmeyi hedefler.

## Benchmark — Canterbury Corpus

| Dosya          | Boyut   | HybridZ | gzip*  | bzip2* |
| -------------- | ------: | ------: | -----: | -----: |
| alice29.txt    |  148 KB | **67.5%** |  64.4% |  71.6% |
| asyoulik.txt   |  122 KB | **63.7%** |  59.4% |  68.4% |
| cp.html        |   24 KB | **64.2%** |  68.9% |  75.9% |
| fields.c       |   11 KB | **68.4%** |  72.1% |  75.9% |
| kennedy.xls    | 1006 KB | **74.4%** |  41.0% |  47.0% |
| lcet10.txt     |  417 KB | **70.2%** |  63.4% |  72.3% |
| plrabn12.txt   |  471 KB | **64.6%** |  59.0% |  69.0% |
| ptt5           |  501 KB | **86.3%** |  91.0% |  93.0% |
| sum            |   37 KB | **62.2%** |  64.1% |  67.9% |
| xargs.1        |    4 KB | **53.9%** |  63.5% |  69.8% |
| **TOPLAM**     | 2.7 MB  | **73.1%** |  ~62%  |  ~68%  |

\* gzip/bzip2 referans değerleri Canterbury Corpus sonuçlarından alınmıştır; yüzde = tasarruf oranı (yüksek iyidir).

**Öne çıkan:** `kennedy.xls` üzerinde %74.4 tasarrufla bzip2'yi **27 puan** geride bırakır.

## Hız (release build, single-core)

| Dosya        | Compress         | Decompress        |
| ------------ | ---------------: | ----------------: |
| alice29.txt  | 4.60 MB/s        | 23.51 MB/s        |
| kennedy.xls  | 5.47 MB/s        | 24.63 MB/s        |
| ptt5         | 89.55 MB/s       | 221.77 MB/s       |
| notepad.exe  | 5.96 MB/s        | 18.38 MB/s        |

## Nasıl Çalışır?

```
                 ┌──────────────┐
     ham veri ──▶│  analyzer    │──┐
                 └──────────────┘  │
                                   ▼
                 ┌──────────────────────────┐
                 │ transform pipeline       │
                 │ BWT → MTF (+ BCJ opsiy.) │
                 │ Delta, RLE, DeltaRle     │
                 │ (veya transform yok)     │
                 └──────────────────────────┘
                                   │
                                   ▼
                 ┌──────────────────────────┐
                 │ entropy coder            │
                 │ Huffman ⚔ ANS            │
                 │ (küçük olan kazanır)     │
                 └──────────────────────────┘
                                   │
                                   ▼
                          sıkıştırılmış veri
```

`analyzer.rs`, Shannon entropisi + bigram tekrar oranı + text byte oranı + delta/rle/bcj uygunluk skorlarına bakarak veriyi beş kategoriye ayırır: Text, Numeric, Repetitive, Binary, Unknown. Her kategori için önceden belirlenmiş bir pipeline çalıştırılır. BWT+MTF ve BCJ+BWT+MTF dallarında hem Huffman hem ANS denenir, çıktısı küçük olan seçilir.

## Pipeline ID'leri

Her sıkıştırılmış blok, header'ın ilk byte'ında bir pipeline ID taşır. Decoder buna bakıp doğru ters dönüşümü uygular.

| ID   | Transform          | Entropy  |
| :--- | :----------------- | :------- |
| 0x00 | (yok)              | Huffman  |
| 0x01 | Delta              | Huffman  |
| 0x02 | RLE                | Huffman  |
| 0x03 | Delta + RLE        | Huffman  |
| 0x04 | BWT + MTF          | Huffman  |
| 0x05 | BWT + MTF          | ANS      |
| 0x06 | BCJ + BWT + MTF    | Huffman  |
| 0x07 | BCJ + BWT + MTF    | ANS      |

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

- **BWT** — Burrows-Wheeler Transform. Suffix array, **SA-IS** (Nong/Zhang/Chan 2009) ile `data + data` üzerinde kuruluyor. Rotasyonel sıralama klasik BWT ile birebir uyumlu.
- **MTF** — Move-to-Front, BWT çıktısını düşük-indeksli (çoğunlukla 0/1) dizilere yığar.
- **RLE** — Run-Length Encoding, `0xFE` marker ile kaçış; `MIN_RUN_LENGTH = 3`.
- **Delta** — Ardışık byte farkı (u8 wrapping). Sayısal seriler için.
- **BCJ** — x86 E8/E9 (CALL/JMP) instruction'larının relatif adreslerini mutlak adrese çevirir; binary'lerde entropy coder'a daha öngörülebilir bir akış sağlar.
- **Huffman** — Length-limited canonical Huffman, **Package-Merge + Kraft rebalancing**. Maks. kod uzunluğu 16 bit.
- **ANS** — Canonical rANS (Fabian Giesen referansı), 4096 slotlu frekans tablosu.

## Test

```powershell
cargo test
```

Şu anda **83 test** geçiyor: transform roundtrip'leri, entropy coder roundtrip'leri, Canterbury corpus dosyalarında end-to-end testler, property tabanlı kontroller (Kraft eşitsizliği, BWT'nin klasik referansla eşitliği).

## Durum

**Tamamlanan:**
- BWT (SA-IS), MTF, RLE, Delta, BCJ transform'ları
- Huffman (length-limited, Package-Merge), rANS entropy coder
- İçerik-adaptif analyzer + 8 pipeline
- Canterbury corpus benchmark: %73.1 toplam tasarruf
- 83/83 test yeşil

**Üstünde çalışılan:**
- Executable compression iyileştirmesi (indirect call handling)
- Full Silesia corpus benchmark'ı
- zstd / lz4 / brotli karşılaştırması
- Büyük dosyalar için block-based pipeline seçimi

**Yol haritasında:**
- Paralel sıkıştırma (rayon)
- Stream API (Read/Write trait'leri)
- Crate olarak yayınlama

## Lisans

MIT **veya** Apache-2.0 arasında seçim sizindir.

- [MIT](LICENSE-MIT)
- [Apache License 2.0](LICENSE-APACHE)

## Referanslar

- Burrows & Wheeler (1994), *A Block-sorting Lossless Data Compression Algorithm*
- Nong, Zhang & Chan (2009), *Linear Suffix Array Construction by Almost Pure Induced-Sorting*
- Fabian Giesen, *ryg_rans* (rANS reference implementation)
- Seward (2000), *bzip2 and libbzip2 manual*
