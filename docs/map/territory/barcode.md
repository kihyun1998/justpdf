# Barcode and QR generation

## What it is
Generates QR (the `qrcode` crate) and Code128, EAN-13, Code39, DataMatrix, PDF417 and Aztec (the `rxing` crate's encoders) as images and PNG. It does not build a PDF.

## Governing decisions
- [ADR-0001](../../adr/0001-crates-split-by-dependency-layer.md) — builds without render.
- #55 — the hand-rolled non-QR encoders were replaced by `rxing` rather than repaired or removed (maintainer's call). They never scanned: no Reed-Solomon, no ECC 200 placement, an 18-of-107 Code 128 table falling back to pattern 0, EAN-13 without L/G parity (#242). Do not reintroduce a hand-rolled encoder without a decoder round trip.

## Design model
- rxing is asked for the bare symbol (`EncodeHints` `Margin` = 0, width = height = 0, so one module per matrix cell) and the quiet zone is added here, per symbology at the standard minimum: Code128 and Code39 10 modules each side, EAN-13 11 left and 7 right, DataMatrix 1, PDF417 2, Aztec 0 (values from the ISO symbology specs, not read raw — inferred). rxing's own defaults disagree with each other (EAN-13 9 split 4/5, Code128 10 split 5/5, PDF417 30, DataMatrix and Aztec 0 — measured 2026-10-07). QR keeps the `qrcode` crate's 4-module zone.
- rxing needs `encoding_rs` once its default features are off: without a character-set backend it does not compile, and `no_character_set_support` panics on the first encode (measured 2026-10-07, rxing 0.9.3). `chrono` comes in unconditionally.
- PDF417 rows come from rxing 4 modules tall. rxing's PDF417 writer rotates the symbol 90° when the requested aspect disagrees with the symbol's; with width = height = 0 it rotates only a taller-than-wide symbol, and rxing picks wider-than-tall dimensions for every length measured (1–400 characters). `pdf417_is_upright` pins that.
- EAN-13 keeps requiring 13 digits: rxing would accept 12 and compute the check digit. rxing rejects a wrong check digit (`Contents do not pass checksum`).
- `encode_symbol` turns a panic inside rxing into an error with `catch_unwind`. rxing 0.9.3 panics on DataMatrix data past the 144x144 symbol's capacity (`.unwrap().unwrap()` on the symbol lookup in `encoder_context.rs`): 2336 `x` panic and 2335 encode, digits from 3117 (measured 2026-10-07). Maintainer's call (2026-10-07), chosen over a length cap (1555 bytes would refuse valid digit and alphanumeric input) and over shipping the panic. It does not hold under `panic = "abort"`, and the default panic hook still prints to stderr.
- Code39 encodes the upper-cased input; rxing would otherwise switch lower case to Full ASCII pairs, which a plain Code39 reader returns as `+H+E…`.
- `generate_barcode` fits 2D symbols into `width` x `height` with one square module size (the old PDF417 stretched modules independently). `generate_aztec` and `generate_pdf417` go through it; `generate_datamatrix` takes a module size.

## Code
- `justpdf-special/src/barcode/mod.rs` — `BarcodeType`, `BarcodeImage`, `generate_qr`, `generate_qr_png`, `generate_barcode`, `generate_barcode_png`, `generate_datamatrix`, `generate_pdf417`, `generate_aztec`; private `Symbol`, `QuietZone`, `encode_symbol`, `render`

## Reference behaviour
- The tests decode every symbology from the generated image, as produced, with rxing's decoder (`helpers::detect_in_luma`, dev-dependency only). rxing's decoder is lenient about quiet zones — removing every quiet zone left all but EAN-13 decoding (measured) — so quiet-zone widths are asserted on pixels, not through decoding.
- Independent check, measured 2026-10-07: rxing's encoder output for all six non-QR symbologies also decodes in zxing-cpp. Before the replacement, zxing-cpp read only QR, Code39 and EAN-13 with a leading 0; Code128, DataMatrix, PDF417 and Aztec never, and `generate_barcode(.., DataMatrix, 200, 200)` returned 2000×2000 px.

## Cross-cutting invariants
**None.**

## Blast radius
- [Crate layering](crate-layering.md) — the isolation script checks that `barcode` builds on its own.

## Known holes / open
- The real fix for the DataMatrix capacity panic is in rxing (return an error from the symbol lookup); the `catch_unwind` above stands in for it. Reported upstream: rxing-core/rxing#106.
- `rxing` is Apache-2.0 only, while justpdf is MIT OR Apache-2.0; enabling `barcode` brings Apache-2.0 terms (noted in the crate README).
