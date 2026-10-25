# Optional content (layers, OCG)

## What it is
Reads the layers (OCG) and membership dictionaries (OCMD) of `/OCProperties`, decides visibility under the default configuration, and adds layers, changes their visibility and removes them. The renderer uses it to decide whether to skip a BDC section.

## Governing decisions
**None.**

## Design model
- **`is_oc_visible(oc, doc, config)` decides an `/OC` value** (#44), in core so the raster renderer, the SVG renderer (#218) and text extraction (#219) share it: an OCG given by reference is visible per `is_ocg_visible`; an OCMD (by reference or inline, `/OCGs` and `/VE` possibly by reference) per `is_ocmd_visible`; anything else (an inline OCG, which has no reference to look up, a value that is neither, a reference that does not resolve) is visible — fail open.
- **`/VE` visibility expressions** are parsed into `VisibilityExpression` (`And`/`Or`/`Not` over OCG references and nested expressions) and, when present, decide visibility instead of `/OCGs` + `/P`: ISO 32000-1 Table 99 NOTE 2, "if it is present … it should be used in preference to OCGs and P" (read raw). An OCMD with neither `/OCGs` nor `/VE` has no effect (Table 99: "the membership dictionary shall have no effect on the visibility of any content"); until #44 `parse_ocmd` returned `None` for it, and a `/VE`-only OCMD could not be read.
- Only the default configuration (`/D`) is used. `/RBGroups`, `/Locked`, `/AS`, `/Intent` and `/Configs` are not handled (maintainer scope, #44 triage: the first two are UI constraints that do not change default visibility; `/AS` needs a usage that does not exist yet). Usage and Intent are parsed but do not affect visibility.
- `add_ocg` only creates the group and does not tag any content.
- **The raster renderer hides OFF content** (#44): `/OC /Name BDC` looks `Name` up in the current scope's `/Properties` (forms, patterns and annotation appearances included; `check_oc_visibility`) and skips to the matching `EMC`, nested marked content counted; a Form or image XObject whose `/OC` is hidden is not drawn (`do_xobject`); an annotation whose `/OC` is hidden is not drawn. A nested content stream starts with no hidden section open and the caller's state is restored after it (`nested_stream`), so a hidden `BDC` a form leaves open does not hide the caller's later content, and one the page leaves open does not hide its annotations (both measured by mutation). Until #44 a name was never looked up and XObject and annotation `/OC` were not checked, so OFF content was always drawn (measured 2026-10-06 and 2026-10-24: red where white is right). Pinned by `justpdf-render/tests/render_optional_content.rs` and `justpdf-core/tests/ocg_visibility.rs`.
- The SVG renderer ignores BDC/EMC and XObject `/OC` (#218), and text extraction does not look at OC (#219: decided to leave hidden text out by default, with a caller option to include it). Both can now call `is_oc_visible`.

## Code
- `justpdf-core/src/ocg/parse.rs` — `read_oc_properties`, `is_ocg_visible`, `is_ocmd_visible`, `parse_ocmd`, `is_oc_visible`, `parse_expression`
- `justpdf-core/src/ocg/builder.rs` — `add_ocg`, `set_ocg_visibility`, `remove_ocg`
- `justpdf-core/src/ocg/types.rs` — `OCConfig`, `OCMembership`, `VisibilityExpression`
- `justpdf-render/src/interpreter.rs` — `check_oc_visibility`, `oc_entry_visible`, `do_xobject`, `render_annotations`, `nested_stream`

## Reference behaviour
ISO 32000-1 §8.11.2.2, Table 99 (read raw for #44).

## Cross-cutting invariants
**None.**

## Blast radius
- [Render interpreter](render-interpreter.md) — `BDC`, `Do` and the nested-stream boundary.
- [SVG renderer](svg-renderer.md), [Text extraction](text-extraction.md) — ignore OC.
- [Render annotations](render-annotations.md) — annotation `/OC` checked through `is_oc_visible`.
- [Compress unused resources](compress-unused-resources.md) — `/Properties` is not a cleanup target.

## Known holes / open
- Tracked: #218 (SVG renderer), #219 (text extraction)
