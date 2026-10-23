# Render annotations

## What it is
After drawing the page content, draws each annotation's `/AP /N` appearance stream.

## Governing decisions
**None.**

## Design model
- Respects the Hidden (0x02) and NoView (0x20) flags.
- **Which appearance** (`normal_appearance`, #206): `/AP /N` when it is a stream. When it is a state dictionary, the entry the annotation's `/AS` names. With no `/AS`, or an `/AS` naming no entry, nothing is drawn. Until #206 a state dictionary was skipped.
- **The stream is decoded** (`doc.decode_stream`) before it runs, as every other Form XObject path does. Until #206 its raw bytes were executed, so a FlateDecode appearance (every one `justpdf-core` writes) drew nothing (measured: 0 pixels, `a_flate_compressed_appearance_is_drawn` on master).
- **Mapping onto `/Rect`** (ISO 32000-2 §12.5.5, `appearance_mapping`, #206):
  - The form's `/BBox` corners are transformed by its `/Matrix` (identity when absent), and their bounding box is taken.
  - Matrix A scales and translates that box onto the (normalised) `/Rect`.
  - The CTM becomes `A × CTM` and the existing `render_form_xobject` concatenates `/Matrix` on top, giving `Matrix × A × CTM` for both the plain and the transparency-group path.
  - A box with no width or height draws nothing; the guard keeps the matrix finite, and its absence is not visible in pixels, because tiny-skia drops a non-finite transform (measured by mutation).
  - Until #206 only the form's own `/Matrix` applied (measured: a highlight at `[100 500 300 520]` with Matrix `[1 0 0 1 -100 -500]` was drawn at page `(0..200, 0..20)`).
- **Clipped to `/BBox`** in form space (`clip_to_bbox`): the BBox rectangle through `/Matrix × A × CTM`, intersected with any clip already set. The device clip mask is saved before and restored after each annotation, since popping the graphics state does not restore it (#50). Within the annotation pass a mutation that drops the restore stays green: popping clears `has_clip`, so the next annotation sets a fresh clip, and annotations are drawn last (measured by mutation).
- Page-content `Do` does not map onto a `/Rect`, but since #365 it clips to `/BBox` through the same `clip_to_bbox` ([Render clipping](render-clipping.md)). An appearance is not reached through `do_xobject`, so it is clipped once.
- The appearance form is drawn in its own `/Resources` scope, else the page's — [resource name scope](../invariant/resource-scope.md).
- The annotation's `/OC` is not checked.

## Code
- `justpdf-render/src/interpreter.rs` — `render_annotations`, `normal_appearance`, `clip_to_bbox`, `appearance_mapping`, `form_matrix`, `render_form_xobject`
- `justpdf-core/src/annot/placement.rs` — the selection, the §12.5.5 matrix and the Hidden/NoView test, shared with form flatten (#40)
- `justpdf-render/tests/render_annotations.rs` — the §12.5.5 cases (local, absolute, unit and matrix BBoxes, clip, states, transparency group, flags, degenerate box)

## Reference behaviour
ISO 32000-2 §12.5.5 (the algorithm mapping the appearance stream onto the annotation rectangle; clause not re-read for #206, the brief's statement of it was followed).

## Cross-cutting invariants
- [resource name scope](../invariant/resource-scope.md)

## Blast radius
- [Annotation appearance](annotation-appearance.md) — the side that builds the XObject drawn here. Fix the coordinate rules on both sides together.
- [Annotations](annotations.md) — input.
- [optional content](optional-content.md) — annotation `/OC`.
- [Render transparency](render-transparency.md) — shares the Form XObject path.

## Known holes / open
- NoZoom / NoRotate, `/AP /D` and `/R`, the Print flag: not handled.
- Tracked: #44 (annotation `/OC`), #41 (the core generator's coordinates)
