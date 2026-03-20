# Extended Formats

The `justpdf-formats` crate adds support for additional document formats beyond PDF.

## Supported Formats

| Format | Read | Render | Notes |
|--------|------|--------|-------|
| XPS    | Yes  | Yes    | XML Paper Specification |
| EPUB   | Yes  | Yes    | E-book format |
| SVG    | Yes  | Yes    | Scalable Vector Graphics |
| CBZ    | Yes  | Yes    | Comic book archive |
| DOCX   | Yes  | Yes    | Word documents |
| XLSX   | Yes  | Yes    | Excel spreadsheets |
| PPTX   | Yes  | Yes    | PowerPoint presentations |

## Usage

```rust
use justpdf_formats::open_document;

let doc = open_document("report.docx")?;
let text = doc.text()?;
let png = doc.render_page(0, 150.0)?;
```
