# Form flatten

## What it is
Draws the field widgets' appearances into the page content and removes `/AcroForm`, turning the form into static content.

## Governing decisions
**None.**

## Design model
- **Objects are read from the modifier**, falling back to the source document, so a value filled with `set_field_value` in the same modifier is the appearance flattened (maintainer decision, #40, 2026-10-23). Until #40 they were read from `doc` only, and the old appearance was drawn.
- **Each widget is drawn by ISO 32000-2 §12.5.5** as `q A cm /FlatN Do Q`:
  - A is `annot::placement::appearance_matrix`: the `/BBox` through `/Matrix`, mapped onto the normalised `/Rect`. `Do` applies the form's `/Matrix` itself.
  - The appearance is `annot::placement::normal_appearance`: `/AP /N`, or its `/AS` entry when it is a state dictionary.
  - The renderer uses the same two functions (#206), so a flattened page and a rendered unflattened one place an appearance identically.
  - Until #40 it wrote `w 0 0 h llx lly cm`, scaling a w×h BBox a second time (measured: a 200×20 BBox on a 200×20 Rect got `200 0 0 20 100 700 cm`). It ignored `/Matrix` and skipped state dictionaries.
- **Names resolve:**
  - each drawn appearance is added to the page's `/Resources /XObject` under the first `FlatN` its existing XObjects do not use
  - the effective resources (its own, else the inherited ones, an indirect object copied rather than modified) are written as a direct page `/Resources`
  - until #40 `/Fm{n}` was written and never registered (measured)
- **Isolation:** the existing content is wrapped in `q … Q` (a stream holding `q`, then the existing streams, then `Q` and the drawn content), so a leftover `cm` cannot move the appearances (measured: `an_unbalanced_cm_in_the_page_content_does_not_move_the_appearance`).
- **Only an indirect appearance stream is drawn**: it is named in `/XObject` by its reference. A direct one is removed without being drawn, deliberately (maintainer decision, #366 wontfix, 2026-10-23). It cannot come from a file: the parser builds a `PdfObject::Stream` only at the top level of an indirect object (`object/mod.rs`, *inferred*), and justpdf's own appearance writers (`annot/builder.rs`, `form/fill.rs`) insert references. Only a caller that puts a direct stream into a widget through `set_object` reaches it, and the serializer would write that object as an invalid file anyway (*inferred*).
- **Every `/Widget` is removed** from `/Annots` on every page, drawn or not. Hidden and NoView widgets are removed without being drawn. Other annotations stay. Until #40 a page whose widgets had no usable appearance kept them (measured).

## Code
- `justpdf-core/src/form/flatten.rs` — `flatten_form`
- `justpdf-core/src/annot/placement.rs` — `appearance_matrix`, `normal_appearance`, `form_matrix`, `is_hidden` (shared with the renderer)
- `justpdf-render/tests/render_flatten.rs` — flattened pages rendered and sampled

## Reference behaviour
ISO 32000-2 §12.5.5 (clause not re-read for #40; the #40 and #206 briefs' statement of it was followed).

## Cross-cutting invariants
**None.**

## Blast radius
- [Form appearance](form-appearance.md) — the coordinate rule of the appearances it draws in.
- [Render interpreter](render-interpreter.md), [Render annotations](render-annotations.md) — draw the flattened result; share `annot::placement`.
- [AcroForm](acroform.md), [Document modifier](document-modifier.md).

## Known holes / open
- The Print flag, NoZoom and NoRotate are not looked at.
