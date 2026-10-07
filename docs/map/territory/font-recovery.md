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
- [Glyph rendering](glyph-rendering.md) — the side that would use a substitute font (currently a placeholder rectangle). #227 makes it the renderer's source for picking a bundled URW base-14 substitute.
- [Text extraction](text-extraction.md) — could use it for width substitution.

## Known holes / open
- Neither text extraction nor rendering calls it.
