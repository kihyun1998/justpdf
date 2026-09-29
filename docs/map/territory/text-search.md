# Text search

## What it is
Runs exact, case-insensitive and simple regular-expression searches over page text, and returns match positions as rectangles (quads).

## Governing decisions
**None.**

## Design model
- Uses its own `SimpleRegex` instead of a regex crate. An unsupported pattern returns an empty result (not an error).

## Code
- `justpdf-core/src/text/search.rs` — `search_page`, `search_exact`, `search_case_insensitive`, `search_regex`, `SimpleRegex`, `compute_quad`, `TextQuad`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [Text extraction](text-extraction.md) — the input (glyph positions).
- [Facade](facade.md) — the only consumer of `search` and `search_case_insensitive`.

## Known holes / open
- An unsupported regular expression silently becomes an empty result.
