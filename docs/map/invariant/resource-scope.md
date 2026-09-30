# Resource name scope

## The fact
A resource name in a content stream — the XObject of `Do`, the Pattern of `scn`/`SCN`, the Shading of `sh`, the ExtGState of `gs`, the Font of `Tf` — resolves in **the scope of the stream it appears in**. The scope is the `/Resources` of the streams being executed, searched from the innermost down to the page's; the first one that names it wins. Entering a stream pushes its `/Resources` and leaving pops them: Form XObjects (`Do`, transparency groups included), tiling pattern tiles, soft mask `/G` forms and annotation appearance forms. A stream without `/Resources` pushes nothing, so it resolves in its caller's scope.

A name resolves **when it is selected**. `Tf` and `scn`/`SCN` put the resolved object in the graphics state, not the name (`FontKey` for fonts, `PatternSelection` for patterns). Forms and tiles inherit the graphics state, so a name carried there would resolve again in the inheriting stream's scope, to a different object. A direct font dictionary is told apart by the nearest indirect object holding it plus its name (`FontKey::Direct`), so the same name in two scopes does not share one cache entry.

Basis (**derived**, 2026-09-30, #127): MuPDF `pdf-interpret.c` — `pdf_lookup_resource` walks the resource stack from the innermost to the end, and `pdf_process_contents` pushes only when there are `/Resources`. `pdf-op-run.c` — `pdf_run_xobject` runs a form with its `/Resources`, else its caller's scope (`proc->rstack->resources`); a tile runs with `pat->resources`; the gstate holds the font (`pdf_font_desc`) and the pattern object. ISO 32000-2 §7.8.3 says forms and patterns shall have their own `/Resources` (inferred — not checked against the text). Innermost first means a conforming file uses its own; walking outwards means a file that left a name out of its own `/Resources` still draws the page's, as before #127. The triage brief said "own only, the caller's only when absent"; this MuPDF basis replaced it — shown to the maintainer, who answered to proceed. It is a derivation, not a judgement.

One divergence from MuPDF (derived from `pdf-op-run.c`, 2026-09-30; not measured): the soft mask `/G` form runs at the `gs` (see [content stream recursion](content-stream-recursion.md)), so a name its own `/Resources` lack falls back to the whole stack as it stood at the `gs`. MuPDF saves only the innermost dictionary at the `gs` and, at paint time, pushes it (or `/G`'s own) over the stack as it stands then. The two differ only when the mask is inherited into another form or group and `/G` leaves a name out of its own `/Resources` — a non-conforming file.

Holding the pattern as an object is **the maintainer's judgement** (2026-09-30, #127 triage). Shown: clearing the painting side's pattern on tile entry (MuPDF `pdf_unset_pattern(what)`, reversing #119's call), and leaving it to the implementer. Chosen: hold the object. That keeps [content stream recursion](content-stream-recursion.md)'s "the pattern selection is not cleared on tile entry" true. Holding the font as an object is a derivation for the same reason; the maintainer did not decide it separately.

## Why it is cross-cutting
There are two content stream executors: `RenderInterpreter` (raster) and `SvgRenderer` (SVG). They do not call each other but share the scope stack `ResourceScopes` and the graphics state types. Text extraction and font subsetting read only the page's `/Font` and do not enter forms (#48, #68). Once they do, they become a third and fourth executor under this rule.

## Territories it holds in
- [Render interpreter](../territory/render-interpreter.md) — `ResourceScopes` (`select_font`, `select_pattern`), `with_stream_resources`. `render_page` sets up the page scope; `do_xobject` enters a form's.
- [SVG renderer](../territory/svg-renderer.md) — `do_xobject` enters a form's scope; `Tf` and `scn`/`SCN` select through the same `ResourceScopes`.
- [Tiling patterns](../territory/render-tiling-patterns.md) — `render_pattern` draws the tile in the pattern's scope.
- [Transparency](../territory/render-transparency.md) — `apply_soft_mask` draws the `/G` form in its scope.
- [Render annotations](../territory/render-annotations.md) — `render_annotations` draws the appearance form in its scope.

## What a violation looks like
Measured before #127 (2026-09-30 probe, reproduced by `tests/render_resources.rs`):
- `Do` of a form only the outer form's `/Resources` name → not drawn (white).
- The same name is a blue form on the page and a red one in the form's own → drawn blue.
- A tile `Do`es a form only the pattern's `/Resources` name → the tile is empty.
- `ca 0` from the form's own `/ExtGState` → ignored; the fill shows.

Measured by mutation (2026-09-30):
- Names carried in the graphics state: the page selects its wide `/F1` and a form (whose own `/F1` is narrow) shows text without `Tf` → narrow. The page selects pattern `/P0` and a tile (whose own `/P0` is another pattern) fills without a colour → the other pattern.
- Own scope only, without walking outwards: a form that left a name out of its own `/Resources` loses what it drew before #127 — `name_missing_from_form_resources_resolves_in_the_page` catches it.

## Discovery history
Found by reading code during #119 on 2026-09-29 (#127). Reproduced with a probe and fixed on 2026-09-30.

## Where it will recur
**New code that resolves a resource name an operator in a content stream uses is under this invariant.** To find it: `rg -n 'resources_ref' justpdf-*/src` — places that read the page's resources directly. In the renderer only `ResourceScopes::for_page` should remain. The other site today is text extraction's `resolve_fonts` (`justpdf-core/src/text/mod.rs`) — not yet a violation, since it does not enter forms. Also watch for a new graphics state field that holds a name: `rg -n 'pub \w+: .*Vec<u8>' justpdf-render/src/graphics_state.rs` (no match today).

What this invariant does not cover: the `/Properties` lookup for `BDC /OC /name` (no lookup exists — TODO), colour space names (`cs`/`CS` — resource colour spaces are not resolved), Type3 glyph `/Resources` (Type3 unsupported), ExtGState `/Font` (ignored), inline image colour spaces (`BI` is not rendered). Once implemented, they resolve through the same scope stack.
