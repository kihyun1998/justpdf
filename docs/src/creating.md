# Creating PDFs

## Document Builder

```rust
use justpdf::{DocumentBuilder, PageBuilder};

let mut builder = DocumentBuilder::new();
let font = builder.add_standard_font("Helvetica");

let mut page = PageBuilder::new(612.0, 792.0); // US Letter
page.begin_text();
page.set_font(&font, 12.0);
page.move_to(72.0, 720.0);
page.show_text("Hello, World!");
page.end_text();
builder.add_page(page);

builder.set_title("My document");
let pdf_bytes = builder.build()?;
std::fs::write("output.pdf", &pdf_bytes)?;
```

Standard fonts are not embedded and cover ASCII text only.

## Adding Images

```rust
use justpdf::{embed_jpeg, DocumentBuilder, PageBuilder};

let mut builder = DocumentBuilder::new();
let jpeg = std::fs::read("photo.jpg")?;
let (name, image_ref) = embed_jpeg(&mut builder, &jpeg)?;

let mut page = PageBuilder::new(612.0, 792.0);
page.add_image(&name, image_ref);
page.draw_image(&name, 72.0, 500.0, 200.0, 150.0); // x, y, width, height
builder.add_page(page);
```

`embed_png` works the same way for PNG data.

## Encryption

Call `set_encryption` before `build()`:

```rust
use justpdf::core::crypto::{EncryptionConfig, EncryptionMethod, Permissions};

builder.set_encryption(EncryptionConfig {
    user_password: b"user".to_vec(),
    owner_password: b"owner".to_vec(),
    permissions: Permissions::allow_all(),
    method: EncryptionMethod::AES256,
    encrypt_metadata: true,
});
let encrypted = builder.build()?;
```

To encrypt an existing file, use the CLI: `justpdf encrypt`.
