# Creating PDFs

## Document Builder

```rust
use justpdf_core::writer::DocumentBuilder;

let mut builder = DocumentBuilder::new();
let mut page = builder.add_page(612.0, 792.0); // Letter size
page.set_font("Helvetica", 12.0);
page.draw_text(72.0, 720.0, "Hello, World!");
let pdf_bytes = builder.finish()?;
std::fs::write("output.pdf", &pdf_bytes)?;
```

## Adding Images

```rust
let image_data = std::fs::read("photo.jpg")?;
page.draw_image(72.0, 500.0, 200.0, 150.0, &image_data)?;
```

## Font Embedding

justpdf supports embedding TrueType and OpenType fonts for full Unicode coverage.
