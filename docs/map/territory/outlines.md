# Outlines (bookmarks)

## What it is
Reads the `/Outlines` tree (`read_outlines`) and the named destinations (`read_named_destinations`), and sets and removes outlines (`set_outlines`, `remove_outlines`). Owns `Destination`, which [Actions](actions.md) re-exports.

## Governing decisions
**None.** [ADR 0005](../../adr/0005-sanitizing-removes-unpainted-text-by-category.md)'s navigation category removes `/Outlines` and named destinations (after rewriting every reference to a name into its explicit destination array); it decides [Sanitize](sanitize.md), not this module.

## Design model
From reading the source; no record governs it.
- **Reading**: `/Outlines` is followed only when the catalog holds it as a reference, and its `/First` likewise (inferred).
- **Cycles and budget**: an item that leads back to its own ancestor (through `/First` or `/Next`) is `CircularReference`. A `/Next` loop within one level stops at the first repeated item. When different items share the same `/First`, that child is read twice, and exceeding the visit budget is `LimitExceeded`. The named destination tree skips cyclic kids and kids past the budget — [Tree traversal cycles](../invariant/tree-traversal-cycles.md).
- **Destination**: `/Dest` first; else `/A /D` from a direct `/A`, whatever the action's `/S` (`read_outline_siblings`). An outline item whose action is a GoToR therefore reports the remote file's destination as one in this document (inferred, not run).
- **Open state**: `is_open` is `/Count > 0`, so an item without `/Count` reads as closed (inferred).
- **`Destination::from_array`** takes an integer page (the form for another file's page) as an `IndirectRef` with that object number, and `to_pdf_array` writes it back as a reference (measured 2026-10-30 at 88b2a08, scratch probe: `[3 /Fit]` comes back as `[3 0 R /Fit]`). An unknown fit type reads as `/Fit`.
- **Named destinations**: the `/Names /Dests` name tree (catalog `/Names` direct or indirect, `/Dests` direct or indirect) and the legacy catalog `/Dests` dictionary; values are arrays or dictionaries with `/D`. Keys are decoded lossily.
- **Writing** (`set_outlines`): a new `/Outlines` root and one indirect object per item with `/Title`, `/Parent`, `/Prev`, `/Next`, `/Dest` (always an explicit array or a string), `/C`, `/F`, `/First`, `/Last` and `/Count` (negative when closed); the catalog's `/Outlines` points at the new root and the old tree is left unlinked. `remove_outlines` only removes the catalog entry. An empty item list changes nothing.
- **Text**: `/Title` is decoded lossily (UTF-16BE not handled) and written as UTF-8 bytes — [Text string encoding](../invariant/text-string-encoding.md).
- **Bounds on input**: items and name-tree kids are counted by the visit budget; depth is not bounded (below). A name-tree value that is a reference is resolved and cloned per entry without being charged, so entries sharing one large value cost entries × size (measured on #135: 84 ms at K = M = 4,000).

## Code
- `justpdf-core/src/outline/parse.rs` — `read_outlines`, `read_outline_siblings`, `read_named_destinations`, `parse_name_tree`
- `justpdf-core/src/outline/builder.rs` — `set_outlines`, `build_outline_children`, `remove_outlines`
- `justpdf-core/src/outline/types.rs` — `Destination`, `Destination::from_object`, `Destination::from_array`, `Destination::to_pdf_array`, `OutlineItem`, `OutlineStyle`

## Reference behaviour
**None.** Clause to compare against: ISO 32000-2 §12.3.3.

## Cross-cutting invariants
- [Text string encoding](../invariant/text-string-encoding.md)
- [Tree traversal cycles](../invariant/tree-traversal-cycles.md)

## Blast radius
- [Actions](actions.md) — re-exports `Destination` and writes it with `to_pdf_array`; a change to the destination model changes every action built. Outlines also read `/A /D` without the action parser, so a fix to action meaning lands in two places.
- [Document modifier](document-modifier.md) — `delete_page`, `insert_page`, `reorder_pages` and `merge_documents` do not touch `/Outlines`: destinations keep pointing at deleted pages, and `merge_documents` writes a new catalog holding only `/Pages`, so the merged file has no outlines (inferred, code read at 88b2a08).
- [Sanitize](sanitize.md) — ADR 0005 removes `/Outlines` and rewrites named-destination references to explicit arrays; `remove_outlines` and `read_named_destinations` are its building blocks, and a destination form not modelled here would be missed there (not built: #310).
- [Facade](facade.md) — `Document::outlines` returns `OutlineItem`, so a field change is a public API change.

## Known holes / open
- A bookmark title outside ASCII is written as BOM-less UTF-8 and comes out garbled in other viewers (inferred, from the invariant above). Tracked: #33
- There is no depth limit: extreme depth without a cycle can overflow the stack (inferred). Tracked: #122
- Name-tree values are resolved outside the visit budget. Tracked: #135
- An outline item's `/A` is read as a GoTo whatever its type (inferred).
- An integer destination page is written back as an indirect reference (measured above).
- Page edits and merging leave outlines stale or drop them (inferred).
- An integer page in a destination (another file's page) is read as an object reference and written back as `n 0 R` (measured, Design model). Tracked: #423
