# Render annotations

## What it is
After drawing the page content, draws each annotation's `/AP /N` appearance stream onto its `/Rect`.

## Governing decisions
**None.**

## Design model
Read from the source and the issues cited; no decision record states it.

- `render_page` always draws annotations after the content, and ignores their errors (`let _ = self.render_annotations(page)`; re-verified). `/Annots` may be a direct or an indirect array; an entry that is not a dictionary, or has no readable `/Rect`, is skipped.
- Respects the Hidden (0x02) and NoView (0x20) flags (core `annot::placement::is_hidden`; re-verified). No other flag is read.
- **Which appearance** (`normal_appearance`, #206): `/AP /N` when it is a stream. When it is a state dictionary, the entry the annotation's `/AS` names. With no `/AS`, or an `/AS` naming no entry, nothing is drawn. Until #206 a state dictionary was skipped.
- **The stream is decoded** (`doc.decode_stream`) before it runs, as every other Form XObject path does. Until #206 its raw bytes were executed, so a FlateDecode appearance (every one `justpdf-core` writes) drew nothing (measured: 0 pixels, `a_flate_compressed_appearance_is_drawn` on master).
- **Mapping onto `/Rect`** (ISO 32000-2 §12.5.5, `appearance_mapping`, #206):
  - The form's `/BBox` corners are transformed by its `/Matrix` (identity when absent), and their bounding box is taken.
  - Matrix A scales and translates that box onto the (normalised) `/Rect`.
  - The CTM becomes `A × CTM` and the existing `render_form_xobject` concatenates `/Matrix` on top, giving `Matrix × A × CTM` for both the plain and the transparency-group path.
  - A box with no width or height draws nothing; the guard keeps the matrix finite, and its absence is not visible in pixels, because tiny-skia drops a non-finite transform (measured by mutation).
  - Until #206 only the form's own `/Matrix` applied (measured: a highlight at `[100 500 300 520]` with Matrix `[1 0 0 1 -100 -500]` was drawn at page `(0..200, 0..20)`).
- **Clipped to `/BBox`** in form space (`clip_to_bbox`): the BBox rectangle through `/Matrix × A × CTM`, intersected with any clip already set. Popping the graphics state after each annotation restores the clip (#50). Until #50 the device clip mask was saved and restored by hand around each annotation.
- Page-content `Do` does not map onto a `/Rect`, but since #365 it clips to `/BBox` through the same `clip_to_bbox` ([Render clipping](render-clipping.md)). An appearance is not reached through `do_xobject`, so it is clipped once.
- The appearance form is drawn in its own `/Resources` scope, else the page's — [resource name scope](../invariant/resource-scope.md). It is not entered through `with_running_stream` and does not count toward `xobject_depth`; a form it draws with `Do` does (inferred, `render_annotations`).
- An annotation whose `/OC` is hidden under the default configuration is not drawn (`ocg::is_oc_visible`, #44) — [optional content](optional-content.md).

## Code
- `justpdf-render/src/interpreter.rs` — `render_annotations`, `normal_appearance`, `Appearance`, `oc_entry_visible`, `clip_to_bbox`, `appearance_mapping`, `form_matrix`, `matrix_of`, `render_form_xobject`
- `justpdf-core/src/annot/placement.rs` — `normal_appearance`, `appearance_matrix`, `form_matrix`, `is_hidden`: the selection, the §12.5.5 matrix and the Hidden/NoView test, shared with form flatten (#40; re-verified, form flatten imports them)
- `justpdf-render/tests/render_annotations.rs` — the §12.5.5 cases (local, absolute, unit and matrix BBoxes, clip, states, transparency group, flags, degenerate box)

## Reference behaviour
ISO 32000-2 §12.5.5 (the algorithm mapping the appearance stream onto the annotation rectangle; clause not re-read for #206, the brief's statement of it was followed).

## Cross-cutting invariants
- [resource name scope](../invariant/resource-scope.md)

## Blast radius
- [Annotation appearance](annotation-appearance.md) — builds the XObjects drawn here; its `/BBox`, `/Matrix` and `/Rect` must agree with the §12.5.5 mapping, so fix the coordinate rules on both sides together (#41).
- [Form flatten](form-flatten.md) — uses the same `annot::placement` functions; a change to the selection, the matrix or the hidden test changes flattened output too.
- [Annotations](annotations.md) — the `/Annots`, `/AP`, `/AS`, `/F` and `/Rect` entries read here.
- [optional content](optional-content.md) — an annotation's `/OC` is tested with the same default configuration as page content.
- [Render transparency](render-transparency.md), [Render clipping](render-clipping.md) — an appearance goes through `render_form_xobject` (group or direct) and `clip_to_bbox`, the Form XObject paths `Do` uses.

## Known holes / open
- NoZoom / NoRotate, `/AP /D` and `/R`, the Print flag: not handled (re-verified: `is_hidden` reads bits 0x02 and 0x20 only).
- Tracked: #41 (the core generator's coordinates)
- Appearances justpdf generates with absolute page coordinates (highlight, line, ink, circle — [Annotation appearance](annotation-appearance.md)) are clipped away by `clip_to_bbox`: a generated highlight renders 0 of 4,000 expected pixels (measured 2026-10-30 by the map rewrite). Tracked: #41
