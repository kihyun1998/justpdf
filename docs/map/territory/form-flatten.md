# Form flatten

## What it is
Draws each widget's normal appearance into its page's content, removes every widget annotation and removes `/AcroForm` from the catalog (`flatten_form`), turning the form into static content.

## Governing decisions
**None.** The rules below are maintainer decisions recorded on issues (#40, #366), not in a decision record.

## Design model
- **Entry**: nothing happens unless `parse_acroform(doc)` finds an AcroForm in the **source** document; pages come from `collect_pages(doc)`.
- **Objects are read from the modifier**, falling back to the source document (`object_at`), so a value filled with `set_field_value` in the same modifier is the appearance flattened (maintainer decision, #40, 2026-10-23). Until #40 they were read from `doc` only, and the old appearance was drawn.
- **Each widget is drawn by ISO 32000-2 §12.5.5** as `q A cm /FlatN Do Q`:
  - A is `annot::placement::appearance_matrix`: the `/BBox` through `/Matrix`, mapped onto the normalised `/Rect`. `Do` applies the form's `/Matrix` itself.
  - The appearance is `annot::placement::normal_appearance`: `/AP /N`, or its `/AS` entry when it is a state dictionary.
  - The renderer uses the same functions (`justpdf-render/src/interpreter.rs` calls `placement::normal_appearance`, `appearance_matrix`, `form_matrix`, `is_hidden`; #206), so a flattened page and a rendered unflattened one place an appearance identically.
  - Until #40 it wrote `w 0 0 h llx lly cm`, scaling a w×h BBox a second time (measured: a 200×20 BBox on a 200×20 Rect got `200 0 0 20 100 700 cm`, #40). It ignored `/Matrix` and skipped state dictionaries.
- **Names resolve:**
  - each drawn appearance is added to the page's `/Resources /XObject` under the first `FlatN` its existing XObjects do not use (`free_name`, `xobject_names`)
  - the effective resources (the page's own, else the inherited ones from `PageInfo::resources_ref`, an indirect object copied rather than modified) are written as a direct page `/Resources` (`effective_resources`, `with_xobjects`)
  - until #40 `/Fm{n}` was written and never registered (measured, #40)
- **Isolation**: `/Contents` becomes an array of a new stream holding `q`, the existing streams, and a new stream holding `Q` and the drawn content (`add_drawn_content`), so a leftover `cm` cannot move the appearances (`an_unbalanced_cm_in_the_page_content_does_not_move_the_appearance`).
- **Only an indirect appearance stream is drawn**: it is named in `/XObject` by its reference. A direct one is removed without being drawn, deliberately (maintainer decision, #366 wontfix, 2026-10-23). It cannot come from a file: the parser builds a `PdfObject::Stream` only at the top level of an indirect object (`object/mod.rs`, inferred), and justpdf's own appearance writers (`annot/builder.rs`, `form/fill.rs`) insert references. Only a caller that puts a direct stream into a widget through `set_object` reaches it, and the serializer would write that object as an invalid file anyway (inferred).
- **Every `/Widget` is removed** from `/Annots` on every page whose `/Annots` resolves to an array, drawn or not. Hidden and NoView widgets (`placement::is_hidden`) are removed without being drawn. Other annotations stay. Until #40 a page whose widgets had no usable appearance kept them (measured, #40).
- **Not looked at**: the widget's `/OC`, its Print flag, NoZoom and NoRotate (inferred). The renderer skips an annotation whose `/OC` is hidden (`oc_entry_visible` in `render_annotations`); flatten draws it into the page as visible content.
- The field objects stay in the modifier after `/AcroForm` is removed; only `garbage_collect` drops them.
- **Bounds on input**: one pass over each page's `/Annots`; the work is the number of widgets.

## Code
- `justpdf-core/src/form/flatten.rs` — `flatten_form`, `object_at`, `direct`, `effective_resources`, `xobject_names`, `free_name`, `with_xobjects`, `add_drawn_content`
- `justpdf-core/src/annot/placement.rs` — `appearance_matrix`, `normal_appearance`, `form_matrix`, `is_hidden` (shared with the renderer)
- `justpdf-render/tests/render_flatten.rs` — flattened pages rendered and sampled

## Reference behaviour
- ISO 32000-2 §12.5.5 (clause not re-read for #40; the #40 and #206 briefs' statement of it was followed).

## Cross-cutting invariants
- [Hidden content state](../invariant/hidden-content-state.md) — `flatten_form` paints a widget whose own `/OC` is off, which the renderer skips.

## Blast radius
- [Render annotations](render-annotations.md), [Render interpreter](render-interpreter.md) — share `annot::placement`; a change to the mapping, the state choice or the hidden test changes both a rendered page and a flattened one, and a check added only on one side (as `/OC` is on the render side) makes them disagree.
- [Form appearance](form-appearance.md) — the `/BBox [0 0 w h]` it writes is what `appearance_matrix` maps onto `/Rect`; a generator that wrote another BBox or a `/Matrix` would be placed by that.
- [Form fill](form-fill.md) — flatten draws the appearance fill wrote into the modifier; fill writing a direct `/AP /N` stream would be dropped here (#366).
- [AcroForm](acroform.md) — flatten runs only when `parse_acroform` finds a form.
- [Document modifier](document-modifier.md) — reads go to the modifier first; the page, its `/Resources` and `/Contents` are rewritten through `set_object` and `add_object`.

## Known holes / open
- The widget's `/OC`, the Print flag, NoZoom and NoRotate are not looked at (inferred). A widget in hidden optional content is flattened as visible.
- Open question (maintainer): should a widget whose `/OC` is hidden under the default configuration be removed without being drawn, as Hidden and NoView widgets are, so that flattening agrees with the renderer?
