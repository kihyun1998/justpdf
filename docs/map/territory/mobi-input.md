# MOBI input

## What it is
Reads the text of a Mobipocket file (a PDB container, uncompressed or PalmDOC-compressed), strips its HTML, and writes it into a PDF as wrapped Courier lines.

## Governing decisions
- [ADR-0001](../../adr/0001-crates-split-by-dependency-layer.md) — `mobi` pulls render for its preview (#2, closed 2026-05-08); the isolation script checks that it does.

## Design model
Status: read from the source only.
- Container: PDB header (≥ 78 bytes), record count at 76, record list at 78; record 0 holds compression, text length and text record count. Records 1..=count are concatenated and cut at the text length.
- Compression 1 (none) and 2 (PalmDOC, `decompress_palmdoc`) are read; any other — including HUFF/CDIC (17480) — is an error. An invalid back reference is skipped, not an error.
- Title: the MOBI header's full name when it is in range, else the PDB name.
- Text: `String::from_utf8_lossy` whatever the header's text encoding, so a CP1252 book has its non-ASCII replaced (inferred); trailing-entry bytes that the MOBI extra-data flags describe are not stripped (inferred). `strip_html_tags` drops tags, turns `p`, `br`, `div`, `h1`–`h3` into newlines and decodes six entities.
- Layout: `paginate` wraps with `wrap_fixed_pitch` at `usable_width / (10 × 0.6)` characters and splits into pages of `usable_height / 12` lines (US Letter, 72 pt margins). Each line goes through `move_to` ([Format document](format-document.md)).
- Preview: `build_page_pdf` makes a one-page PDF with the same loop and renders it.
- Input bound: the record list is checked against the file length. The output buffer is `Vec::with_capacity(text_length)`, a `u32` from the header, so a 100-byte file can ask for a 4 GiB reservation (inferred); decompressed output is otherwise bounded by the records' size.

## Code
- `justpdf-formats/src/mobi/mod.rs` — `MobiDocument`, `MobiDocument::from_bytes`, `MobiDocument::open`, `MobiDocument::extract_mobi_title`, `MobiDocument::build_page_pdf`, `parse_pdb_name`, `decompress_palmdoc`, `strip_html_tags`, `paginate`, `impl FormatDocument for MobiDocument` (`render_page`, `render_page_png`, `to_pdf`)

## Reference behaviour
**None.**

## Cross-cutting invariants
- [Content text encoding](../invariant/content-text-encoding.md)

## Blast radius
- [CLI](cli.md) — `convert` reads `.mobi` and `.prc` (`justpdf-cli/tests/convert.rs`: `mobi_converts_to_pdf`, `fb2_and_mobi_convert_to_png`).
- [Render API](render-api.md) — the preview assumes `render_page_to_pixmap` returns RGBA and `render_page` PNG.
- [Document builder](document-builder.md) — assumes `show_text` writes WinAnsi for Courier and that `move_to` is absolute.

## Known holes / open
- Text encoding and trailing entries are not handled; HUFF/CDIC books are refused (Design model).
- `text_length` drives an unchecked reservation (inferred).
- Caller-supplied fonts and width-based layout. Tracked: #174
