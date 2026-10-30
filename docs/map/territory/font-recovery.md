# Font recovery

## What it is
`font::recovery` picks a Standard 14 substitute for a font from its name alone: it guesses whether the name is bold, italic, monospaced, serif, symbol or dingbats and maps it to one of the 14 (`find_substitute`). The raster renderer draws a simple font that has no usable embedded program with the bundled URW program for that name (#227). `fallback_font_info` builds a whole `FontInfo` for the substitute but has no caller.

## Governing decisions
**None.** No decision record.

## Design model
Status: from reading the source; no record.

- **Order of the rules** (`find_substitute`), after stripping a six-letter subset tag: a Standard 14 name passes through as its canonical variant; then a dingbats name (`ZapfDingbats`, `Wingdings`, `Webdings`, …) → ZapfDingbats; a symbol name → Symbol; a monospace name → Courier; a serif name → Times; a known sans-serif family (Arial, Verdana, Calibri, Noto, Roboto, Liberation, …) → Helvetica; `ArialMT`/`Arial-`/`ArialNarrow` → Helvetica, `TimesNewRoman` → Times, `CourierNew` → Courier; anything else → Helvetica (code comment "most commonly used fallback in PDF viewers"). Bold and italic pick the variant (`select_variant`). Names are matched case-insensitively by substring (`contains_ci`) (inferred).
- Only the name is read. The descriptor's `/Flags` (FixedPitch, Serif, Symbolic, Italic) and `/FontWeight` are not consulted (inferred: the renderer passes `info.base_font` only).
- CJK names (e.g. `MSGothic`) go to Helvetica (unit test `find_substitute(b"MSGothic") == b"Helvetica"`).
- **Where it is used**: `resolve_font` in the raster renderer substitutes when `extract_font_data` gives no program `GlyphSource::parse` accepts, and only for a font that is neither Type0 nor Type3 (`crate::substitute::base14_program(find_substitute(&info.base_font))`). `base14_program` returns a program only with the `base14-fonts` feature of `justpdf-render`; without it the name is computed and nothing is drawn from it (inferred, `#[cfg(feature = "base14-fonts")]` in `substitute.rs`). So a non-embedded Type0 (CJK) font gets no substitute and is drawn as boxes (inferred; pinned by `render_substitute.rs` per #293). The SVG renderer and text extraction do not call it (`git grep`, 2026-10-30).
- `fallback_font_info` (substitute's `standard14_widths`, WinAnsi encoding, a synthetic descriptor: ascent 750, descent −250, StemV 140 bold / 76 otherwise) has no caller in the workspace (measured, `git grep fallback_font_info` 2026-10-30).
- `recovery.rs` has its own `strip_subset_prefix`, a copy of `standard14::strip_subset_prefix` (inferred).
- **Input bound.** The name comes from the file; matching is linear in its length over a fixed list of patterns (inferred).

## Code
- `justpdf-core/src/font/recovery.rs` — `find_substitute`, `fallback_font_info`, `select_variant`, `is_bold_name`, `is_italic_name`, `is_monospace_name`, `is_serif_name`, `is_symbol_name`, `is_dingbats_name`, `contains_ci`, `strip_subset_prefix`
- `justpdf-render/src/interpreter.rs` — `resolve_font` (the substitution call)
- `justpdf-render/src/substitute.rs` — `base14_program`

Measured: the 8 tests in `justpdf-render/tests/render_substitute.rs` pass at 88b2a08 (2026-10-30).

## Reference behaviour
**None.** Not compared against a reference. Issue #293 cites MuPDF's CJK substitutes by `/CIDSystemInfo` ordering as prior art, inferred from MuPDF's repository layout and not read raw (#293, 2026-10).

## Cross-cutting invariants
**None.**

## Blast radius
- [Glyph rendering](glyph-rendering.md) — draws a simple font with no usable program from the URW program for the name `find_substitute` returns; a change to the name rules changes which face such text is drawn in.
- [Font loading](font-loading.md) — the substitute draws with the font's `/Widths`, or with the approximate `standard14_widths` when it has none; a substitute whose real advances differ from those widths shows uneven spacing (#292).
- [Text extraction](text-extraction.md) — does not substitute; widths for a font without `/Widths` come from `parse_font_info`, not from here.

## Known holes / open
- A non-embedded Type0 (CJK) font gets no substitute and is drawn as boxes. Tracked: #293.
- Only the name is read; the descriptor's flags are not (inferred).
- `fallback_font_info` has no caller (measured, `git grep`).
- The SVG renderer does not substitute; it emits `<text font-family=…>` from the font's name (`svg_device.rs`, read 2026-10-30).
