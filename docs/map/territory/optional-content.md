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
- **The raster renderer hides OFF content** (#44): `/OC /Name BDC` looks `Name` up in the current scope's `/Properties` (forms, patterns and annotation appearances included; `optional_content::section_visible`) and skips to the matching `EMC`, nested marked content counted; a Form or image XObject whose `/OC` is hidden is not drawn (`do_xobject`); an annotation whose `/OC` is hidden is not drawn. A nested content stream starts with no hidden section open and the caller's state is restored after it (`nested_stream`), so a hidden `BDC` a form leaves open does not hide the caller's later content, and one the page leaves open does not hide its annotations (both measured by mutation). Until #44 a name was never looked up and XObject and annotation `/OC` were not checked, so OFF content was always drawn (measured 2026-10-06 and 2026-10-24: red where white is right). Pinned by `justpdf-render/tests/render_optional_content.rs` and `justpdf-core/tests/ocg_visibility.rs`.
- **The SVG renderer hides the same content** (#218) through the same `justpdf-render/src/optional_content.rs` (`section_visible` for `BDC`, `entry_visible` for an `/OC` entry, `default_config`), which both interpreters call, so the operand-to-`/OC`-value resolution exists once: hidden `/OC` sections are skipped through the matching `EMC` in `execute_ops`, a Form or image XObject whose `/OC` is hidden is not drawn (`do_xobject`), and `render_form_xobject` starts the form with no hidden section open and restores the caller's after it. Resetting to 0 on entry cannot be observed there: a `Do` inside a hidden section is never executed and the SVG renderer draws no annotations (inferred), so only the restore is pinned (measured by mutation). Until #218 every marked-content operator was a no-op and `Do` ignored `/OC` (measured 2026-10-07 and 2026-10-25: the red fill present). `svg_and_raster_hide_the_same_content` renders each fixture both ways and compares.
- **Text extraction leaves hidden text out by default** (#219): the shared [Content interpreter](content-interpreter.md) flags glyphs in hidden content through `is_oc_visible` and keeps running every operator, as §8.11.3.1 asks; [Text extraction](text-extraction.md) drops the flagged ones unless `TextExtractOptions::include_hidden`. The renderers instead skip every operator in a hidden section, so state changes there are lost (measured 2026-10-25: `/OC /L BDC 1 0 0 1 50 0 cm EMC` then a fill draws at x 0 where §8.11.3.1 puts it at 50, raster and SVG).

## Code
- `justpdf-core/src/ocg/parse.rs` — `read_oc_properties`, `is_ocg_visible`, `is_ocmd_visible`, `parse_ocmd`, `is_oc_visible`, `parse_expression`
- `justpdf-core/src/ocg/builder.rs` — `add_ocg`, `set_ocg_visibility`, `remove_ocg`
- `justpdf-core/src/ocg/types.rs` — `OCConfig`, `OCMembership`, `VisibilityExpression`
- `justpdf-core/src/content/interpret.rs` — `properties_hidden`, `oc_hidden`
- `justpdf-render/src/optional_content.rs` — `default_config`, `section_visible`, `entry_visible`
- `justpdf-render/src/interpreter.rs` — `oc_entry_visible`, `do_xobject`, `render_annotations`, `nested_stream`
- `justpdf-render/src/svg_device.rs` — `execute_ops`, `do_xobject`, `render_form_xobject`

## Reference behaviour
ISO 32000-1 §8.11.2.2, Table 99 (read raw for #44).

## Cross-cutting invariants
**None.**

## Blast radius
- [Render interpreter](render-interpreter.md) — `BDC`, `Do` and the nested-stream boundary.
- [SVG renderer](svg-renderer.md) — `BDC`, `Do` and the form boundary, through `optional_content`.
- [Content interpreter](content-interpreter.md), [Text extraction](text-extraction.md) — glyphs flagged hidden, left out by default.
- [Render annotations](render-annotations.md) — annotation `/OC` checked through `is_oc_visible`.
- [Compress unused resources](compress-unused-resources.md) — `/Properties` is not a cleanup target.

## Known holes / open
- The renderers drop state operators inside hidden sections (measured 2026-10-25, above); the interpreter does not.
