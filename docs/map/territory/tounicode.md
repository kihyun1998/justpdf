# ToUnicode CMap

## What it is
Parses the `bfchar`/`bfrange` sections of a font's `/ToUnicode` stream into a code → Unicode mapping (`ToUnicodeCMap`), the first mapping text extraction tries. It also lists every mapped code (`entries`), which `FontEncoder` inverts to write text in a font a document already holds. The writing side, the ToUnicode CMaps justpdf generates, lives in [CJK font embedding](cjk-font-embedding.md) (`generate_to_unicode_cmap`) and [Document builder](document-builder.md).

## Governing decisions
**None.** No decision record.

## Design model
Status: from reading the source; no record.

- **Text scan, not a PostScript parse.** `parse` takes the stream as lossy UTF-8 and finds each `beginbfchar`…`endbfchar` and `beginbfrange`…`endbfrange` by substring; every `<hex>` in a section is a token (inferred, read from `parse`). `codespacerange` is not parsed, so code length is lost: `<41>` and `<0041>` are the same `u32` code.
- **bfchar** pairs go into `mappings` (a later pair for the same code wins). All `bfchar` sections are read before any `bfrange` section, so a `bfrange` array entry overrides a `bfchar` entry for the same code, while a `bfrange` with a single destination does not, because `lookup` tries `mappings` first (inferred).
- **bfrange** with one `<dst>` is kept as a range `(start, end, dst)`; `lookup` gives the one character `dst + (code − start)`. A destination that is not one scalar value — a UTF-16 surrogate pair or a multi-character string such as `<00660069>` — gives `None` for every code of the range (inferred: `char::from_u32` of the joined number fails). What ISO 32000-2 §9.10.3 asks for such a destination was not read for this note.
- **bfrange** with an array writes one `mappings` entry per element, starting at the start code; the array is not cut at the end code (inferred).
- `hex_to_unicode_string` reads a destination as UTF-16BE: two bytes as one character, four bytes as a surrogate pair or else two characters, longer strings pair by pair. A hex token of more than 8 digits overflows `u32::from_str_radix` and maps code 0 (inferred).
- `entries` lists every code with its text, sorted by code; ranges are walked for at most `MAX_RANGE_CODES` (2^20) codes in all, bfchar entries winning over range codes.
- **Four places locate and decode the stream**, each on its own, and all call `ToUnicodeCMap::parse`: `resolve_to_unicode` in the content interpreter (an indirect stream, or a direct one), `to_unicode` in `font/encoder.rs`, and `resolve_font` in the raster renderer and in the SVG device (an indirect stream only). The raster renderer stores the result in `ResolvedFont::cmap` and never reads it; the SVG device uses it for `<text>` (inferred, `git grep` 2026-10-30). The old note's "two copies of the interpretation code" was corrected: the parser is one, the lookup of the stream is four.
- **Input bounds.** The stream comes from the file, its decoded size set by [Stream filters](stream-filters.md). Nothing bounds the number of `bfchar` pairs or ranges; `lookup` walks the ranges linearly, and only `entries` has a cap (2^20 codes) (inferred).

## Code
- `justpdf-core/src/font/cmap.rs` — `ToUnicodeCMap`, `ToUnicodeCMap::parse`, `ToUnicodeCMap::lookup`, `ToUnicodeCMap::entries`, `MAX_RANGE_CODES`, `parse_bfchar_section`, `parse_bfrange_section`, `extract_hex_values`, `hex_to_unicode_string`
- `justpdf-core/src/content/interpret.rs` — `resolve_to_unicode`
- `justpdf-core/src/font/encoder.rs` — `to_unicode`
- `justpdf-core/tests/font_encoding.rs` — `to_unicode_wins_over_differences`

## Reference behaviour
**None.** Not compared against a reference. Clause to compare against: ISO 32000-2 §9.10.3.

## Cross-cutting invariants
- [Font resolution](../invariant/font-resolution.md) — one parser, the stream found and decoded separately by each consumer.

## Blast radius
- [Text extraction](text-extraction.md) — the first-priority mapping for every code; a parse change changes extracted text.
- [SVG renderer](svg-renderer.md) — `<text>` content comes from it.
- [Font encodings](font-encodings.md) — `FontEncoder` writes a character at the lowest code `entries` maps to it; a change in what `lookup` or `entries` gives changes the codes form fields are written in.
- [Font subsetting](font-subsetting.md) — the compress tests judge a subset by ToUnicode-based extraction, which does not see a lost glyph outline.
- [CJK font embedding](cjk-font-embedding.md), [Document builder](document-builder.md) — the writing-side counterparts; this parser must read back the CMaps they generate (surrogate pairs in `bfchar`, sections of 100).

## Known holes / open
- `codespacerange` is not parsed, so CMaps with variable-length codes cannot be told apart (inferred).
- A `bfrange` whose destination is a surrogate pair or several characters maps nothing (inferred).
- A `bfrange` array starting at code `0xFFFFFFFF` with two or more elements overflows `code += 1` (a panic in a debug build, wrap-around in release; inferred, not run).
- A Type0 font without ToUnicode extracts raw bytes; there is no CID → Unicode step through the character collection. Tracked: #328.
