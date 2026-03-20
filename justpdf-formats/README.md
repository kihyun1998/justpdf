# justpdf-formats

Extended format support for the [justpdf](https://github.com/kihyun1998/justpdf) project.

Read and convert XPS, EPUB, SVG, DOCX/XLSX/PPTX, CBZ, MOBI, and FB2 documents.

## Usage

All formats are feature-gated. Enable what you need:

```toml
[dependencies]
justpdf-formats = { version = "0.1", features = ["all"] }
```

### Example

```rust
use justpdf_formats::FormatDocument;
use justpdf_formats::svg::SvgDocument;

let doc = SvgDocument::from_bytes(svg_data)?;
let pdf = doc.to_pdf()?;
```

## Supported Formats

| Feature | Format               |
|---------|----------------------|
| `xps`   | XPS documents        |
| `epub`  | EPUB eBooks          |
| `svg`   | SVG images           |
| `ooxml` | DOCX, XLSX, PPTX     |
| `cbz`   | Comic book archives  |
| `mobi`  | MOBI eBooks          |
| `fb2`   | FB2 eBooks           |

## Repository

[https://github.com/kihyun1998/justpdf](https://github.com/kihyun1998/justpdf)

## License

MIT OR Apache-2.0
