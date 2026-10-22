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

Standard fonts are not embedded. `show_text` encodes text as WinAnsi (Windows-1252), so Western European characters such as `é`, `€` and `—` work; any other character is drawn as `?`. `Symbol` and `ZapfDingbats` instead take the characters of their own sets (`α`, `∀`, `✓`, …).

## Text beyond WinAnsi

For Korean, Japanese, Chinese, Cyrillic and other scripts, register a TrueType font you supply. It is embedded subset to the glyphs you draw:

```rust
use justpdf::{DocumentBuilder, FontProgram, PageBuilder};

let mut builder = DocumentBuilder::new();
let korean = builder.add_font(&FontProgram::parse(std::fs::read("NanumGothic.ttf")?, 0)?);
let latin = builder.add_font(&FontProgram::parse(std::fs::read("NotoSans-Regular.ttf")?, 0)?);
let font = korean.with_fallback(&latin); // each character in the first font that has it

let mut page = PageBuilder::new(612.0, 792.0);
page.begin_text();
page.set_font(&font, 12.0);
page.move_to(72.0, 720.0);
page.show_text("안녕하세요, café");
page.end_text();
builder.add_page(page);

// Characters no font in the chain has are drawn as .notdef and listed here.
assert!(builder.missing_chars().is_empty());
```

`FontProgram::parse(bytes, face_index)` takes a `.ttf`, or one face of a `.ttc`. CFF-flavoured OpenType fonts (`.otf` with `OTTO`) are not supported yet. Characters map one-to-one to glyphs: there is no shaping, bidi or vertical writing, so Arabic, Hebrew and Indic scripts do not draw correctly.

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

To encrypt an existing file, see [Modifying PDFs](./modifying.md#encryption) or use the CLI: `justpdf encrypt`.
