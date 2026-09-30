# Outlines (bookmarks)

## What it is
Reads the `/Outlines` tree and named destinations, and sets and removes outlines.

## Governing decisions
**None.**

## Design model
- `/Title` is decoded lossily (UTF-16BE not handled) and written as UTF-8 bytes — [Text string encoding](../invariant/text-string-encoding.md).
- An item that leads back to its own ancestor (through `/First` or `/Next`) is `CircularReference`. A `/Next` loop within one level stops quietly. When different items share the same `/First`, that child is read twice, and exceeding the visit budget is `LimitExceeded`. The named destination tree skips cyclic kids and kids past the budget — [Tree traversal cycles](../invariant/tree-traversal-cycles.md).
- `/A` reads only `/D`.

## Code
- `justpdf-core/src/outline/parse.rs` — `read_outlines`, `read_outline_siblings`, `read_named_destinations`
- `justpdf-core/src/outline/builder.rs` — `set_outlines`, `remove_outlines`
- `justpdf-core/src/outline/types.rs` — `Destination`, `OutlineItem`, `OutlineStyle`

## Reference behaviour
**None.** Clause to compare against: ISO 32000-2 §12.3.3.

## Cross-cutting invariants
- [Text string encoding](../invariant/text-string-encoding.md)
- [Tree traversal cycles](../invariant/tree-traversal-cycles.md)

## Blast radius
- [Actions](actions.md) — shares `Destination`, parses `/A` a second time.
- [Document modifier](document-modifier.md) — the setting path.
- [Merge](document-modifier.md) — no record of checking whether merging carries the outlines over.
- [Facade](facade.md) — `outlines`.

## Known holes / open
- A bookmark with a Korean title breaks in other viewers (inferred, from the invariant above).
- There is no depth limit — extreme depth without a cycle can overflow the stack (inferred).
- Tracked: #33 (text string encoding), #122 (depth limit)
