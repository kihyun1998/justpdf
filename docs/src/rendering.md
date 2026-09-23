# Rendering

## Render to PNG

```rust
use justpdf::Document;

let doc = Document::open("input.pdf")?;
let page = doc.page(0)?;
let png_data = page.render_png(150.0)?; // 150 DPI
std::fs::write("output.png", &png_data)?;
```

## Other Outputs

```rust
let jpeg = page.render_jpeg(150.0, 85)?;   // DPI, quality
let svg: String = page.render_svg()?;
let raw = page.render_raw(72.0)?;          // RGBA pixels: raw.data, raw.width, raw.height
page.render_to_file("page.png", 150.0)?;
```

## Render Options

```rust
use justpdf::{OutputFormat, RenderOptions};

let opts = RenderOptions {
    dpi: 300.0,
    format: OutputFormat::Jpeg { quality: 90 },
    ..Default::default()
};
let data = page.render(&opts)?;
```

`RenderOptions` also has `background: [u8; 4]` (RGBA, default opaque white). `OutputFormat` is `Png`, `Jpeg { quality }` or `RawRgba`.

## Parallel Rendering

With the `parallel` feature, `Document::render_all_png(dpi)` and `Document::render_all_parallel(&opts)` render every page on a thread pool.

Pages with `/Rotate 90` or `270` currently render at the unrotated size (#54).

## Supported Formats

- **PNG** — Lossless, best for text-heavy documents
- **JPEG** — Lossy, smaller file size for photos
- **Raw RGBA** — Pixel buffer for further processing
- **SVG** — Vector output
