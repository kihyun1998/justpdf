# Content stream recursion

## The fact
Code that **executes** a content stream carries the references of the streams currently executing (the ancestor chain) into each of these: a Form XObject (`Do`), a tiling pattern's tile, a soft mask `/G` form. When it meets an ancestor again it silently skips that entry and goes on drawing the rest — rendering raises no error. Meeting a stream that is not an ancestor twice (a parent that draws one form twice) is not a cycle, and it is drawn twice. A direct object cannot be an ancestor — streams are indirect objects.

Two pieces of state are cut separately on entry.
- **The device color operators** (`g`/`rg`/`k`, `G`/`RG`/`K`) clear that side's pattern selection — because the color space changes to Device* (ISO 32000-2 §8.6.8). Without clearing, an `rg` after `scn` was ignored, and `rg … f` inside a tile recursed into the same tile.
- **A tile is drawn without the page's soft mask** — the mask is page-sized and the tile pixmap is cell-sized.

If the soft mask `/G` is an ancestor, **applying that `gs`'s `/SMask` is skipped altogether** (the current mask is left as it is). Building the mask pixmap and skipping only the content execution would install an empty mask (luminosity 0) that hides everything the mask form draws — `soft_mask_form_applying_its_own_gstate_renders` catches this.

Grounds (**derived**, 2026-09-29, #119): MuPDF `pdf-op-run.c` — `pdf_run_xobject` returns on a repeated entry using the `pdf_cycle` ancestor list, `begin_softmask` runs the mask form through the same `pdf_run_xobject`, and `pdf_show_pattern` clears the soft mask when entering a tile. MuPDF clears only the painting side's pattern when entering a colored pattern (`pdf_unset_pattern(what)`) — here the ancestor chain does the same job, so it is not cleared separately. **This equivalence holds only while the pattern a tile inherits is the object selected outside it**: the selection is the object resolved at `scn`/`SCN` (`PatternSelection`), held in the graphics state as MuPDF holds it, so it is not resolved again by name inside the tile — [resource name scope](resource-scope.md) (2026-09-30, #127). Where it differs from MuPDF: MuPDF defers the soft mask until painting, so a cycling `/G` ends as an empty mask. Here it runs right at `gs`, so the application itself is skipped. MuPDF stops a tile that selects its own pattern again with the gstate nesting limit (4096), not with the ancestor list.

Two things are **maintainer decisions** (2026-09-29, #119): not clearing the pattern name on tile entry (a departure from the approved scope — shown: MuPDF `pdf_unset_pattern(what)`, the result that a mutation turning that clearing off reddens no test, and the resource condition above), and skipping the whole `/SMask` application on a cycling `/G` (shown: the difference from MuPDF's empty-mask result, and that switching to deferred execution is out of scope). These two are for the maintainer to overturn.

## Why it is cross-cutting
There are two content stream executors and neither calls the other: `RenderInterpreter` (raster) and `SvgRenderer` (SVG, a copy of the interpreter — recurses only into Form XObjects). When there was no defence, both guarded only depth (>10), and the interpreter's pattern and soft mask paths did not count depth either. Once text extraction and font subsetting go into Form XObjects, they become a third and fourth executor (#48, #68).

## Territories it holds in
- [Render interpreter](../territory/render-interpreter.md) — `with_running_stream` manages the ancestor chain (`running_streams`). `do_xobject` wraps Form XObjects in it.
- [Render tiling patterns](../territory/render-tiling-patterns.md) — `render_pattern` wraps the tile, and `render_tiling_pattern` clears the soft mask. The device color operators clear the pattern selection.
- [Render transparency](../territory/render-transparency.md) — `apply_soft_mask` wraps the whole of `render_soft_mask`.
- [SVG renderer](../territory/svg-renderer.md) — `do_xobject` follows the same rule with `running_forms`.

## What a violation looks like
(Before #119, reproduced 2026-09-29 with a probe and `tests/render_recursion.rs`)
- A normal file with no cycle: page `/Pattern cs /P0 scn … f`, tile `1 0 0 rg 0 0 5 5 re f` → the process dies of a stack overflow.
- `/G` content re-applies the `gs` that installed it → stack overflow.
- A form that calls itself → drawn 11 times, up to the depth limit. Calling itself k times runs k^11 times (16.5 s at k=4).
- Using a global visited set instead of the ancestor chain makes the second copy disappear from a parent that draws one form twice — `form_drawn_twice_by_its_parent_is_not_a_cycle` catches this. Looking only at the direct parent misses a cycle of two forms calling each other — `form_cycle_through_another_form_stops_at_the_first_repeat` catches this.

## Discovery history
Found 2026-09-29 during the completeness review of the #52 work, and reproduced with a probe (#119). Fixed the same day.

## Where it will recur
**New code that executes content stream operators and follows `Do`, a pattern selection (`scn`/`SCN`) or a `gs`'s `/SMask` into another stream is subject to this invariant.** Command to find it: `rg -n 'execute_ops\(|parse_content_stream\(' justpdf-*/src`, the hits inside a recursive call. A collector that gathers once and is done (e.g. the unused-resource search in `writer/compress.rs`) iterates with a global `checked` set, and since it collects rather than draws, a global set is right — it is not subject to this.

What this invariant does not cover: fan-out where different forms each call two per level (no cycle, 2^10 by depth 10) — the same shape as the shared-node revisits in [Tree traversal cycles](tree-traversal-cycles.md). Which `/Resources` a resource name resolves in — [resource name scope](resource-scope.md). The ancestor chain compares object references, so it is independent of that rule. It also does not cover **other state** at nested stream boundaries (reproduced in the 2026-09-29 review): the graphics state stack has no per-stream floor, so an unbalanced `q`/`Q` crosses the boundary and pops or leaves the parent's state (MuPDF stops this with `gbot`), a path left after filling (`B`, `b`) goes into the tile, and the soft mask `/G` and transparency group forms inherit the caller's `ca`/`CA`, blend and soft mask (MuPDF's `begin_softmask` starts with alpha 1, Normal, no mask). Tracked: #128. There is no depth limit on different patterns and soft masks chained deep without a cycle — stack overflow at 66 levels in debug and 177 in release (measured 2026-09-29). Tracked: #122
