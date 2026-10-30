# Reading order

## What it is
Takes the lines [Text extraction](text-extraction.md) built for a page, splits them into columns, groups each column's lines into blocks, joins hyphenated line ends, and returns the blocks in reading order (`detect_columns_and_reorder`). The blocks are `PageText::blocks`, which plain text and every [Text output format](text-output-formats.md) print.

## Governing decisions
**None.**

## Design model
From reading the source (`text/layout.rs`) and its unit tests; no record or comparison sets these numbers.
- **Columns** (`detect_column_boundaries`): the line start positions (`TextLine::x`) are sorted and merged within 5 pt. A gap between consecutive starts wider than `(spread × 0.15).max(30.0)` starts a new column, where `spread` is the distance from the leftmost to the rightmost line start — not the page width, though the code names it `page_width`. A column's right edge is the furthest word end of the lines starting in it. Several columns are kept only if each has at least two lines starting in it; otherwise the page is one column.
- **Assigning lines**: each line goes to the column whose centre is nearest its start x (`find_column_for_line`); columns are read left to right, lines in each top to bottom.
- **Blocks** (`group_into_blocks_with_dehyphenation`): a new block starts where the baseline gap to the previous line exceeds twice the font size of that line's first word (the code calls it the average; it is one word's size).
- **Hyphens** (`build_block_dehyphenated`): a line ending in `-` loses its trailing hyphens and is appended to the *previous* output line with a space; the next line still starts a new output line. So the two halves of the word are not joined, and the hyphen is gone (measured 2026-10-30, probe: lines `The exam-`, `ple works` give `"The exam\nple works"`; with `First line` before them, `"First line The exam\nple works"`). A hyphen that belongs to the word (`well-` / `known`) is dropped the same way.
- `reading_order_sort` (public) sorts words top to bottom and left to right with a half-font-size tolerance; nothing outside the unit tests calls it. Its comparator is not transitive (the tolerance), which `sort_by` does not require of callers but may report by panicking on recent Rust versions (inferred, not run).
- **Bounds on input**: column detection is O(lines × columns); the line count is whatever extraction produced. No input is bounded here.

## Code
- `justpdf-core/src/text/layout.rs` — `detect_columns_and_reorder`, `ColumnBound`, `detect_column_boundaries`, `find_max_x_for_cluster`, `find_column_for_line`, `group_into_blocks_with_dehyphenation`, `build_block_dehyphenated`, `reading_order_sort`

## Reference behaviour
**None.** No comparison with MuPDF's structured-text block building (`stext-device.c`) or pdf.js is recorded.

## Cross-cutting invariants
**None.**

## Blast radius
- [Text extraction](text-extraction.md) — builds the lines this reads; its line grouping across the whole page width and its text-space `TextChar::width` (#318, through word widths) decide what columns can be found here.
- [Text output formats](text-output-formats.md) — print `PageText::blocks` as they come; a change in block boundaries or joined text changes every format's output.
- [Facade](facade.md), [CLI](cli.md), [Language bindings](language-bindings.md) — plain text is `PageText::plain_text`, the blocks joined; a change here changes every text call's output.

## Known holes / open
- Columns that share baselines are not detected: [Text extraction](text-extraction.md) has already merged the words of both columns into one line, so every line starts in the left column (measured 2026-10-30, probe: three rows of `LeftN` at x 50 and `RightN` at x 350 on the same baselines extract as `LeftOne RightOne\nLeftTwo RightTwo\n…`). Not tracked by an issue.
- De-hyphenation drops the hyphen without joining the word, and moves the first half onto the previous line (measured, above). Not tracked by an issue.
- Open question for the maintainer: is de-hyphenation meant to join the halves (`example`), or to keep line breaks and only drop the hyphen? The unit test `test_dehyphenation` checks only that `para-\n` is gone.
