# API Reference

Full API documentation is generated from source with `cargo doc`.

## Generate Docs

```bash
cargo doc --workspace --no-deps --open
```

## Key Types

- **`justpdf::Document`** — High-level document handle
- **`justpdf::Page`** — Page access, text and rendering
- **`justpdf::Modifier`** — Document modification (from `Document::modify`)
- **`justpdf::DocumentBuilder`** / **`justpdf::PageBuilder`** — Creating PDFs
- **`justpdf_core::PdfDocument`** — Low-level PDF document
- **`justpdf_core::PdfObject`** — PDF object model
- **`justpdf_core::writer::compress_pdf`** / **`CompressOptions`** — Compression
- **`justpdf_render::RenderOptions`** — Rendering configuration
