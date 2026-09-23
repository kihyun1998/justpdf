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
for page in doc.pages() {
    println!("--- Page {} ---", page.index() + 1);
    println!("{}", page.text()?);
}
```

## Structured Text

`Page::text_structured` returns a `justpdf_core::text::PageText` with characters, lines and blocks, each carrying position information — useful for table extraction and layout analysis.

```rust
let structured = doc.page(0)?.text_structured()?;
println!(
    "{} chars, {} lines, {} blocks",
    structured.chars.len(),
    structured.lines.len(),
    structured.blocks.len()
);
```

## Search

```rust
for (page_index, hits) in doc.search("keyword")? {
    for hit in &hits {
        println!("page {}: {:?}", page_index + 1, hit.matched_text);
    }
}

// Case-insensitive search on one page
let hits = doc.page(0)?.search_case_insensitive("Keyword")?;
```

Each hit also carries a `quad` with the match's bounding box.
