# Reading order

## What it is
Splits extracted lines into columns, groups them into blocks (including de-hyphenation), and sorts them into reading order.

## Governing decisions
**None.**

## Design model
- Column boundary gap threshold: `(page_width * 0.15).max(30.0)`.
- Block boundary: twice the average font size.

## Code
- `justpdf-core/src/text/layout.rs` — `detect_columns_and_reorder`, `detect_column_boundaries`, `group_into_blocks_with_dehyphenation`, `build_block_dehyphenated`, `reading_order_sort`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [Text extraction](text-extraction.md) — the caller.
- [Text output formats](text-output-formats.md) — outputs the block structure as is.

## Known holes / open
**None.** No holes were found while writing this note.
