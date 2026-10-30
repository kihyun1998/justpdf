# EPUB input

## What it is
Reads the spine documents of an EPUB through its OPF, strips them to text, and writes the text into a PDF as Courier lines. Rejects a DRM-protected file.

## Governing decisions
- [ADR-0001](../../adr/0001-crates-split-by-dependency-layer.md) — `epub` (`zip`, `roxmltree`) builds without render (#2).
- With `plaintext` off, the preview returns an error rather than the feature pulling `plaintext` — maintainer's call on #35 (2026-10-02), chosen to keep `epub` render-free as #2 set.

## Design model
Status: read from the source; the preview rule comes from #35.
- DRM: the presence of `META-INF/encryption.xml` is an error ("DRM-protected EPUB is not supported"). That file also lists font obfuscation, which is not DRM, so such a book is refused too (inferred).
- Structure: `META-INF/container.xml` → first `rootfile@full-path` → OPF; the first `title` and `creator` become metadata; spine `itemref@idref` → manifest `item@href`, resolved against the OPF directory and, failing that, as given. Unreadable spine items are skipped silently; no readable chapter is an error.
- Text: `strip_xml_tags` walks the XHTML with roxmltree (skipping `script` and `style`, newlines around block elements) and falls back to dropping everything between `<` and `>` when the XHTML does not parse; `normalize_whitespace` collapses runs and blank lines.
- One `FormatDocument` page per chapter. `to_pdf` paginates each chapter at 72 pt margins, 12 pt lines (`lines_per_page`), so a chapter may take several PDF pages; lines are not wrapped and run off the right edge. Each line goes through `move_to` ([Format document](format-document.md)).
- Preview (`render_page`, `render_page_png`): under `cfg(feature = "plaintext")` it builds a `PlainTextDocument` from the chapter and renders its page 0, so only the chapter's first page is previewed. Without `plaintext` it checks the index (`OutOfRange`) and returns `FormatError::preview_needs_plaintext()` (``"page preview needs the `plaintext` feature"``). Tests: `test_preview_needs_plaintext_feature`, `test_preview_renders_with_plaintext`, `test_preview_out_of_range` (measured 2026-10-30: pass with `--features epub`).
- `epub` compiles alone (measured 2026-10-30, `cargo check -p justpdf-formats --features epub`), with one dead-code warning for `wrap_fixed_pitch` — the old "compiles without warnings" (measured 2026-10-07) no longer holds; until #35 it failed with `E0433: cannot find plaintext in crate`.
- Input bound: none. Each spine item and the OPF are read whole from the ZIP (inferred).

## Code
- `justpdf-formats/src/epub/mod.rs` — `EpubDocument`, `EpubDocument::open`, `EpubDocument::from_bytes`, `find_opf_path`, `parse_opf`, `strip_xml_tags`, `extract_text_recursive`, `normalize_whitespace`, `impl FormatDocument for EpubDocument` (`render_page`, `render_page_png`, `to_pdf`)
- `justpdf-formats/src/error.rs` — `FormatError::preview_needs_plaintext`

## Reference behaviour
**None.**

## Cross-cutting invariants
- [Content text encoding](../invariant/content-text-encoding.md)

## Blast radius
- [Plaintext input](plaintext-input.md) — the preview is `PlainTextDocument`'s page 0; a change to its constructor or pagination changes what EPUB previews.
- [Crate layering](crate-layering.md) — `epub` must stay render-free and compile alone; the isolation script checks both.
- [Format document](format-document.md) — the contract; EPUB is the one format whose `page_count` is not its PDF page count.
- [Document builder](document-builder.md) — assumes `show_text` writes WinAnsi for Courier and that `move_to` is absolute.

## Known holes / open
- Long lines are not wrapped (Design model).
- A book with font obfuscation in `encryption.xml` is refused as DRM (inferred).
- No bound on entry size.
- Dead-code warning when built without `plaintext` ([Format document](format-document.md)).
- Wrapping and caller-supplied fonts. Tracked: #174
