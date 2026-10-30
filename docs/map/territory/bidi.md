# BiDi

## What it is
Uses `unicode-bidi` to split a string into direction runs (`resolve_bidi`) and tells whether a string holds right-to-left characters (`contains_rtl`). Nothing in the repo calls either.

## Governing decisions
- [ADR-0001](../../adr/0001-crates-split-by-dependency-layer.md) — places it in `justpdf-special` (`bidi` feature, `unicode-bidi`).

## Design model
Status: read from the source only.
- `resolve_bidi` runs `BidiInfo::new(text, None)` and, per paragraph, takes `visual_runs`: the runs come in visual order, but each run's text is the logical-order substring, so an RTL run's characters are not reversed (inferred). The level is read at the run's first byte. A non-empty text with no runs falls back to one LTR run.
- `contains_rtl` checks a fixed list of block ranges (Hebrew, Arabic, Syriac, Thaana, NKo, Samaritan, Mandaic, Arabic Extended-A, the presentation forms, and four SMP blocks). It is not the Unicode `Bidi_Class` and differs from `resolve_bidi` at the edges (inferred).
- No writing path uses it: `PageBuilder::show_text` maps characters one to one, so RTL text is written in logical order (inferred; [Content text encoding](../invariant/content-text-encoding.md) names this as #177's scope).
- Input bound: the string is the caller's.

## Code
- `justpdf-special/src/bidi/mod.rs` — `Direction`, `BidiRun`, `resolve_bidi`, `contains_rtl`, `is_rtl_char`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
**None.** No code in the repo calls this module (`git grep -n "bidi::" -- '*.rs' ':!justpdf-special'` finds nothing, 2026-10-30), so a change here reaches no other territory.

## Known holes / open
- No consumer inside the repository.
- Shaping and bidi reordering for embedded-font text; #177 notes `unicode-bidi` exists only here and is not wired in. Tracked: #177
