# justpdf

High-level PDF library for Rust. The recommended entry point for the [justpdf](https://github.com/kihyun1998/justpdf) project.

Provides a simple, ergonomic API for reading, rendering, and manipulating PDF documents.

## Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
justpdf = "0.1"
```

### Example

```rust
use justpdf::Document;

let doc = Document::open("input.pdf")?;
println!("Pages: {}", doc.page_count());
let text = doc.page(0)?.text()?;
let png = doc.page(0)?.render_png(150.0)?;
```

## Features

- Open and inspect PDF documents
- Extract text per page
- Render pages to PNG
- Simple, high-level API built on `justpdf-core` and `justpdf-render`

## Repository

[https://github.com/kihyun1998/justpdf](https://github.com/kihyun1998/justpdf)

## License

MIT OR Apache-2.0
