# Barcode and QR generation

## What it is
Generates QR (the `qrcode` crate), Code128, EAN-13 and Code39 (own implementation), and DataMatrix, PDF417 and Aztec (own implementation) as images and PNG. It does not build a PDF.

## Governing decisions
- [ADR-0001](../../adr/0001-crates-split-by-dependency-layer.md) — builds without render.

## Design model
- The PDF417 codeword pattern is "a simplified encoding".
- `barcode/mod.rs` has no Reed-Solomon code — DataMatrix, PDF417 and Aztec output lacks the error correction the standards require and may not scan (inferred).

## Code
- `justpdf-special/src/barcode/mod.rs` — `BarcodeType`, `BarcodeImage`, `generate_qr`, `generate_qr_png`, `generate_barcode`, `generate_barcode_png`, `generate_datamatrix`, `generate_pdf417`, `generate_aztec`

## Reference behaviour
**None.** No record of checking with a real scanner.

## Cross-cutting invariants
**None.**

## Blast radius
- [Crate layering](crate-layering.md) — the isolation script checks that `barcode` builds on its own.

## Known holes / open
- No error correction for 2D codes (above).
- Tracked: #55 (2D barcode error correction)
