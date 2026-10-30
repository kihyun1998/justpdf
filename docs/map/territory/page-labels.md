# Page labels

## What it is
Reads the `/PageLabels` number tree into `PageLabelRange`s (`read_page_labels`), computes a page index's display label from them (`label_for_page`, with `to_roman` and `to_alpha`), and writes ranges back (`set_page_labels`).

## Governing decisions
**None.** [ADR 0005](../../adr/0005-sanitizing-removes-unpainted-text-by-category.md)'s navigation category removes the `/P` prefixes of `/PageLabels` and keeps `/S` and `/St`; it decides [Sanitize](sanitize.md), not this module.

## Design model
From reading the source; no record governs it.
- **Number tree** (`parse_number_tree`): kids that point at an ancestor and indirect kids past the visit budget are skipped; an indirect root is put among the ancestors first — [Tree traversal cycles](../invariant/tree-traversal-cycles.md). A direct dictionary kid is walked without the budget (it cannot form a cycle). Entries are sorted by key.
- **A range**: `/S` (an unknown name is no numbering), `/P` decoded lossily — [Text string encoding](../invariant/text-string-encoding.md) — and `/St`, default 1. A negative key is cast to `usize` and so becomes a range no page reaches (inferred).
- **`label_for_page`**: the last range starting at or before the index; no ranges, or none before the index, gives `index + 1`. The number is `/St + (index − start)`; roman and letters give the empty string for values below 1.
- **Writing** (`set_page_labels`): a new indirect number tree with one sorted `/Nums` array, `/S` when there is a style, `/P` as the Rust string's UTF-8 bytes when non-empty, `/St` when not 1; the catalog's `/PageLabels` points at it. A catalog missing from the modifier, or not a dictionary, is reported as `FormError`.
- **Bounds on input**: kids are counted by the visit budget; depth is not bounded (below). A `/Nums` value that is a reference is resolved and cloned per entry outside the budget (measured on #135: 855 ms at K = M = 4,000, a 78 KB file). **`/St` is not bounded**: `to_roman` writes one `M` per thousand, so its output grows linearly with the value (measured 2026-10-30 at 88b2a08, scratch probe: `to_roman(2_000_000)` is 2,000 characters; a file's `/St` of 10¹² would ask for 10⁹), and `/St + offset` can overflow `i64` (inferred).

## Code
- `justpdf-core/src/page_label.rs` — `PageLabelStyle`, `PageLabelRange`, `parse_number_tree`, `build_nums_array`, `read_page_labels`, `label_for_page`, `to_roman`, `to_alpha`, `set_page_labels`

## Reference behaviour
**None.** Clause to compare against: ISO 32000-2 §12.4.2.

## Cross-cutting invariants
- [Text string encoding](../invariant/text-string-encoding.md)
- [Tree traversal cycles](../invariant/tree-traversal-cycles.md)

## Blast radius
- [Page tree](page-tree.md) — keys are 0-based page indices in page-tree order; a change to how pages are counted or ordered shifts every label.
- [Document modifier](document-modifier.md) — `delete_page`, `insert_page` and `reorder_pages` do not adjust `/PageLabels`, so labels after an edit belong to the old page order, and `merge_documents` writes a catalog without `/PageLabels` (inferred, code read at 88b2a08).
- [Sanitize](sanitize.md) — ADR 0005 strips `/P` and keeps `/S` and `/St`; `read_page_labels` and `set_page_labels` are the path it would take (not built: #310).
- [Facade](facade.md) — `Document::page_labels` returns `PageLabelRange`, so a field change is a public API change.

## Known holes / open
- There is no depth limit: extreme depth without a cycle can overflow the stack (inferred). Tracked: #122
- `/Nums` values are resolved outside the visit budget. Tracked: #135
- A large `/St` makes `label_for_page` build a roman numeral as long as `/St / 1000`, and `/St + offset` can overflow (above; not tracked).
- Page edits do not move labels; merging drops them (inferred).
- Prefixes are written as BOM-less UTF-8 and read as lossy UTF-8. Tracked: #33
