# Plaintext input

## What it is
Wraps and paginates plain text and writes it into a PDF in Courier. Its one-page PDF and render path is also the preview of EPUB and Office.

## Governing decisions
- [ADR-0001](../../adr/0001-crates-split-by-dependency-layer.md) — `plaintext` pulls render for its preview (#2, closed 2026-05-08); the isolation script checks that it does.
- EPUB and Office use this module only under `cfg(feature = "plaintext")` and otherwise return an error — maintainer's call on #35 (2026-10-02), see [EPUB input](epub-input.md).

## Design model
Status: read from the source only.
- Defaults (`from_string`): US Letter, 72 pt margins, Courier 10 pt, line spacing 1.2; `from_string_with_options` takes all five.
- Layout: character width is hard-coded as `font_size * 0.6`, Courier's 600/1000 advance — the same value as core's `standard14_widths` for Courier, which it does not call ([Font loading](font-loading.md)). Lines wrap with `wrap_fixed_pitch`; pages hold `usable_height / line_height` lines; empty text gives one empty page.
- `to_pdf` and `build_page_pdf` (the preview) draw each line with `move_to(margin, y)` inside one `BT`, so only the first line of each page is visible — measured 2026-10-30 at 88b2a08 with a three-line `.txt` through `justpdf convert` to PDF and PNG ([Format document](format-document.md)).
- `render_page` builds a one-page PDF and draws it with `justpdf_render::render_page_to_pixmap`; `render_page_png` with `render_page`. EPUB and Office call `PlainTextDocument::from_string(..).render_page(0, ..)`, so **this module's feature decides whether two other formats can preview**.
- Input bound: none; `open` reads the file whole with `read_to_string`, so a non-UTF-8 file is an I/O error (inferred).

## Code
- `justpdf-formats/src/plaintext/mod.rs` — `PlainTextDocument`, `PlainTextDocument::from_string`, `PlainTextDocument::from_string_with_options`, `PlainTextDocument::open`, `PlainTextDocument::build_page_pdf`, `impl FormatDocument for PlainTextDocument` (`render_page`, `render_page_png`, `to_pdf`); tests `wrapping_a_non_ascii_line_does_not_panic`, `win_ansi_text_round_trips_through_pdf`

## Reference behaviour
**None.**

## Cross-cutting invariants
- [Content text encoding](../invariant/content-text-encoding.md)

## Blast radius
- [EPUB input](epub-input.md), [Office input](office-input.md) — their previews are this module's page 0; a change to `from_string`'s defaults or pagination changes what they show.
- [Crate layering](crate-layering.md) — the `plaintext` feature is the one that pulls render for three formats.
- [Render API](render-api.md) — the preview assumes `render_page_to_pixmap` returns RGBA and `render_page` PNG.
- [Document builder](document-builder.md) — assumes `show_text` writes WinAnsi for Courier and that `move_to` is absolute.

## Known holes / open
- Only the first line of each page is visible (Design model).
- The 0.6 width is right only for Courier; any other font breaks the wrapping. Tracked: #174
