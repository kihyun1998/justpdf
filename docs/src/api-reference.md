# API Reference

Full API documentation is generated from source with `cargo doc`.

## Generate Docs

```bash
cargo doc --workspace --no-deps --open
```

## Key Types

- **`justpdf::Document`** — High-level document handle
- **`justpdf::Page`** — Page access and rendering
- **`justpdf::Modifier`** — Document modification
- **`justpdf_core::PdfDocument`** — Low-level PDF document
- **`justpdf_core::PdfObject`** — PDF object model
- **`justpdf_render::RenderOptions`** — Rendering configuration
