# justpdf-render

Rendering engine for the [justpdf](https://github.com/kihyun1998/justpdf) project.

Renders PDF pages to PNG, JPEG, raw RGBA, and SVG output formats.

## Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
justpdf-render = "0.1"
justpdf-core = "0.1"
```

### Example

```rust
use justpdf_core::PdfDocument;
use justpdf_render::{render_page, RenderOptions};

let doc = PdfDocument::open(std::path::Path::new("input.pdf"))?;
let opts = RenderOptions { dpi: 150.0, ..Default::default() };
let png = render_page(&doc, 0, &opts)?;
std::fs::write("page1.png", &png)?;

let svg = justpdf_render::render_page_to_svg(&doc, 0)?;
```

## Features

- Configurable DPI and background color
- PNG, JPEG, raw RGBA, and SVG output
- Page-level rendering; multi-threaded rendering of many pages with the `parallel` feature

## Repository

[https://github.com/kihyun1998/justpdf](https://github.com/kihyun1998/justpdf)

## License

MIT OR Apache-2.0
