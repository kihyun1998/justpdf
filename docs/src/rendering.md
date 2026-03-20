# Rendering

## Render to PNG

```rust
use justpdf::Document;

let doc = Document::open("input.pdf")?;
let page = doc.page(0)?;
let png_data = page.render_png(150.0)?; // 150 DPI
std::fs::write("output.png", &png_data)?;
```

## Render Options

```rust
use justpdf_render::{RenderOptions, OutputFormat};

let opts = RenderOptions {
    dpi: 300.0,
    format: OutputFormat::Jpeg,
    ..Default::default()
};
let data = justpdf_render::render_page(&doc, 0, &opts)?;
```

## Supported Formats

- **PNG** — Lossless, best for text-heavy documents
- **JPEG** — Lossy, smaller file size for photos
- **SVG** — Vector output, scalable
