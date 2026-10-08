# ToUnicode CMap

## What it is
Parses the `bfchar`/`bfrange` sections of a font's `/ToUnicode` stream into a code → Unicode mapping. The writing side (generating ToUnicode for embedded fonts) lives separately in [Document builder](document-builder.md) and [CJK font embedding](cjk-font-embedding.md).

## Governing decisions
**None.**

## Design model
- Scans the text for `beginbfchar`/`beginbfrange`. `codespacerange` is not parsed.
- ToUnicode interpretation is duplicated in two places: `resolve_to_unicode` in the [Content interpreter](content-interpreter.md) (for text extraction) and `resolve_font` in the renderer (SVG included). Only the SVG device uses the render side's result.

## Code
- `justpdf-core/src/font/cmap.rs` — `ToUnicodeCMap`, `parse`, `lookup`, `parse_bfchar_section`, `parse_bfrange_section`, `hex_to_unicode_string`

## Reference behaviour
**None.** Clause to compare against: ISO 32000-2 §9.10.3.

## Cross-cutting invariants
- [Font resolution](../invariant/font-resolution.md) — duplicated interpretation code.

## Blast radius
- [Text extraction](text-extraction.md) — first-priority mapping.
- [SVG renderer](svg-renderer.md) — `<text>` output.
- [Font subsetting](font-subsetting.md) — the compress tests use ToUnicode-based extraction as their judge.
- [Document builder](document-builder.md), [CJK font embedding](cjk-font-embedding.md) — the writing-side counterparts. This parser must be able to read back the CMaps they generate.

## Known holes / open
- `codespacerange` is not parsed → CMaps with variable-length codes cannot be told apart.
