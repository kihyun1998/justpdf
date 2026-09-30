# Page labels

## What it is
Reads the `/PageLabels` number tree to compute each page index's display label (roman numerals, letters, prefix), and sets labels.

## Governing decisions
**None.**

## Design model
- In the number tree, kids that point at an ancestor and kids past the visit budget are skipped — [Tree traversal cycles](../invariant/tree-traversal-cycles.md). The prefix is decoded lossily — [Text string encoding](../invariant/text-string-encoding.md).
- Setting sorts `/Nums`.

## Code
- `justpdf-core/src/page_label.rs` — `read_page_labels`, `label_for_page`, `to_roman`, `to_alpha`, `set_page_labels`, `parse_number_tree`

## Reference behaviour
**None.** Clause to compare against: ISO 32000-2 §12.4.2.

## Cross-cutting invariants
- [Text string encoding](../invariant/text-string-encoding.md)
- [Tree traversal cycles](../invariant/tree-traversal-cycles.md)

## Blast radius
- [Page tree](page-tree.md) — the index basis.
- [Document modifier](document-modifier.md) — no record of checking whether labels drift after pages are inserted or deleted.
- [Facade](facade.md) — `page_labels`.

## Known holes / open
- There is no depth limit — extreme depth without a cycle can overflow the stack (inferred).
- Tracked: #33 (text string encoding), #122 (depth limit)
