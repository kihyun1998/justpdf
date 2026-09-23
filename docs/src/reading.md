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

Opening tries the empty password automatically. If that fails, authenticate explicitly:

```rust
let mut doc = Document::open("encrypted.pdf")?;
if doc.is_encrypted() && !doc.is_authenticated() {
    doc.authenticate(b"password")?;
}
```

## Accessing Metadata

```rust
let doc = Document::open("input.pdf")?;
println!("Version: {}", doc.version_string());
println!("Pages: {}", doc.page_count());
println!("Title: {:?}", doc.title());
println!("Author: {:?}", doc.author());
for (key, value) in doc.metadata() {
    println!("{key}: {value}");
}
```

## Page Information

```rust
let page = doc.page(0)?;
let media_box = page.media_box();
println!("Size: {} x {} points", media_box.width(), media_box.height());
println!("Rotation: {}", page.rotation());
```

## Document Structure

```rust
let outlines = doc.outlines()?;          // bookmarks
let annots = doc.annotations(0)?;        // annotations on page 1
let form = doc.form_fields()?;           // AcroForm, if any
let files = doc.embedded_files()?;       // attachments
let sigs = doc.signatures()?;            // signature fields
```
