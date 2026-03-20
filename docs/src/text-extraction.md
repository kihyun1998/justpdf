# Text Extraction

## Basic Extraction

```rust
use justpdf::Document;

let doc = Document::open("input.pdf")?;
let text = doc.text()?;
println!("{text}");
```

## Per-Page Extraction

```rust
for i in 0..doc.page_count()? {
    let page = doc.page(i)?;
    println!("--- Page {} ---", i + 1);
    println!("{}", page.text()?);
}
```

## Structured Text

The `justpdf_core::text` module provides access to text with position information, useful for table extraction and layout analysis.

## Search

```rust
let results = doc.search("keyword")?;
for hit in &results {
    println!("Page {}: found at ({}, {})", hit.page, hit.x, hit.y);
}
```
