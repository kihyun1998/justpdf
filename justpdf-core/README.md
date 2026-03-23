# justpdf-core

Core PDF engine for the [justpdf](https://github.com/kihyun1998/justpdf) project.

Pure Rust PDF library — parsing, writing, text extraction, compression, encryption, digital signatures, and more.

## Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
justpdf-core = "0.1"
```

### Example — Read & Extract

```rust
use justpdf_core::PdfDocument;

let doc = PdfDocument::open(std::path::Path::new("input.pdf"))?;
let pages = justpdf_core::page::collect_pages(&doc)?;
println!("Pages: {}", pages.len());

let text = justpdf_core::text::extract_all_text_string(&doc)?;
println!("{text}");
```

### Example — Compress

```rust
use justpdf_core::writer::compress::{compress_pdf, CompressOptions};

let pdf_data = std::fs::read("input.pdf")?;
let options = CompressOptions::from_preset("high").unwrap();
let (compressed, stats) = compress_pdf(&pdf_data, &options)?;

println!("{}MB → {}MB ({:.1}% reduction)",
    stats.original_size as f64 / 1_000_000.0,
    stats.compressed_size as f64 / 1_000_000.0,
    (1.0 - stats.ratio()) * 100.0,
);
std::fs::write("output.pdf", &compressed)?;
```

## Features

- **PDF parsing & writing** — incremental updates, object streams, cross-reference streams
- **Text extraction** — Unicode, CJK, ToUnicode CMap support
- **Compression engine** — JPEG re-encoding, image downscaling, font subsetting, stream dedup, Flate re-compression, metadata stripping, object stream packing, grayscale conversion, DPI-aware scaling
- **4 compression presets** — `low`, `medium`, `high`, `extreme` + fully custom options
- **Encryption & decryption** — RC4, AES-128, AES-256 (R2–R6)
- **Digital signatures** — sign, detect, verify (PKCS#7/CMS, RSA, SHA-256/384/512)
- **Visible signature appearance** — Form XObject generation
- **RFC 3161 timestamps** — timestamp request/response helpers
- **Annotations & forms** — AcroForm fields, widget annotations
- **Optional Content Groups (OCG)** — layers, visibility control
- **Font subsetting** — TrueType/CIDFontType2 glyph-level subsetting
- **Linearization** — fast web view optimization

### Optional Features

| Feature | Description |
|---------|-------------|
| `mmap` | Memory-mapped file I/O via `memmap2` |
| `arena` | Arena allocator via `bumpalo` for reduced allocation overhead |

## Repository

[https://github.com/kihyun1998/justpdf](https://github.com/kihyun1998/justpdf)

## License

MIT OR Apache-2.0
