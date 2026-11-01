# Format conversion contract (`FormatDocument`)

## What it is
The common trait that non-PDF inputs implement: metadata, page count, page info, page text, a preview render (RGBA and PNG) and `to_pdf`. Eight types implement it — XPS, EPUB, Office, SVG, CBZ, MOBI, FB2 and plain text (`git grep -n "impl FormatDocument for"`); each input format note describes one implementation. The crate's shared pieces — the error type, the fixed-pitch wrapper and the feature gates — live here too.

## Governing decisions
- [ADR-0001](../../adr/0001-crates-split-by-dependency-layer.md) — `justpdf-formats` is its own layer over core, adding `zip` and `roxmltree`; the `justpdf` facade leaves it out. The ADR does not name `image` (`png`, `jpeg`), which `justpdf-formats/Cargo.toml` takes unconditionally (inferred, read 2026-10-30).
- Render is optional and pulled only by the preview features `plaintext`, `mobi` and `fb2` (#2, closed 2026-05-08). `scripts/check-feature-isolation.sh` checks it: `xps`, `epub`, `svg`, `office`, `cbz` and no features pull no `justpdf-render`, the three preview features do, and every feature compiles alone (#35).

## Design model
Status: the rules below are read from the source; no record states them.
- **roxmltree 0.21** (#449): since 0.21.0 `Node::attribute("name")` matches the local name whatever the namespace, where 0.20 matched only an attribute without one, so a lookup now also finds a prefixed attribute (`xlink:href` for `"href"`). Every name the readers look up is unprefixed in its format, and the suites passed unchanged after the bump (inferred for real files; the CHANGELOG and `attribute_node` read raw at tag `v0.21.2`).
- **All PDF output goes through core's [Document builder](document-builder.md)** (`DocumentBuilder`, `PageBuilder`, `embed_rgb`); no format writes PDF syntax by hand (inferred: `git grep -n "PdfWriter\|b\"BT" justpdf-formats/src` finds none).
- **Text formats** (xps, epub, office, plaintext, mobi, fb2) draw with `add_standard_font("Courier")` at 10 pt, one `show_text` per line, so text is WinAnsi with `?` beyond it — [Content text encoding](../invariant/content-text-encoding.md) (measured for plaintext and DOCX in `win_ansi_text_round_trips_through_pdf`, #34). Plaintext, MOBI and FB2 wrap with `common::wrap_fixed_pitch`, which counts and cuts by `char`; until #34 each had a byte-slicing copy that panicked inside a multi-byte character (measured on #34, 2026-10-07; tests `wrapping_a_non_ascii_line_does_not_panic` in plaintext and MOBI, `wrapping_a_non_ascii_paragraph_does_not_panic` in FB2). EPUB, XPS and Office do not wrap.
- **Every text format positions each line with `PageBuilder::move_to(margin, y)` inside one `BT`…`ET`, passing absolute coordinates**, which is what `move_to` takes ([Document builder](document-builder.md)). Each line is drawn at its own `y` on the page (measured 2026-10-30: all six converters' lines extract inside the page, top to bottom; plaintext and FB2 are pinned by `every_line_is_drawn_on_the_page_top_to_bottom` and `every_line_of_a_section_is_drawn_on_the_page_top_to_bottom`; #407).
- **Image formats** (cbz, svg) rasterize, embed the raster as a Flate DeviceRGB image XObject with `embed_rgb`, and draw it over the whole page with `draw_image` (#88, commit fccf37a — before it they used `draw_inline_image` with no `cm`, so the image landed at 1 pt × 1 pt). justpdf's renderer draws these pages (measured 2026-10-30: an SVG and a CBZ converted and rendered back with `justpdf convert` show the raster; until #229 it decoded the stream twice and drew them blank — see [SVG input](svg-input.md)).
- **The preview** (`render_page`, `render_page_png`) comes in three kinds: build a one-page PDF and draw it with render (plaintext, mobi, fb2; epub and office borrow `PlainTextDocument` only under `cfg(feature = "plaintext")` and otherwise return `FormatError::preview_needs_plaintext()` after the index check); an own raster (svg, cbz); a blank white page (xps).
- **`page_count` is the format's own unit** (the trait doc says "pages (or chapters, images, etc.)"). For EPUB it counts chapters while `to_pdf` splits a chapter over several PDF pages, so an index into `FormatDocument` is not a PDF page index there; the other formats emit one PDF page per unit (inferred from each `to_pdf`).
- **Modules compile only behind their feature** (`lib.rs`); `FormatError` variants `Zip`, `Xml` and `Render` are themselves feature-gated (`error.rs`).
- **Input size:** nothing in the crate bounds it. Every `open` reads the whole file; the ZIP formats (xps, epub, office, cbz) read each entry they want to its full decompressed size (`read_to_string` / `read_to_end`, three private `read_zip_text` copies), SVG and XPS size their page rasters with core `image::rgba_len` (#426) and the PDF-backed previews go through the renderer's 16384-pixel side cap, while CBZ resizes to the requested DPI with no cap (inferred; see each format note).

## Code
- `justpdf-formats/src/common.rs` — `FormatDocument`, `FormatDocument::text`, `FormatPage`, `FormatMetadata`, `RenderedPage`, `wrap_fixed_pitch`
- `justpdf-formats/src/lib.rs` — feature-gated `pub mod` per format; re-exports `FormatDocument`, `FormatMetadata`, `FormatPage`, `FormatError`, `Result`
- `justpdf-formats/src/error.rs` — `FormatError`, `FormatError::preview_needs_plaintext`, `Result`
- `justpdf-formats/Cargo.toml` — features `xps`, `epub`, `svg`, `office`, `cbz`, `plaintext`, `mobi`, `fb2`, `all`

## Reference behaviour
**None.** MuPDF (example) reads the same input formats (`docs/mupdf-feature-analysis.md`); no record compares conversion results with it.

## Cross-cutting invariants
- [Content text encoding](../invariant/content-text-encoding.md)

## Blast radius
- Each implementation — [xps](xps-input.md), [epub](epub-input.md), [office](office-input.md), [svg](svg-input.md), [cbz](cbz-input.md), [mobi](mobi-input.md), [fb2](fb2-input.md), [plaintext](plaintext-input.md): a change to the trait's signature or to what a method promises (the unit `page_count` counts, RGBA vs RGB in `RenderedPage`) has to hold in all eight.
- [Document builder](document-builder.md) — every `to_pdf` assumes `show_text` writes WinAnsi for a standard font, `draw_image` maps the unit square to the given box, and that `move_to` is absolute.
- [Render API](render-api.md) — five previews assume `render_page_to_pixmap` returns RGBA rows from the top and `render_page` returns PNG bytes.
- [Crate layering](crate-layering.md) — which features may pull render; the isolation script reads this crate's feature list.
- [CLI](cli.md) — `convert` is the only consumer in the repo; it assumes every type here can be built from a path with `open` and that `render_page_png(0, 150.0)` works for each.
- [Published docs](published-docs.md) — the trait is public API.

## Known holes / open
- `to_pdf` tests in xps, epub, office, mobi, cbz and svg check only `starts_with(b"%PDF")`; plaintext and FB2 pin every line's position on the first page; Office reads back one line.
- Without `plaintext`, `mobi` or `fb2`, the crate compiles with a dead-code warning, `function wrap_fixed_pitch is never used` (measured 2026-10-30: `cargo check -p justpdf-formats --features epub`, and `office`). The isolation script's `assert_check_ok` discards stderr and does not deny warnings, and clippy runs with all features, so CI does not see it.
- No bound on input size (Design model).
- Text beyond WinAnsi through caller-supplied fonts, and width-based layout shared by all six text converters. Tracked: #174
