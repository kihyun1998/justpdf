# justpdf

[![CI](https://github.com/kihyun1998/justpdf/actions/workflows/ci.yml/badge.svg)](https://github.com/kihyun1998/justpdf/actions/workflows/ci.yml)

A pure Rust PDF engine for reading, rendering, text extraction, creation, modification, and security.

## Features

- **Read** - Parse PDF 1.0-2.0, cross-reference tables/streams, incremental updates
- **Render** - Rasterize pages to pixel buffers with tiny-skia, parallel rendering support
- **Text extraction** - Unicode-aware text extraction with positioning and layout analysis
- **Create** - Build PDFs from scratch with page builder API
- **Modify** - Merge, split, rotate, add/remove pages, incremental save
- **Security** - Decrypt/encrypt (RC4, AES-128/256), permission enforcement, digital signatures
- **Performance** - Memory-mapped I/O, arena allocation, async I/O, parallel rendering
- **Formats** - XPS, EPUB, SVG, Office (DOCX/XLSX/PPTX), CBZ, MOBI, FB2, plaintext
- **Special** - OCR (template-based), barcode/QR generation, ZUGFeRD invoices, BiDi text, deskew

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
        println!("{}", page.extract_text()?);
    }

    // Render a page to PNG
    let pixmap = doc.pages().next().unwrap().render(150.0)?;
    std::fs::write("page1.png", pixmap.encode_png()?)?;

    Ok(())
}
```

## CLI

```bash
# Extract text from a PDF
justpdf text input.pdf

# Render pages to PNG images
justpdf render input.pdf --dpi 150 --out ./pages/

# Get PDF metadata and page count
justpdf info input.pdf

# Merge multiple PDFs
justpdf merge a.pdf b.pdf -o combined.pdf

# Convert formats
justpdf convert document.epub -o output.pdf
```

## Crate structure

| Crate | Description |
|-------|-------------|
| `justpdf-core` | Parser, object model, text extraction, writer, crypto, signatures |
| `justpdf-render` | Page rasterization with tiny-skia |
| `justpdf` | Unified high-level API |
| `justpdf-cli` | Command-line interface |
| `justpdf-formats` | XPS, EPUB, SVG, Office, CBZ, MOBI, FB2 support |
| `justpdf-special` | OCR, barcode, ZUGFeRD, BiDi, deskew |
| `justpdf-ffi` | C FFI bindings |
| `justpdf-python` | Python bindings (PyO3) |
| `justpdf-wasm` | WebAssembly bindings (wasm-bindgen) |
| `justpdf-node` | Node.js bindings (napi-rs) |

## Language bindings

### Python

```python
import justpdf

doc = justpdf.open("input.pdf")
text = doc.page(0).extract_text()
print(text)
```

### JavaScript / WASM

```javascript
import init, { Document } from 'justpdf-wasm';

await init();
const doc = Document.open(pdfBytes);
const text = doc.page(0).extractText();
console.log(text);
```

### C

```c
#include "justpdf.h"

JustPdfDoc *doc = justpdf_open("input.pdf");
char *text = justpdf_page_extract_text(doc, 0);
printf("%s\n", text);
justpdf_free_string(text);
justpdf_close(doc);
```

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT License ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.
