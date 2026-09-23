# Getting Started

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
justpdf = "0.1"
```

Optional features: `mmap` (memory-mapped files), `parallel` (multi-threaded rendering), `arena` (arena-allocated content parsing in `justpdf-core`), `async` (async file loading).

## CLI Installation

```bash
cargo install justpdf-cli
```

Prebuilt binaries for Linux, macOS and Windows are attached to each GitHub release.

## Basic Usage

```rust
use justpdf::Document;

let doc = Document::open("document.pdf")?;
for page in doc.pages() {
    println!("{}", page.text()?);
}
```
