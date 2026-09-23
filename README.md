# justpdf

[![CI](https://github.com/kihyun1998/justpdf/actions/workflows/ci.yml/badge.svg)](https://github.com/kihyun1998/justpdf/actions/workflows/ci.yml)

A pure Rust PDF engine for reading, rendering, text extraction, creation, modification, and security.

## Features

- **Read** - Parse PDF 1.0-2.0, cross-reference tables/streams, incremental updates, object streams
- **Render** - Rasterize pages to PNG/JPEG/raw RGBA with tiny-skia, or export SVG; parallel rendering (feature `parallel`)
- **Text extraction** - Unicode-aware text extraction with positions, reading-order analysis, search
- **Create** - Build PDFs from scratch with the page builder API
- **Modify** - Merge, split, reorder, delete pages, edit metadata
- **Compress** - Image re-encoding/downscaling, font subsetting, stream dedup and recompression (4 presets)
- **Security** - Decrypt/encrypt (RC4, AES-128/256), signature detection and verification (RSA)
- **Performance** - Memory-mapped I/O (`mmap`), arena allocation (`arena`), async file loading (`async`)
- **Formats** - XPS, EPUB, SVG, Office (DOCX/XLSX/PPTX), CBZ, MOBI, FB2, plaintext → PDF
- **Special** - OCR via Tesseract, barcode/QR generation, ZUGFeRD invoice reading, BiDi, deskew

## Quick start

Add to your `Cargo.toml`:

```toml
[dependencies]
justpdf = "0.1"
```

### Rust example

```rust
use justpdf::Document;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Read and extract text
    let doc = Document::open("input.pdf")?;
    for page in doc.pages() {
        println!("{}", page.text()?);
    }

    // Render a page to PNG
    let png = doc.page(0)?.render_png(150.0)?;
    std::fs::write("page1.png", png)?;

    Ok(())
}
```

## CLI

```bash
# Extract text from a PDF
justpdf text input.pdf

# Render page 1 to PNG, or every page into a directory
justpdf render input.pdf --dpi 150 -o page1.png
justpdf render input.pdf --all --dpi 150 -o ./pages/

# Get PDF metadata and page count
justpdf info input.pdf

# Merge multiple PDFs
justpdf merge a.pdf b.pdf -o combined.pdf

# Compress with a preset (low, medium, high, extreme), or preview first
justpdf compress input.pdf --preset high -o smaller.pdf
justpdf compress input.pdf --analyze

# Convert formats
justpdf convert document.epub -o output.pdf
```

Prebuilt binaries for Linux, macOS and Windows are attached to each [GitHub release](https://github.com/kihyun1998/justpdf/releases).

## Crate structure

| Crate | Description |
|-------|-------------|
| `justpdf-core` | Parser, object model, text extraction, writer, compression, crypto, signatures |
| `justpdf-render` | Page rasterization with tiny-skia, SVG export |
| `justpdf` | Unified high-level API (core + render) |
| `justpdf-cli` | Command-line interface (`justpdf` binary) |
| `justpdf-formats` | XPS, EPUB, SVG, Office, CBZ, MOBI, FB2, plaintext support |
| `justpdf-special` | OCR, barcode, ZUGFeRD, BiDi, deskew |
| `justpdf-compress-wasm` | Browser PDF compression (WebAssembly) |
| `justpdf-ffi` | C FFI bindings |
| `justpdf-python` | Python bindings (PyO3) |
| `justpdf-wasm` | WebAssembly bindings (wasm-bindgen) |
| `justpdf-node` | Node.js bindings (napi-rs) |

## Language bindings

### Python

```python
import justpdf

doc = justpdf.open("input.pdf")
print(doc.page_count)
text = doc.page_text(0)
print(text)
```

### JavaScript / WASM

```javascript
import init, { WasmDocument } from 'justpdf-wasm';

await init();
const doc = new WasmDocument(pdfBytes);
const text = doc.page_text(0);
console.log(text);
```

### C

```c
#include "justpdf.h"

JustPdfDocument *doc = NULL;
if (justpdf_open("input.pdf", &doc) == JUSTPDF_OK) {
    char *text = NULL;
    if (justpdf_extract_page_text(doc, 0, &text) == JUSTPDF_OK) {
        printf("%s\n", text);
        justpdf_free_string(text);
    }
    justpdf_close(doc);
}
```

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT License ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.
