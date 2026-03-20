# justpdf-core

Core PDF engine for the [justpdf](https://github.com/kihyun1998/justpdf) project.

Provides PDF parsing, writing, text extraction, encryption/decryption, annotations, and form handling.

## Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
justpdf-core = "0.1"
```

### Example

```rust
use justpdf_core::PdfDocument;

let doc = PdfDocument::open(std::path::Path::new("input.pdf"))?;
let pages = justpdf_core::page::collect_pages(&doc)?;
println!("Pages: {}", pages.len());

let text = justpdf_core::text::extract_all_text_string(&doc)?;
println!("{text}");
```

## Features

- PDF parsing and writing
- Text extraction
- Encryption and decryption
- Annotation support
- Form field handling

## Repository

[https://github.com/kihyun1998/justpdf](https://github.com/kihyun1998/justpdf)

## License

MIT OR Apache-2.0
