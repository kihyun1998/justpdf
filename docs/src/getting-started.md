# Getting Started

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
justpdf = "0.1"
```

## CLI Installation

```bash
cargo install justpdf-cli
```

## Basic Usage

```rust
use justpdf::Document;

let doc = Document::open("document.pdf")?;
for page in doc.pages()? {
    println!("{}", page.text()?);
}
```
