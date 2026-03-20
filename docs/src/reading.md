# Reading PDFs

## Opening a Document

```rust
use justpdf::Document;

// From a file path
let doc = Document::open("input.pdf")?;

// From bytes
let bytes = std::fs::read("input.pdf")?;
let doc = Document::from_bytes(bytes)?;
```

## Password-Protected PDFs

```rust
let mut doc = Document::open("encrypted.pdf")?;
if doc.is_encrypted() {
    doc.authenticate(b"password")?;
}
```

## Accessing Metadata

```rust
let doc = Document::open("input.pdf")?;
println!("Version: {}.{}", doc.version().0, doc.version().1);
println!("Pages: {}", doc.page_count()?);
println!("Title: {:?}", doc.title());
println!("Author: {:?}", doc.author());
```

## Page Information

```rust
let page = doc.page(0)?;
let media_box = page.media_box();
println!("Size: {} x {} points", media_box.width(), media_box.height());
```
