# Optional content (layers, OCG)

## What it is
Reads the layers (OCG) and membership dictionaries (OCMD) of `/OCProperties`, decides visibility under the default configuration, and adds layers, changes their visibility and removes them. The renderer uses it to decide whether to skip a BDC section.

## Governing decisions
**None.**

## Design model
- `/VE` visibility expressions, `/RBGroups`, `/Locked` and `/AS` are not handled. Usage and Intent are parsed but do not affect visibility.
- `add_ocg` only creates the group and does not tag any content.
- **In the renderer, layers are effectively always visible**: the common form (`/OC /Name BDC`) is always visible, with "TODO: look up in page /Resources /Properties dict"; an inline `/Type /OCG` is visible too; only an inline OCMD is evaluated. XObject `/OC` and annotation `/OC` are not checked.
- The SVG renderer ignores BDC/EMC, and text extraction does not look at OC.

## Code
- `justpdf-core/src/ocg/parse.rs` — `read_oc_properties`, `is_ocg_visible`, `is_ocmd_visible`, `parse_ocmd`
- `justpdf-core/src/ocg/builder.rs` — `add_ocg`, `set_ocg_visibility`, `remove_ocg`
- `justpdf-core/src/ocg/types.rs` — `OCConfig`
- `justpdf-render/src/interpreter.rs` — `check_oc_visibility`, `check_oc_dict_visibility`

## Reference behaviour
**None.** Clause to compare against: ISO 32000-2 §8.11.

## Cross-cutting invariants
**None.**

## Blast radius
- [Render interpreter](render-interpreter.md) — the point where visibility is checked (where the `/Properties` lookup would go).
- [SVG renderer](svg-renderer.md), [Text extraction](text-extraction.md) — ignore OC.
- [Render annotations](render-annotations.md) — annotation `/OC` not checked.
- [Compress unused resources](compress-unused-resources.md) — `/Properties` is not a cleanup target.

## Known holes / open
- The `/Properties` name lookup is still a renderer TODO.
- Tracked: #44 (OCG always visible)
