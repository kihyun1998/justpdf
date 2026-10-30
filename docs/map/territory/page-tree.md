# Page tree

## What it is
Walks Catalog → `/Pages` → `/Kids` to build the `PageInfo` list, passing down the inherited attributes MediaBox, CropBox, Rotate and Resources. `get_page` finds one page and skips subtrees by `/Count`; `page_count` reads the root `/Count`. It also owns `Rect`, the rectangle type that page boxes and annotation and widget `/Rect`s are read into. It is the common entry for per-page features (render, text, annotations, editing).

## Governing decisions
**None.** The error split and the page-count rules were decided on issues (#134, #133), not in a record.

## Design model
Rules read from the code (*inferred* unless marked).
- **A rectangle read from an array is normalised** (`Rect::from_pdf_array` → `Rect::normalized`, #345): `llx = min(a, c)`, `lly = min(b, d)`, `urx = max(a, c)`, `ury = max(b, d)`, so page boxes and annotation and widget `/Rect`s read the same whichever two opposite corners the file gives (ISO 32000-2 §7.9.5, not re-read). MuPDF `pdf_to_rect` (`source/pdf/pdf-parse.c`) and pdf.js `Util.normalizeRect` / `lookupNormalRect` do the same (raw source, read 2026-10-23, #345 triage); it was a derivation, not a judgement. A `Rect` built in code is not normalised, so `width`/`height` keep their `.abs()`. Until #345 the corners were kept in file order, so an annotation appearance for `/Rect [100 120 0 100]` differed from the one for `[0 100 100 120]` (*measured* then: `a_reversed_rect_gives_the_same_appearance` in `annot/appearance.rs` failed on master).
- Only MediaBox, CropBox, Rotate and Resources are inherited (`InheritedAttrs::with_overrides`). Bleed, Trim and Art boxes are read from the page alone.
- With no MediaBox anywhere, 612 × 792.
- A node is a page if it is `/Type /Page`, has a MediaBox, or has an inherited MediaBox. So under a `/Pages` node with a MediaBox, any kid that is not `/Type /Pages` is taken as a page. A node that is none of these is skipped.
- `/Rotate` is stored as read, any integer; nothing reduces it to 0/90/180/270 here.
- A kid that is not an indirect reference is skipped. A box given as an indirect array is ignored, because `get_array` does not follow references ([Object model](object-model.md)).
- `page_count` trusts the root `/Count`; a negative or missing one is 0 (`subtree_count(..).unwrap_or(0)`).
- `get_page` checks the index against the root `/Count`, then skips a subtree only by a non-negative integer `/Count`. A negative or missing `/Count` skips nothing, so such a subtree finds the same pages as `collect_pages`.
- `get_page` fails with `PageOutOfRange { index, count }` when the index is at or past the root `/Count` (`count` is that `/Count`), or, when the root has no usable `/Count`, when the walk ends without reaching the index (`count` is what the walk counted, a pruned subtree counting as its `/Count`). An index below the root `/Count` that the walk does not reach is `InvalidObject`: the `/Count` overstates the tree. The bindings use the variant to tell an index out of range from a damaged file (#134; pinned by the `test_get_page_*` tests in `tests/integration.rs`).
  - An overstating `/Count` is file damage, not an index out of range — the maintainer's judgement (2026-09-30, #134). Shown: MuPDF `pdf-page.c` throws `FZ_ERROR_ARGUMENT` "invalid page number" only in `pdf_load_page` when the number is at or past `pdf_count_pages` (the root `/Count`; a missing one counts as 0), and `FZ_ERROR_FORMAT` "cannot find page %d in page tree" when `pdf_lookup_page_loc` walks past the tree. Options were that split, reporting both as `PageOutOfRange` (the brief), or deferring to #133. Chosen: the split, accepting that iterating a damaged document in Python raises `RuntimeError` at the first missing page instead of stopping.
- The two walkers `walk_page_tree` and `walk_page_tree_find` are near duplicates; only the second prunes by `/Count`.
- A `/Pages` node that lists one of its own ancestors as a kid is `CircularReference` (`enter_pages_node`). The same node listed twice is walked twice, but passing the visit budget is `LimitExceeded` — [Tree traversal cycles](../invariant/tree-traversal-cycles.md). The budget does not look at `/Count`, so a `get_page` whose pruning a false `/Count` switched off also stops.
- **Input bound:** the number of node visits is bounded by `VisitBudget` (`tree_walk.rs`), which charges each visit by the node's element count against a multiple of the distinct nodes read. Recursion depth is not bounded. The pages collected are bounded by the visits.

## Code
- `justpdf-core/src/page/mod.rs` — `Rect`, `Rect::from_pdf_array`, `Rect::normalized`, `PageInfo`, `collect_pages`, `page_count`, `get_page`, `subtree_count`, `walk_page_tree`, `walk_page_tree_find`, `enter_pages_node`, `InheritedAttrs`, `InheritedAttrs::with_overrides`, `a_rect_written_upper_right_first_is_normalised`
- `justpdf-core/src/tree_walk.rs` — `VisitBudget`, `VisitBudget::enter` (shared with the other tree walkers — [Tree traversal cycles](../invariant/tree-traversal-cycles.md))

## Reference behaviour
- Rectangle normalisation: MuPDF `pdf_to_rect` and pdf.js `Util.normalizeRect` / `lookupNormalRect` (#345, read 2026-10-23).
- Page lookup errors: MuPDF `pdf-page.c` `pdf_load_page` / `pdf_lookup_page_loc` (#134, 2026-09-30).
- Page count without a usable `/Count`: pdf.js `checkLastPage` falls back to `getAllPageDicts` (#133, 2026-10-07).
- Not compared: ISO 32000-2 §7.7.3 (page tree, inherited attributes).

## Cross-cutting invariants
- [Tree traversal cycles](../invariant/tree-traversal-cycles.md) — `walk_page_tree`, `walk_page_tree_find`.

## Blast radius
- [Content interpreter](content-interpreter.md) (`DocResources::page`), [Render interpreter](render-interpreter.md) (`ResourceScopes::for_page`) — both start the page's [resource name scope](../invariant/resource-scope.md) from `PageInfo::resources_ref`, so a change to how `/Resources` is inherited or stored changes what every page-level name resolves to (inferred, `rg -n 'resources_ref' justpdf-*/src`).
- [Render API](render-api.md), [Render interpreter](render-interpreter.md) — take page size, boxes and `rotate` from `PageInfo` as given. A change to normalisation, defaults or unreduced `/Rotate` changes every rendered page.
- [Annotations](annotations.md), [AcroForm](acroform.md) — read `/Rect` through `Rect::from_pdf_array` and rely on its normalisation.
- [Text extraction](text-extraction.md), [Page labels](page-labels.md), [Optional content](optional-content.md) — rely on page order and `get_page` indices; a change to what counts as a page shifts their indices.
- [Linearization](linearization.md) — the first-page section and hint table follow `collect_pages` order.
- [Document modifier](document-modifier.md) — page insertion, deletion and reordering rewrite this structure; it must keep the tree readable by these walkers.
- [Facade](facade.md), [CLI](cli.md), [Language bindings](language-bindings.md) — call `collect_pages`, `get_page` and `page_count` and map `PageOutOfRange` versus other errors; the variant split is their contract.

## Known holes / open
- There is no depth limit: a deep tree without a cycle overflows the stack. Measured in #122 (2026-10-07, Windows, 1 MB main-thread stack, `justpdf info` on a `/Kids` chain): debug overflows at depth 500 (400 ok), release at 900 (850 ok). Decided there (maintainer): a core depth limit of 256, past which `LimitExceeded`. Tracked: #122
- `get_page`, `collect_pages` and `page_count` can answer differently for one file — "What this invariant does not cover" in [Tree traversal cycles](../invariant/tree-traversal-cycles.md). Decided in #133 (2026-10-07, maintainer): without a usable root `/Count`, `page_count` walks the tree under the visit budget and counts pages, as pdf.js does; and a shared-node tree under an honest `/Count`, where `get_page` succeeds and `collect_pages` is `LimitExceeded`, is the intended contract (`page_count` and `get_page` follow `/Count`, as pdf.js and MuPDF do; `collect_pages` expands every page and may refuse). At HEAD `page_count` still returns 0 without a usable `/Count`. Tracked: #133
- An inherited `/Resources` is cloned into each page's `PageInfo::resources_ref` (`InheritedAttrs`, `dict.get(b"Resources").cloned()`). A reference clones cheaply; a direct dictionary on a `/Pages` node is copied whole into every page below it, N pages × R entries, with no shared node and outside the visit budget. Tracked: #351, #135
- Page-index lookups outside `get_page` report an index past the last page as other error kinds. Tracked: #150
