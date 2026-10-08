# Font recovery

## What it is
For a missing or damaged font, guesses from its name whether it is bold, italic, monospaced or serif, and picks a standard 14 substitute.

## Governing decisions
**None.**

## Design model
- The default is Helvetica ("most commonly used fallback in PDF viewers"). CJK fonts also go to Helvetica.

## Code
- `justpdf-core/src/font/recovery.rs` — `find_substitute`, `fallback_font_info`, `is_bold_name`, `is_italic_name`, `is_monospace_name`, `is_serif_name`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [Glyph rendering](glyph-rendering.md) — the raster renderer draws a simple font with no embedded program from the bundled URW program for the name `find_substitute` returns (#227), so a change to the name rules changes which face such text is drawn in.
- [Text extraction](text-extraction.md) — could use it for width substitution.

## Known holes / open
- Text extraction does not call it; the SVG renderer does not either (it emits `font-family`).
