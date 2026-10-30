# Optional content (layers, OCG)

## What it is
Reads a document's optional content groups (OCGs) and membership dictionaries (OCMDs) from `/OCProperties`, decides whether a piece of content is visible under the default configuration (`is_oc_visible`), and adds groups, changes their default visibility and removes them through a `DocumentModifier`. The decision is consumed by the raster renderer, the SVG renderer and the core [Content interpreter](content-interpreter.md) (text extraction).

## Governing decisions
- [ADR 0005 — Sanitizing removes unpainted text by category](../../adr/0005-sanitizing-removes-unpainted-text-by-category.md), "Unseen content" and its rejected option: hidden optional content means off in the default configuration (`/D`), the configuration text extraction judges by, because judging differently would let sanitizing and extraction disagree about the same content. The groups themselves stay. Sanitizing is not built.
- [ADR 0004](../../adr/0004-redaction-never-under-erases.md) decides that redaction judges optional content by geometry, never by visibility; it constrains what callers do with this decision, not the decision.

## Design model
From the ADRs above for which configuration counts; the rest from reading the source, the maintainer scope cited, and the tests named.
- **`is_oc_visible(oc, doc, config)` decides an `/OC` value** (#44), in core so the raster renderer, the SVG renderer (#218) and text extraction (#219) share it: an OCG given by reference is visible per `is_ocg_visible` (`BaseState`, then `/ON` or `/OFF`); an OCMD, by reference or inline, with `/OCGs` and `/VE` possibly by reference, per `is_ocmd_visible`; anything else — an inline OCG, which has no reference to look up, a value of another type, a reference that does not resolve — is visible (fail open).
- **`/VE` visibility expressions** are parsed into `VisibilityExpression` (`And`/`Or`/`Not` over OCG references and nested expressions) and, when present, decide instead of `/OCGs` and `/P`: ISO 32000-1 Table 99 NOTE 2, "if it is present … it should be used in preference to OCGs and P" (read raw for #44). An OCMD with neither `/OCGs` nor `/VE` has no effect (Table 99: "the membership dictionary shall have no effect on the visibility of any content"); until #44 `parse_ocmd` returned `None` for it, and a `/VE`-only OCMD could not be read. `/P` defaults to `AnyOn`. Pinned by `tests/ocg_visibility.rs`.
- **Only the default configuration (`/D`) is used.** `/RBGroups`, `/Locked`, `/AS`, `/Intent` and `/Configs` do not affect visibility (maintainer scope at #44 triage: the first two are UI constraints that do not change default visibility; `/AS` needs a usage that does not exist yet). `/Configs`, `/Order`, `/Intent` and `/Usage` are parsed into `OCProperties`, `OCConfig`, `OCGroup` for callers.
- **Where content is judged**, by consumer:
  - Raster renderer (#44): `/OC /Name BDC` looks `Name` up in the current scope's `/Properties` (forms, patterns and annotation appearances included) through `optional_content::section_visible`, and draws nothing through the matching `EMC`, nested marked content counted, while state operators still run — [Hidden content keeps its state](../invariant/hidden-content-state.md). A Form or image XObject whose `/OC` is hidden is not drawn (`do_xobject`); an annotation whose `/OC` is hidden is not drawn (`render_annotations`). A nested stream starts with no hidden section open and the caller's is restored after it (`nested_stream`). Until #44 no name was looked up and XObject and annotation `/OC` were not checked, so OFF content was always drawn (measured 2026-10-06 and 2026-10-24). Pinned by `justpdf-render/tests/render_optional_content.rs`.
  - SVG renderer (#218): the same `optional_content` functions, so the operand-to-`/OC` resolution exists once for both renderers; hidden sections in `execute_ops`, hidden XObjects in `do_xobject`, and `render_form_xobject` starts the form with no hidden section open and restores the caller's. The SVG renderer draws no annotations (inferred). Until #218 every marked-content operator was a no-op there (measured 2026-10-07 and 2026-10-25). `svg_and_raster_hide_the_same_content` renders each fixture both ways and compares.
  - Content interpreter (#219): flags glyphs in hidden content and runs every operator; [Text extraction](text-extraction.md) drops flagged glyphs unless `TextExtractOptions::include_hidden`. The renderers took the same "state still applies" rule after #219; until then they skipped every operator in a hidden section (measured 2026-10-25: `/OC /L BDC 1 0 0 1 50 0 cm EMC` then a fill drew at x 0 where §8.11.3.1 puts it at 50, raster and SVG).
  - The two sides differ on an inline property list (`/OC <<…>> BDC`): the renderers convert it (`operand_to_pdf_object`) and evaluate it; the content interpreter reads only a named one and treats an inline one as visible (measured on the extraction side, see [Content interpreter](content-interpreter.md)).
- **Editing** (`ocg/builder.rs`): `add_ocg` creates the group with `/Name` as the UTF-8 bytes of a Rust `&str` and adds it to `/OCGs`, to `/D`'s `/ON` or `/OFF` as `BaseState` requires, and to `/Order`; it tags no content. `set_ocg_visibility` moves a group between `/ON` and `/OFF`. `remove_ocg` takes it out of `/OCGs` and of `/D`'s `/ON`, `/OFF` and `/Order`, and leaves content that names it. All three handle only a direct `/OCProperties`, `/D` and direct arrays (below).
- **Text**: OCG and configuration `/Name` and `/Creator` are read as lossy UTF-8 and written as raw UTF-8 — [Text string encoding](../invariant/text-string-encoding.md).
- **Bounds on input**: groups, configurations and `/ON`/`/OFF` lists are as long as the file makes them; `/Order` and `/VE` are walked recursively as deep as the object parser nested them, which nothing bounds. `is_ocg_visible` scans the `/ON`/`/OFF` list per decision, so a page deciding many sections against a long list costs their product (inferred).

## Code
- `justpdf-core/src/ocg/parse.rs` — `read_oc_properties`, `parse_ocg`, `parse_config`, `parse_order`, `is_ocg_visible`, `is_ocmd_visible`, `is_expression_true`, `parse_ocmd`, `parse_expression`, `is_oc_visible`
- `justpdf-core/src/ocg/builder.rs` — `add_ocg`, `set_ocg_visibility`, `remove_ocg`
- `justpdf-core/src/ocg/types.rs` — `OCGroup`, `OCGUsage`, `OCGState`, `VisibilityPolicy`, `OCMembership`, `VisibilityExpression`, `OCConfig`, `OCOrderItem`, `OCProperties`
- `justpdf-core/src/content/interpret.rs` — `properties_hidden`, `DocResources::oc_hidden`
- `justpdf-render/src/optional_content.rs` — `default_config`, `section_visible`, `entry_visible`, `operand_to_pdf_object`
- `justpdf-render/src/interpreter.rs` — `oc_entry_visible`, `do_xobject`, `render_annotations`, `nested_stream`
- `justpdf-render/src/svg_device.rs` — `execute_ops`, `do_xobject`, `render_form_xobject`

## Reference behaviour
- ISO 32000-1 §8.11.2.2 and Table 99 (OCMD, `/VE` precedence), §8.11.3.1–2 (content in hidden sections), read raw for #44, #219 and the hidden-state fix.

## Cross-cutting invariants
- [Hidden content keeps its state](../invariant/hidden-content-state.md) — what "hidden" decided here means for the walkers.
- [Text string encoding](../invariant/text-string-encoding.md) — OCG `/Name` read lossily and written as UTF-8.

## Blast radius
- [Render interpreter](render-interpreter.md) — `BDC`, `Do`, annotations and the nested-stream boundary consume the decision; a change to fail-open or to the configuration used changes what is drawn.
- [SVG renderer](svg-renderer.md) — the same, through `optional_content`; the two renderers must keep matching (`svg_and_raster_hide_the_same_content`).
- [Content interpreter](content-interpreter.md), [Text extraction](text-extraction.md) — what extraction leaves out by default; ADR 0005 requires it to judge as the renderers and sanitizing do, and inline property lists already differ.
- [Render annotations](render-annotations.md) — annotation `/OC` is checked through `entry_visible`.
- [Redaction](redaction.md) — must not consult this decision (ADR 0004); erasing hidden content like visible content depends on it staying out.
- [Compress unused resources](compress-unused-resources.md) — `/Properties` entries are not a cleanup target, so names that point at groups survive.
- [Clean](clean.md) — `clean_objects` merges value-equal OCGs into one, putting one group in both `/ON` and `/OFF` (#82).

## Known holes / open
- `add_ocg` on a document whose `/OCProperties` is an indirect object replaces it with a new dictionary holding only the new group, dropping the existing ones; `set_ocg_visibility` reports "no OCProperties in catalog" there, and `remove_ocg` does nothing (measured 2026-10-30, probe: groups `["Old"]` before `add_ocg(…, "New", true)`, `["New"]` after; the reader `read_oc_properties` accepts an indirect one). An indirect `/D` or `/OCGs` array is dropped the same way (inferred). Tracked: #422.
- Inline property lists are judged differently by the renderers and the content interpreter (above). Which side moves is a maintainer question: the interpreter follows §8.11.3.2; ADR 0005 asks that extraction and sanitizing agree.
- `clean_objects` can merge distinct OCGs with equal values. Tracked: #82.
- Sanitizing hidden optional content is decided and not built. Tracked: #321.
- What stays out of scope (`/RBGroups`, `/Locked`, `/AS`, `/Intent`, `/Configs` for visibility) is out by the #44 maintainer scope, above.
