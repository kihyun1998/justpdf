# Render annotations

## What it is
After drawing the page content, draws each annotation's `/AP /N` appearance stream.

## Governing decisions
**None.**

## Design model
- Respects the Hidden (0x02) and NoView (0x20) flags.
- If `/N` is a state dictionary, it is skipped. `/AS` is not looked at.
- Calls `render_form_xobject` **without the Rect mapping and without the BBox clip** (inferred: the appearance is drawn in the wrong place). The spec requires a transformation that maps the BBox, transformed by the Matrix, onto the Rect.
- The appearance form is drawn in its own `/Resources` scope, else the page's — [resource name scope](../invariant/resource-scope.md).
- The annotation's `/OC` is not checked.

## Code
- `justpdf-render/src/interpreter.rs` — `render_annotations`, `render_form_xobject`

## Reference behaviour
**None.** Clause to compare against: ISO 32000-2 §12.5.5 (the algorithm mapping the appearance stream onto the annotation rectangle).

## Cross-cutting invariants
- [resource name scope](../invariant/resource-scope.md)

## Blast radius
- [Annotation appearance](annotation-appearance.md) — the side that builds the XObject drawn here. Fix the coordinate rules on both sides together.
- [Annotations](annotations.md) — input.
- [optional content](optional-content.md) — annotation `/OC`.
- [Render transparency](render-transparency.md) — shares the Form XObject path.

## Known holes / open
- There is no annotation render test.
- Tracked: #41 (annotation appearance coordinates)
