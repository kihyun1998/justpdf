# Barcode and QR generation

## What it is
Generates QR (the `qrcode` crate) and Code 128, EAN-13, Code 39, DataMatrix, PDF417 and Aztec (the `rxing` crate's encoders) as RGBA images or PNG bytes. It does not build a PDF; a caller places the image itself.

## Governing decisions
- [ADR-0001](../../adr/0001-crates-split-by-dependency-layer.md) — `barcode` builds without render; the isolation script builds it alone (`special/barcode-only`). The ADR names `qrcode` for this layer, not `rxing`.
- The hand-rolled non-QR encoders were replaced by `rxing` rather than repaired or removed — maintainer's call on #55 (closed 2026-10-07, commit 0ba1da4). They never scanned: no Reed-Solomon, no ECC 200 placement, an 18-of-107 Code 128 table falling back to pattern 0, EAN-13 without L/G parity (#242). Do not reintroduce a hand-rolled encoder without a decoder round trip.
- A panic inside rxing is caught with `catch_unwind` — maintainer's call (2026-10-07), chosen over a length cap (1555 bytes would refuse valid digit and alphanumeric input) and over shipping the panic.

## Design model
Status: the two calls above are recorded on issues; the rest is read from the source, with the dated measurements as given.
- `encode_symbol` asks rxing for the bare symbol (`EncodeHintValue::Margin("0")`, width = height = 0, so one module per matrix cell) and the quiet zone is added here per symbology: Code 128 and Code 39 10 modules each side, EAN-13 11 left and 7 right, DataMatrix 1, PDF417 2, Aztec 0 (values from the ISO symbology specs, not read raw — inferred; pinned by `quiet_zones_have_standard_width`). rxing's own defaults disagree with each other (EAN-13 9 split 4/5, Code 128 10 split 5/5, PDF417 30, DataMatrix and Aztec 0 — measured 2026-10-07). QR keeps the `qrcode` crate's 4-module zone.
- rxing is built with default features off and `encoding_rs` on: without a character-set backend it does not compile, and `no_character_set_support` panics on the first encode (measured 2026-10-07, rxing 0.9.3). `chrono` comes in unconditionally (`cargo tree`, 2026-10-30).
- PDF417 rows come from rxing 4 modules tall. rxing's PDF417 writer rotates the symbol 90° when the requested aspect disagrees with the symbol's; with width = height = 0 it rotates only a taller-than-wide symbol, and rxing picks wider-than-tall dimensions for every length measured (1–400 characters, 2026-10-07). `pdf417_is_upright` pins it.
- EAN-13 requires 13 digits (`encode_ean13`): rxing would accept 12 and compute the check digit; rxing rejects a wrong check digit (`Contents do not pass checksum`). Tests `ean13_twelve_digits_is_rejected`, `ean13_wrong_check_digit_is_rejected`.
- `catch_unwind` in `encode_symbol`: rxing 0.9.3 panics on DataMatrix data past the 144 × 144 symbol's capacity (`.unwrap().unwrap()` on the symbol lookup in `encoder_context.rs`): 2336 `x` panic and 2335 encode, digits from 3117 (measured 2026-10-07; `datamatrix_over_capacity_is_an_error`). It does not hold under `panic = "abort"`, and the default panic hook still prints to stderr.
- Code 39 encodes the upper-cased input (`encode_code39`); rxing would otherwise switch lower case to Full ASCII pairs, which a plain Code 39 reader returns as `+H+E…` (`roundtrip_code39_upper_cases`).
- Sizing: `generate_barcode` gives 1D symbols `height` pixels and the largest whole module width within `width`; 2D symbols one square module size fitting `width` × `height` (the old PDF417 stretched modules independently); at least one pixel per module. `generate_aztec` and `generate_pdf417` go through it; `generate_datamatrix` takes a module size; `generate_qr` takes a minimum size.
- Input bound: data length is bounded by each symbology's capacity (rxing or `qrcode` return an error). The output size is not: `width`, `height`, `size` and `module_size` are the caller's `u32`s, and `render` allocates `total_w * module_w * total_h * module_h * 4` bytes, so a large request allocates without limit (inferred).

## Code
- `justpdf-special/src/barcode/mod.rs` — `BarcodeType`, `BarcodeImage`, `generate_qr`, `generate_qr_png`, `generate_barcode`, `generate_barcode_png`, `generate_datamatrix`, `generate_datamatrix_png`, `generate_pdf417`, `generate_pdf417_png`, `generate_aztec`, `generate_aztec_png`; private `Symbol`, `QuietZone`, `encode_symbol`, `encode_ean13`, `encode_code39`, `encode_datamatrix`, `encode_pdf417`, `encode_aztec`, `render`, `encode_png`
- `justpdf-special/Cargo.toml` — `rxing` features (`encoders`, the four symbologies, `encoding_rs`) and its decoder dev-dependency

## Reference behaviour
- The tests decode every symbology from the generated image with rxing's decoder (`helpers::detect_in_luma`, dev-dependency only) (measured 2026-10-30: all barcode tests pass). rxing's decoder is lenient about quiet zones — removing every quiet zone left all but EAN-13 decoding (measured 2026-10-07) — so quiet-zone widths are asserted on pixels, not through decoding.
- zxing-cpp, measured 2026-10-07: rxing's output for all six non-QR symbologies decodes. Before the replacement zxing-cpp read only QR, Code 39 and EAN-13 with a leading 0; Code 128, DataMatrix, PDF417 and Aztec never, and `generate_barcode(.., DataMatrix, 200, 200)` returned 2000 × 2000 px.

## Cross-cutting invariants
**None.**

## Blast radius
- [Crate layering](crate-layering.md) — `barcode` must build alone and without render; rxing's feature set is part of that.

## Known holes / open
- The real fix for the DataMatrix capacity panic is in rxing (return an error from the symbol lookup), reported upstream as rxing-core/rxing#106; `catch_unwind` stands in for it. Bump and pin the error once rxing 0.9.4 is out. Tracked: #276
- `rxing` is Apache-2.0 only while justpdf is MIT OR Apache-2.0; enabling `barcode` brings Apache-2.0 terms (`justpdf-special/README.md`).
- No bound on output image size (Design model).
