# Page tree

## What it is
Walks Catalog → `/Pages` → `/Kids` to build the `PageInfo` list, passing down inherited attributes (MediaBox, CropBox, Rotate, Resources). `get_page` skips subtrees by `/Count`. It is the common entry for per-page features (render, text, annotations, editing).

## Governing decisions
**None.**

## Design model
Rules read from the code.
- Only MediaBox, CropBox, Rotate and Resources are inherited. Bleed/Trim/Art are not.
- With no MediaBox, 612×792.
- A node is taken as a page if it is `/Type /Page` or has a MediaBox (inherited included).
- A box given as an indirect array is ignored (`get_array` does not follow references — [Object model](object-model.md)).
- `page_count` trusts the root `/Count`. Negative or missing is 0.
- `get_page` skips a subtree only by a non-negative integer `/Count`. A negative or missing `/Count` skips nothing — such a subtree finds the same pages as `collect_pages`.
- The two walkers (`walk_page_tree`, `walk_page_tree_find`) are near duplicates.
- `get_page` fails with `PageOutOfRange { index, count }` when the index is at or past the root `/Count` (`count` is that `/Count`), or, when the root has no usable `/Count`, when the walk ends without reaching the index (`count` is what the walk counted, a pruned subtree counting as its `/Count`). An index below the root `/Count` that the walk does not reach is `InvalidObject` — the `/Count` overstates the tree. The bindings use the variant to tell an index out of range from a damaged file (#134; measured by `integration.rs` `test_get_page_*`).
- An overstating `/Count` is file damage, not an index out of range — the **maintainer's judgement** (2026-09-30, #134). Shown: MuPDF `pdf-page.c` throws `FZ_ERROR_ARGUMENT` "invalid page number" only in `pdf_load_page` when the number is at or past `pdf_count_pages` (the root `/Count`; a missing one counts as 0), and `FZ_ERROR_FORMAT` "cannot find page %d in page tree" when `pdf_lookup_page_loc` walks past the tree. Options were that split, reporting both as `PageOutOfRange` (the brief), or deferring to #133. Chosen: the split, accepting that iterating a damaged document in Python raises `RuntimeError` at the first missing page instead of stopping, and that it answers, for `get_page` only, one case of the question #133 asks (which of `/Count` and the walk to believe) — not the two cases #133 lists. Those were decided in #133 (2026-10-07, maintainer): without a usable root `/Count`, `page_count` walks the tree under the visit budget and counts pages, as pdf.js does (`checkLastPage` falls back to `getAllPageDicts` when `/Count` is not an integer or the last page fails to load); and a shared-node tree under an honest `/Count`, where `get_page` succeeds and `collect_pages` is `LimitExceeded`, is the intended contract — `page_count` and `get_page` follow `/Count` (pdf.js, MuPDF), `collect_pages` expands every page and may refuse.
- A `/Pages` node that has its own ancestor as a kid is `CircularReference`. The same node listed twice is walked twice, but exceeding the visit budget is `LimitExceeded` — [Tree traversal cycles](../invariant/tree-traversal-cycles.md). The budget does not look at `/Count`, so a `get_page` whose pruning a false `/Count` switched off also stops.

## Code
- `justpdf-core/src/page/mod.rs` — `Rect`, `PageInfo`, `collect_pages`, `page_count`, `get_page`, `walk_page_tree`, `walk_page_tree_find`, `InheritedAttrs`, `subtree_count`
- `justpdf-core/src/tree_walk.rs` — `VisitBudget` (shared by the eight tree walkers — [Tree traversal cycles](../invariant/tree-traversal-cycles.md))

## Reference behaviour
**None.** Clause to compare against: ISO 32000-2 §7.7.3 (page tree, inherited attributes).

## Cross-cutting invariants
- [Tree traversal cycles](../invariant/tree-traversal-cycles.md)

## Blast radius
- [Render API](render-api.md), [Render interpreter](render-interpreter.md) — use `PageInfo`/`Rect` as they are.
- [Text extraction](text-extraction.md), [Annotations](annotations.md), [AcroForm](acroform.md), [Page labels](page-labels.md), [optional content](optional-content.md) — rely on page lookup.
- [Document modifier](document-modifier.md) — page insertion, deletion and reordering rewrite this structure.
- [Facade](facade.md), [CLI](cli.md), [Language bindings](language-bindings.md) — call `collect_pages`/`get_page`/`page_count` directly.

## Known holes / open
- There is no depth limit — extreme depth without a cycle can overflow the stack (inferred).
- `get_page` and `collect_pages`/`page_count` can answer differently for the same file — "What this invariant does not cover" in [Tree traversal cycles](../invariant/tree-traversal-cycles.md).
- Tracked: #122 (depth limit: 256, overflow measured at depth 500 debug / 900 release), #133 (page API disagreement)
