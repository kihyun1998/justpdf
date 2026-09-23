# Introduction

**justpdf** is a pure Rust PDF engine that provides comprehensive PDF processing capabilities:

- **Read** — Parse PDF documents, access pages, metadata, outlines, annotations, forms, attachments
- **Render** — Convert pages to PNG, JPEG, raw RGBA, or SVG at any DPI
- **Extract** — Text extraction with position data, reading-order analysis, search
- **Create** — Build PDFs from scratch with text, images, fonts
- **Modify** — Edit existing PDFs: delete, reorder, merge, split pages, edit metadata
- **Compress** — Shrink PDFs with image re-encoding, font subsetting and stream optimization
- **Extended Formats** — XPS, EPUB, SVG, Office (DOCX/XLSX/PPTX), CBZ, MOBI, FB2, plaintext

## Architecture

```
justpdf-core           Core PDF engine (parsing, writing, text, compression, crypto, signatures)
justpdf-render         Rendering (PNG/JPEG/RGBA via tiny-skia, SVG export)
justpdf                High-level API (Document, Page, Modifier) over core + render
justpdf-cli            Command-line tool (`justpdf` binary)
justpdf-formats        Extended format support
justpdf-special        OCR, barcode, ZUGFeRD, BiDi, deskew
justpdf-compress-wasm  Browser PDF compression (WebAssembly)
justpdf-ffi            C API bindings
justpdf-python         Python bindings (PyO3)
justpdf-wasm           WebAssembly bindings
justpdf-node           Node.js bindings (napi-rs)
```

## Quick Start

```rust
use justpdf::Document;

// Open a PDF
let doc = Document::open("input.pdf")?;
println!("Pages: {}", doc.page_count());

// Extract text
let text = doc.text()?;
println!("{text}");

// Render page to PNG
let png = doc.page(0)?.render_png(150.0)?;
std::fs::write("page1.png", &png)?;
```
