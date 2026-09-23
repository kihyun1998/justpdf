# justpdf-special

Special PDF features for the [justpdf](https://github.com/kihyun1998/justpdf) project.

Provides OCR (via the external `tesseract` program), barcode generation, ZUGFeRD invoice reading, BiDi text analysis, and image deskew.

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
| `ocr`      | OCR via Tesseract (pulls in `justpdf-render`) |
| `barcode`  | QR, Code128, EAN-13, Code39; DataMatrix, PDF417, Aztec are experimental (no error correction yet, may not scan — #55) |
| `zugferd`  | ZUGFeRD/Factur-X invoice extraction  |
| `bidi`     | Bidirectional text run analysis      |
| `deskew`   | Scanned image deskew correction      |

## Repository

[https://github.com/kihyun1998/justpdf](https://github.com/kihyun1998/justpdf)

## License

MIT OR Apache-2.0
