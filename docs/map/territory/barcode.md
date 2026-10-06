# Barcode and QR generation

## What it is
Generates QR (the `qrcode` crate), Code128, EAN-13 and Code39 (own implementation), and DataMatrix, PDF417 and Aztec (own implementation) as images and PNG. It does not build a PDF.

## Governing decisions
- [ADR-0001](../../adr/0001-crates-split-by-dependency-layer.md) — builds without render.

## Design model
- The PDF417 codeword pattern is "a simplified encoding": it writes the codeword's raw bits instead of the cluster tables, and rows carry no row indicators (inferred).
- `barcode/mod.rs` has no Reed-Solomon code — DataMatrix, PDF417 and Aztec output lacks the error correction the standards require (inferred). Error correction is not the only gap: DataMatrix has no ECC 200 module placement, pad codewords or standard symbol sizes, and Aztec has no mode message (inferred).
- `code128_pattern` holds 18 of the 107 Code 128 patterns; every other value, Start B (104) included, falls back to pattern 0 (inferred).
- The EAN-13 left half is always L-encoded; the leading digit's L/G parity is never applied and the check digit is not validated (inferred).
- `generate_barcode` passes `width.max(height)` to `generate_datamatrix` as the module size (inferred).

## Code
- `justpdf-special/src/barcode/mod.rs` — `BarcodeType`, `BarcodeImage`, `generate_qr`, `generate_qr_png`, `generate_barcode`, `generate_barcode_png`, `generate_datamatrix`, `generate_pdf417`, `generate_aztec`

## Reference behaviour
- Measured 2026-10-07 with the zxing-cpp decoder, raw and with an added quiet zone: QR `HELLO123` and Code39 `HELLO123` decode; EAN-13 decodes `0012345678905` but not `5901234123457`; Code128 (`HELLO123`, and `01` inside the 18-pattern subset), DataMatrix, PDF417 and Aztec do not decode. `generate_barcode(.., DataMatrix, 200, 200)` returns a 2000×2000 px image.

## Cross-cutting invariants
**None.**

## Blast radius
- [Crate layering](crate-layering.md) — the isolation script checks that `barcode` builds on its own.

## Known holes / open
- DataMatrix, PDF417 and Aztec output does not scan. Tracked: #55 — decided there: replace the hand-rolled non-QR encoders with the `rxing` crate and add decode round-trip tests.
- Code128 never scans, EAN-13 scans only with a leading 0, and DataMatrix via `generate_barcode` is 10× oversized. Tracked: #242, to land with #55.
- The tests check only image size and that generation succeeds.
