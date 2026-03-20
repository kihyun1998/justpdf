# Modifying PDFs

## Using the Modifier

```rust
use justpdf::Modifier;

let mut modifier = Modifier::open("input.pdf")?;
modifier.remove_page(2)?;
modifier.save("output.pdf")?;
```

## Merging Documents

```rust
let mut modifier = Modifier::open("doc1.pdf")?;
modifier.append("doc2.pdf")?;
modifier.save("merged.pdf")?;
```

## Encryption

```rust
modifier.encrypt("user_password", "owner_password")?;
modifier.save("encrypted.pdf")?;
```

## Splitting

```rust
let doc = Document::open("input.pdf")?;
for i in 0..doc.page_count()? {
    let mut m = Modifier::from_document(&doc);
    m.keep_pages(&[i])?;
    m.save(format!("page_{}.pdf", i + 1))?;
}
```
