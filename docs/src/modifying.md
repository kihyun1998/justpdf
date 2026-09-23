# Modifying PDFs

## Using the Modifier

`Document::modify` returns a `Modifier` working on a copy of the document; the original `Document` is not changed.

```rust
use justpdf::Document;

let doc = Document::open("input.pdf")?;
let mut m = doc.modify()?;
m.delete_page(2)?;          // 0-based
m.set_title("Edited");
m.garbage_collect();        // drop objects no longer referenced
m.save("output.pdf")?;
```

Other operations: `insert_page`, `reorder_pages`, `set_author`, `set_subject`, `set_keywords`, and `build()` to get the bytes instead of saving.

## Merging Documents

```rust
let merged = justpdf::merge(&["doc1.pdf", "doc2.pdf"])?;
std::fs::write("merged.pdf", merged)?;
```

`justpdf::merge_bytes` does the same for in-memory PDFs.

## Splitting

```rust
let doc = Document::open("input.pdf")?;
for i in 0..doc.page_count() {
    let mut m = doc.modify()?;
    m.reorder_pages(&[i])?;   // keep only page i
    m.garbage_collect();
    m.save(format!("page_{}.pdf", i + 1))?;
}
```

## Encryption

The high-level `Modifier` has no encryption method. Use the CLI (`justpdf encrypt` / `justpdf decrypt`), or `DocumentBuilder::set_encryption` when creating a new document.
