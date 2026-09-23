# Extended Formats

The `justpdf-formats` crate adds support for document formats beyond PDF. Each format is behind a feature flag; `all` enables every format.

## Supported Formats

| Format | Feature | Text | Preview image | → PDF | Notes |
|--------|---------|------|---------------|-------|-------|
| XPS / OpenXPS | `xps` | Yes | Blank page | Yes (text) | |
| EPUB | `epub` | Yes | Yes | Yes (text) | DRM-protected files are rejected |
| SVG | `svg` | — | Yes | Yes (as image) | Simplified rasterizer |
| CBZ | `cbz` | — | Yes | Yes (images) | |
| DOCX / XLSX / PPTX | `office` | Yes | Yes | Yes (text) | |
| MOBI | `mobi` | Yes | Yes | Yes (text) | PalmDOC or uncompressed |
| FB2 | `fb2` | Yes | Yes | Yes (text) | |
| Plain text | `plaintext` | Yes | Yes | Yes | |

Text-based conversions produce plain text pages in Courier; layout and images from the source are not carried over. `plaintext`, `mobi` and `fb2` pull in `justpdf-render` for previews.

## Usage

Every format implements the `FormatDocument` trait.

```rust
use justpdf_formats::FormatDocument;
use justpdf_formats::office::OfficeDocument;
use std::path::Path;

let doc = OfficeDocument::open(Path::new("report.docx"))?;
println!("{}", doc.text()?);
let png = doc.render_page_png(0, 150.0)?;
let pdf = doc.to_pdf()?;
std::fs::write("report.pdf", pdf)?;
```

`justpdf_formats::detect::detect_format(path)` picks a format from the file extension.
