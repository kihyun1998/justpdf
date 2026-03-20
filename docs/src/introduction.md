# Introduction

**justpdf** is a pure Rust PDF engine that provides comprehensive PDF processing capabilities:

- **Read** — Parse any PDF document, access pages, metadata, annotations, forms
- **Render** — Convert pages to PNG, JPEG, or SVG at any DPI
- **Extract** — Full text extraction with position data, search, structured output
- **Create** — Build PDFs from scratch with text, images, fonts
- **Modify** — Edit existing PDFs, merge, split, encrypt, sign
- **Extended Formats** — XPS, EPUB, SVG, Office (DOCX/XLSX/PPTX), CBZ support

## Architecture

```
justpdf-core      Core PDF engine (parsing, writing, text, crypto)
justpdf-render    Rendering (PNG/JPEG/SVG via tiny-skia)
justpdf           High-level API (Document, Page, Modifier)
justpdf-cli       Command-line tool
justpdf-formats   Extended format support
justpdf-special   OCR, barcode, ZUGFeRD, BiDi, deskew
justpdf-ffi       C API bindings
justpdf-python    Python bindings (PyO3)
justpdf-wasm      WebAssembly bindings
justpdf-node      Node.js bindings (napi-rs)
```

## Quick Start

```rust
use justpdf::Document;

// Open a PDF
let doc = Document::open("input.pdf")?;
println!("Pages: {}", doc.page_count()?);

// Extract text
let text = doc.text()?;
println!("{text}");

// Render page to PNG
let page = doc.page(0)?;
let png = page.render_png(150.0)?;
std::fs::write("page1.png", &png)?;
```
