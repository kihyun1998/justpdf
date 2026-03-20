# justpdf-special

Special PDF features for the [justpdf](https://github.com/kihyun1998/justpdf) project.

Provides OCR, barcode generation/reading, ZUGFeRD invoicing, BiDi text support, and image deskew.

## Usage

All features are feature-gated. Enable what you need:

```toml
[dependencies]
justpdf-special = { version = "0.1", features = ["all"] }
```

### Example

```rust
use justpdf_special::barcode;
let png = barcode::generate_qr_png("https://example.com", 256)?;
```

## Features

| Feature    | Description                          |
|------------|--------------------------------------|
| `ocr`      | Optical character recognition        |
| `barcode`  | QR, DataMatrix, PDF417, Aztec codes  |
| `zugferd`  | ZUGFeRD/Factur-X invoice handling    |
| `bidi`     | Bidirectional text layout            |
| `deskew`   | Scanned image deskew correction      |

## Repository

[https://github.com/kihyun1998/justpdf](https://github.com/kihyun1998/justpdf)

## License

MIT OR Apache-2.0
