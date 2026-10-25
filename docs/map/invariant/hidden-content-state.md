# Hidden content keeps its state

## The fact
Content inside a hidden optional-content section (`/OC <name> BDC … EMC` whose group or OCMD is OFF under the configuration in use) **is not drawn, but every state change in it still happens**. ISO 32000-1 §8.11.3.1 (read raw, 2026-10-25): "Graphics state operations, such as setting the colour, transformation matrix, and clipping, shall still be applied. In addition, graphics state side effects that arise from drawing operators shall be applied; in particular, the current text position shall be updated even for text wrapped in optional content." So after a hidden section, `cm`, `q`/`Q`, colour, `W … n`, `gs`, the text state and the text matrix are what they would be with the section visible.

What is not done inside a hidden section: path painting (`S s f F f* B B* b b*`) draws nothing but still ends the path; text-showing operators advance the text matrix and draw no glyph; `Do`, `BI` and `sh` are skipped. A Form XObject or annotation hidden by its own `/OC` "may be skipped entirely", because its state does not outlive its stream (same clause). `BMC`/`BDC`/`EMC` are counted to find the section's own `EMC` (§8.11.3.2 NOTE 2: an inner section is hidden whenever an outer one is).

## Why it is cross-cutting
There are three content-stream executors, none calling another: `RenderInterpreter` (raster), `SvgRenderer` (SVG) and the core content interpreter (text extraction, and redaction later). Each tracks marked-content nesting itself. The easy way to hide a section, skipping every operator to its `EMC`, is wrong at every one of them, and it looks right on any test that draws only inside the section.

## Territories it holds in
- [Render interpreter](../territory/render-interpreter.md) — `execute_ops`: in a hidden section, painting operators clear `path_builder`, `Do`/`BI`/`sh` are skipped, everything else runs `execute_op`; `render_text_string` draws no glyph while `oc_skip_depth > 0`.
- [SVG renderer](../territory/svg-renderer.md) — `execute_ops` and `render_text_string`, the same rule with `path_data`.
- [Content interpreter](../territory/content-interpreter.md) — runs every operator and flags glyphs `hidden` (#219); it never drops them, so redaction can erase them (ADR 0004).
- [Optional content](../territory/optional-content.md) — what decides "hidden".

## What a violation looks like
Measured 2026-10-25 on master after #44 and #218 (both renderers skipped every operator in a hidden section): `/OC /L BDC 1 0 0 1 50 0 cm EMC 0 0 1 rg 0 0 10 10 re f` with `/L` OFF drew the blue square at x 0–10 where it belongs at 50–60 (raster pixel (5,95) blue, (55,95) white; SVG `transform="matrix(1,0,0,-1,0,100)"`). Likewise a colour or a clip set inside a hidden section was lost, and text after hidden text started where the hidden text began. `justpdf-render/tests/render_optional_content.rs` (`assert_hidden_like`) renders each case against the same page with the hidden part replaced by its state-only equivalent, raster and SVG; a mutation turning any one rule off at either renderer fails a named test there.

## Discovery history
Found 2026-10-25 while reading §8.11.3.1 for #219 (text extraction). The skip-everything loop in the raster interpreter predates #44, but before #44 an `/OC` name never resolved, so it never ran; #44 and #218 made it reachable. The content interpreter was written to the rule from the start (#219).

## Where it will recur
**New code that executes content operators and honours optional content.** Command to find it: `rg -n 'oc_skip_depth|hidden_at|"EMC"' justpdf-*/src`. A walker that only collects (resources in use, `writer/compress.rs`) does not hide anything and is not subject to it. Redaction built on the content interpreter inherits the rule through it.
