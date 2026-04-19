# HybridZ

**HybridZ** is a content-adaptive hybrid compression library written in Rust. It inspects the input data, selects the most suitable transform + entropy coding combination, and applies it.

> ⚠️ **Alpha — API and binary format may change.** Not for production use.

📄 [Türkçe README](README.md)

## Why?

Traditional compressors (gzip, bzip2, zstd) commit to a single strategy. HybridZ first **analyzes** the data type (text, numeric, repetitive, executable, binary), then selects a matching **transform pipeline** + **entropy coder**. The goal is to beat different reference compressors on different data types.

## Benchmark — Canterbury Corpus

| File           |    Size | HybridZ   | gzip*  | bzip2* |
| -------------- | ------: | --------: | -----: | -----: |
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
| **TOTAL**      |  2.7 MB | **73.1%** |   ~62% |   ~68% |

\* gzip/bzip2 reference values are taken from published Canterbury Corpus results; percentage = space savings (higher is better).

**Highlight:** On `kennedy.xls`, HybridZ achieves 74.4% savings — **27 points** ahead of bzip2.

## Speed (release build, single-core)

| File         |      Compress |    Decompress |
| ------------ | ------------: | ------------: |
| alice29.txt  |     4.60 MB/s |    23.51 MB/s |
| kennedy.xls  |     5.47 MB/s |    24.63 MB/s |
| ptt5         |    89.55 MB/s |   221.77 MB/s |
| notepad.exe  |     5.96 MB/s |    18.38 MB/s |

## How It Works

```
                 ┌──────────────┐
     raw data ──▶│  analyzer    │──┐
                 └──────────────┘  │
                                   ▼
                 ┌──────────────────────────┐
                 │ transform pipeline       │
                 │ BWT → MTF (+ optional    │
                 │ BCJ), Delta, RLE,        │
                 │ DeltaRle, or none        │
                 └──────────────────────────┘
                                   │
                                   ▼
                 ┌──────────────────────────┐
                 │ entropy coder            │
                 │ Huffman ⚔ ANS            │
                 │ (smaller output wins)    │
                 └──────────────────────────┘
                                   │
                                   ▼
                          compressed output
```

`analyzer.rs` computes Shannon entropy, bigram repeat ratio, text byte ratio, and delta/rle/bcj suitability scores. It classifies the input into five categories — Text, Numeric, Repetitive, Binary, Unknown — each wired to a pre-selected pipeline. On the BWT+MTF and BCJ+BWT+MTF branches, both Huffman and ANS are run, and the smaller output wins.

## Pipeline IDs

Each compressed block carries a pipeline ID in the first header byte. The decoder uses this to pick the correct inverse transform.

| ID   | Transform          | Entropy |
| :--- | :----------------- | :------ |
| 0x00 | (none)             | Huffman |
| 0x01 | Delta              | Huffman |
| 0x02 | RLE                | Huffman |
| 0x03 | Delta + RLE        | Huffman |
| 0x04 | BWT + MTF          | Huffman |
| 0x05 | BWT + MTF          | ANS     |
| 0x06 | BCJ + BWT + MTF    | Huffman |
| 0x07 | BCJ + BWT + MTF    | ANS     |

## Usage (CLI)

```powershell
# Build
cargo build --release

# Compress / decompress a single file
.\target\release\hybridz.exe compress   input.txt compressed.hz
.\target\release\hybridz.exe decompress compressed.hz restored.txt

# Benchmark (compress, decompress, verify roundtrip, report)
.\target\release\hybridz.exe bench      input.txt

# Compress all files in a directory, print corpus table
.\target\release\hybridz.exe corpus     .\corpus\
```

## As a Library

```rust
use hybridz::{compress, decompress};

let original = b"the cat sat on the mat the cat sat on the mat";
let encoded  = compress(original)?;
let decoded  = decompress(&encoded)?;
assert_eq!(original.to_vec(), decoded);
```

## Algorithms

- **BWT** — Burrows-Wheeler Transform. Suffix array built via **SA-IS** (Nong/Zhang/Chan 2009) over `data + data`. Rotational ordering matches the classic BWT byte-for-byte.
- **MTF** — Move-to-Front, concentrates BWT output into low-index (mostly 0/1) sequences.
- **RLE** — Run-Length Encoding with `0xFE` escape marker; `MIN_RUN_LENGTH = 3`.
- **Delta** — Successive byte difference (u8 wrapping). For numeric series.
- **BCJ** — Rewrites x86 E8/E9 (CALL/JMP) relative addresses as absolute addresses, giving the entropy coder a more predictable stream on executables.
- **Huffman** — Length-limited canonical Huffman via **Package-Merge + Kraft rebalancing**. Max code length 16 bits.
- **ANS** — Canonical rANS (after Fabian Giesen's reference), 4096-slot frequency table.

## Tests

```powershell
cargo test
```

Currently **83 tests passing**: transform roundtrips, entropy coder roundtrips, end-to-end tests on Canterbury corpus files, property checks (Kraft inequality, BWT equivalence with classic reference).

## Status

**Done:**
- BWT (SA-IS), MTF, RLE, Delta, BCJ transforms
- Huffman (length-limited, Package-Merge), rANS entropy coder
- Content-adaptive analyzer + 8 pipelines
- Canterbury corpus benchmark: 73.1% total savings
- 83/83 tests green

**In progress:**
- Improved executable compression (indirect call handling)
- Full Silesia corpus benchmarks
- Comparison against zstd / lz4 / brotli
- Block-based pipeline selection for larger files

**Roadmap:**
- Parallel compression (rayon)
- Stream API (Read/Write traits)
- Publish as a crate

## License

Licensed under either of

- [MIT license](LICENSE-MIT)
- [Apache License 2.0](LICENSE-APACHE)

at your option.

## References

- Burrows & Wheeler (1994), *A Block-sorting Lossless Data Compression Algorithm*
- Nong, Zhang & Chan (2009), *Linear Suffix Array Construction by Almost Pure Induced-Sorting*
- Fabian Giesen, *ryg_rans* (rANS reference implementation)
- Seward (2000), *bzip2 and libbzip2 manual*
