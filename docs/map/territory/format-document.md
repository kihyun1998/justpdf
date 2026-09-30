# Format conversion contract (`FormatDocument`)

## What it is
The common trait that non-PDF inputs (XPS, EPUB, Office, SVG, CBZ, MOBI, FB2, text) implement: metadata, page count, page text, preview render, `to_pdf`. Each input format note is an implementation of this contract.

## Governing decisions
- [ADR-0001](../../adr/0001-crates-split-by-dependency-layer.md) — formats is a separate layer that adds `zip` and `roxmltree`, and it pulls in render only behind feature flags.

## Design model
- **All PDF generation goes through core [Document builder](document-builder.md).** Format code does not write PDF syntax by hand.
- Text formats (xps, epub, office, plaintext, mobi, fb2) use `add_standard_font("Courier")` at 10pt with a `show_text` per line — non-ASCII and CJK break (inferred) — [Content text encoding](../invariant/content-text-encoding.md).
- Image formats (cbz, svg) rasterize, embed the raster as a Flate image XObject with `embed_rgb` and draw it over the whole page with `draw_image` (#88 — before it they used `draw_inline_image` with no `cm`, so the image landed at 1pt×1pt). justpdf's own renderer draws those pages blank: see [Render images](render-images.md), #42.
- The preview render (`render_page`) splits into formats that build a one-page PDF and draw it with the render crate (plaintext, mobi, fb2, and epub and office, which borrow plaintext), formats with their own raster (svg, cbz), and a blank page (xps).
- Modules are compiled only behind feature flags (`lib.rs`).

## Code
- `justpdf-formats/src/common.rs` — `FormatDocument`, `FormatPage`, `FormatMetadata`, `RenderedPage`
- `justpdf-formats/src/lib.rs` — `FormatDocument`
- `justpdf-formats/Cargo.toml` — `plaintext`, `mobi`, `fb2`, `all`

## Reference behaviour
**None.** MuPDF (example) supports the same input formats (`docs/mupdf-feature-analysis.md`). There is no record comparing conversion results.

## Cross-cutting invariants
- [Content text encoding](../invariant/content-text-encoding.md)

## Blast radius
- Each input format — [xps](xps-input.md), [epub](epub-input.md), [office](office-input.md), [svg](svg-input.md), [cbz](cbz-input.md), [mobi](mobi-input.md), [fb2](fb2-input.md), [plaintext](plaintext-input.md). A trait signature change reaches all eight implementations.
- [Document builder](document-builder.md) — the layer under every `to_pdf`.
- [Render API](render-api.md) — preview.
- [Crate layering](crate-layering.md) — which features turn render off.
- [CLI](cli.md) — the only consumer inside the repo (`convert`).

## Known holes / open
- Most `to_pdf` tests check only `starts_with(b"%PDF")`.
- Tracked: #34 (non-ASCII content text)
