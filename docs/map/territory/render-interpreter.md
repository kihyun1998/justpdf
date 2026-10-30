# Render interpreter (operator dispatch, graphics state)

## What it is
`RenderInterpreter` executes a page's content operator by operator, keeps the graphics state (CTM, colour, line, text state, alpha, blend mode, clip, soft mask, pattern selection), and sends drawing requests to the raster device. `interpreter.rs` is one file but holds several concepts: images, clipping, transparency, shading, patterns, glyphs, annotations and optional content each have their own note. This note covers the driver, the dispatch and the graphics state.

## Governing decisions
**None.** [ADR-0001](../../adr/0001-crates-split-by-dependency-layer.md) decides only that rendering is a separate layer that pulls in `tiny-skia`. [ADR-0006](../../adr/0006-breaking-changes-are-allowed-before-1-0.md) keeps `interpreter` and `graphics_state` public modules whose changes may break the API before 1.0; it does not decide how they work.

## Design model
The rules below come from reading the source and from the issues cited; no decision record states them.

- **There is no device abstraction.** `RenderInterpreter` holds a `&mut PixmapDevice` directly, and the crate defines no device `trait` (inferred, `grep trait` over `justpdf-render/src`). It also reaches into `PixmapDevice::pixmap`: transparency groups, soft masks and tiles swap their own pixmap in, and shadings write pixels into it. SVG output is not a device but a second interpreter ([SVG renderer](svg-renderer.md)).
- **`execute_op` is one `match` on the operator string.** An operator it does not name falls to `_ => {}` and is ignored, `ri` and `i` included, so rendering intent and flatness are not applied (inferred, `execute_op`).
- `Tr` is read only to test for mode 3 (invisible) in `render_text_string`. Modes 1 and 2 fill instead of stroking, and modes 4–7 add no clip (inferred).
- `d0`/`d1` are no-ops: [Type3 fonts](type3-fonts.md) are not drawn. Tracked: #226
- `v` takes its first control point from its own first operand pair instead of the current point, which the dispatch does not track; the code comment calls it lossy (inferred, the `v` arm of `execute_op`).
- **`cs`/`CS` resolve a colour space by name** (`colour_space`, #231). DeviceGray/RGB/CMYK (and `G`/`RGB`/`CMYK`) by name; `/Pattern` as the pattern space; any other name is looked up in the current scope's `/Resources /ColorSpace` (forms, patterns and annotation appearances included) and resolved with `ColorSpace::resolve`. A name with no resource is DeviceRGB (re-verified, `colour_space`).
  - Setting a space sets its initial colour (`ColorSpace::initial_color`, ISO 32000-2 §8.6.8: 0 per component, CMYK `0 0 0 1`, Indexed 0, Separation/DeviceN tint 1), and `sc`/`scn` components convert through `Color::to_rgb`. Pinned by `tests/render_colour_spaces.rs`.
  - A resource that names a pattern space (`[/Pattern]`) is resolved like any other: the `scn` that names the pattern selects it again, so coloured patterns paint the same (measured by mutation in #231: treating it as `/Pattern` changed no test; uncoloured patterns are out of scope, see [Render tiling patterns](render-tiling-patterns.md)).
  - Until #231 every name went through a local `cs_from_name` (Device names only, anything else DeviceRGB). Measured 2026-10-06 (#231): an Indexed `1 scn` drew red instead of blue, a Separation tint was read as R, and `cs` left the previous colour.
  - Real files (measured 2026-10-24, #231, 18 local PDFs, 100 pages, pixel-compared against master): 4 pages changed, all colours master misread. In one file, table rows painted in a resource colour space came out solid red on master; they now come out white with the intended highlights (69% and 41% of the page). In another, an `[/ICCBased <</N 1 /Alternate /DeviceGray>>]` gray was read as RGB `(g,0,0)` and tinted a blue band (470 pixels per page). The other 96 pages are pixel-identical.
- **The device colour operators** (`g`/`rg`/`k`, `G`/`RG`/`K`) set that side's Device space and clear its pattern selection (re-verified, `execute_op`) — [Content stream recursion](../invariant/content-stream-recursion.md).
- **Nested content streams run through `nested_stream`**, which floors the graphics-state stack and restores the caller's state, path, optional-content depth and `stream_start` afterwards (#128) — [Content stream recursion](../invariant/content-stream-recursion.md).
- **Form XObjects, tiles and soft mask `/G` forms are entered through `with_running_stream`**: a stream met again while it is running is skipped — [Content stream recursion](../invariant/content-stream-recursion.md). Separately, `do_xobject` skips a Form XObject when `xobject_depth > 10`, so at most 11 nested forms run. Only `Do` counts toward that depth; tiles, soft mask forms and annotation appearances do not (inferred, `do_xobject`).
- Page content is assembled by its own function, `get_page_content` / `concat_content_streams` — [Page content assembly](../invariant/page-content-assembly.md).
- **Resource names resolve in the `/Resources` of the streams being executed, innermost first** (`ResourceScopes`). `Tf` and `scn`/`SCN` put the resolved object in the graphics state. A font is resolved when first selected and cached by `FontKey` — [resource name scope](../invariant/resource-scope.md).
  - `Tf` records the `FontKey` a name selected in the innermost scope (`ResourceScopes::select_font`) and reuses it until that scope is left. Measured 2026-09-30 (#127) on a local 392-page document at 72 dpi (minimum of 6 alternating runs): walking the scopes on every `Tf` rendered it in 9.64 s against 7.15 s before #127; with the per-scope record, 6.47 s against 6.49 s (10 runs).
- **`gs` applies an ExtGState's `/Font [font size]` as `Tf` would** (#238). `apply_extgstate` takes the font by reference through `select_font_ref`, keyed `FontKey::Object(ref)` like a `/Font` resource reached by reference, so one font object reached through `Tf` and through `gs` shares a `fonts` entry, and a font named in no `/Font` dictionary is resolved too. The text state lives in `GraphicsState`, so `q`/`Q` restore it. A malformed entry (not a two-element array, a first element that is not a reference, a size that is not a number, or a reference that does not resolve to a dictionary) changes neither font nor size and raises no error (re-verified, `apply_extgstate`). The SVG renderer does the same ([SVG renderer](svg-renderer.md)). Text extraction reads the entry too: core `content::interpret` looks up the ExtGState's `/Font` (re-verified, `justpdf-core/src/content/interpret.rs`; #235 closed) — [Font resolution](../invariant/font-resolution.md).
  - Until #238 `gs` read only `LW`, `LC`, `LJ`, `ML`, `ca`, `CA`, `BM` and `SMask`: `/GS2 gs (ABC) Tj` with `/GS2` setting `/Font [<Courier> 12]` and no prior `Tf` drew nothing, and after `/F1 12 Tf` it kept F1 (measured 2026-10-07, #238).
- **What bounds the input.** The content comes from the document. Nothing in the renderer bounds the size of a decoded content stream or its operator count: `get_page_content` takes what core `decode_stream` returns (inferred; the core question is #161). Form nesting is bounded at 11 by `xobject_depth`. Chains of distinct tiles and soft mask forms are bounded only by the running-stream check, which stops repeats, not depth (inferred).
- Errors inside a nested draw are dropped, not returned: `do_xobject` ignores a failed image or form, and `render_page` ignores `render_annotations`' result (re-verified, `let _ =` at each site).

## Code
- `justpdf-render/src/interpreter.rs` — `RenderInterpreter`, `RenderInterpreter::render_page`, `get_page_content`, `concat_content_streams`, `execute_ops`, `execute_op`, `effective_transform`, `blend_mode`, `with_running_stream`, `with_stream_resources`, `nested_stream`, `select_font`, `select_font_ref`, `apply_extgstate`, `resolve_font`, `resolve_xobject`, `colour_space`, `dict_entry`, `StreamStart`, `XObjectData`
- `justpdf-render/src/resources.rs` — `ResourceScopes`, `ResourceScopes::for_page`, `ResourceScopes::enter`, `ResourceScopes::leave`, `ResourceScopes::lookup`, `ResourceScopes::select_font`, `ResourceScopes::select_pattern`, `Resource`, `FontSelection`
- `justpdf-render/src/graphics_state.rs` — `GraphicsState`, `GraphicsState::fill_color_rgba`, `GraphicsState::stroke_color_rgba`, `TextState`, `FontKey`, `PatternSelection`, `Matrix`, `PdfBlendMode`
- `justpdf-render/tests/render_extgstate_font.rs` — `svg_text_after_gs_alone_uses_its_font_and_size`, `svg_gs_font_need_not_be_in_the_font_resources`, `svg_a_malformed_gs_font_changes_nothing`, `raster_text_after_gs_alone_is_drawn`
- `justpdf-render/tests/render_colour_spaces.rs` — `cs`/`CS` by resource, initial colours
- `justpdf-render/tests/render_resources.rs` — names resolved innermost scope first
- `justpdf-render/tests/render_recursion.rs` — a stream met again while running is skipped
- `justpdf-render/tests/render_stream_floor.rs` — the stack floor of nested streams

## Reference behaviour
ISO 32000-2 §8.6.8 for the initial colour a colour space sets (#231). No comparison of the dispatch or the graphics state as a whole with MuPDF is recorded; the comparisons made for single areas are in their notes.

## Cross-cutting invariants
- [Page content assembly](../invariant/page-content-assembly.md)
- [Font resolution](../invariant/font-resolution.md) — `resolve_font` resolves fonts apart from text extraction.
- [resource name scope](../invariant/resource-scope.md)
- [Content stream recursion](../invariant/content-stream-recursion.md)
- [Hidden content keeps its state](../invariant/hidden-content-state.md)

## Blast radius
- [Raster device](raster-device.md) — the interpreter swaps `PixmapDevice::pixmap` for groups, soft masks and tiles and writes shading pixels into it; it assumes one page-sized pixmap it can reach.
- [SVG renderer](svg-renderer.md), [BBox device](bbox-device.md) — each repeats the operator dispatch. A fix to an operator here does not reach them; #231 and #238 changed both interpreters by hand.
- [Content interpreter](content-interpreter.md) — core's own operator walk for text extraction. Text advance is computed separately in each, so a rule like word spacing on two-byte code 32 (#314) or vertical writing (#324) has to change in both.
- [Render images](render-images.md), [Render clipping](render-clipping.md), [Render transparency](render-transparency.md), [Render shading](render-shading.md), [Render tiling patterns](render-tiling-patterns.md), [Glyph rendering](glyph-rendering.md), [Render annotations](render-annotations.md) — same file; they share `GraphicsState`, `nested_stream` and `draw_mask`, so a change to what the state holds or what a nested stream resets reaches each.
- [Optional content](optional-content.md) — `oc_skip_depth` decides which painting operators draw.
- [Content stream parsing](content-stream-parsing.md) — `execute_op` matches on the operator strings and operand types the parser produces.
- [Color spaces](color-spaces.md) — every colour goes through `ColorSpace::resolve`, `initial_color` and `Color::to_rgb`; a changed component count or initial colour changes every paint.
- [Render API](render-api.md) — the caller; the page transform it passes is the only mapping to device space.

## Known holes / open
- `interpreter.rs` has three unit tests, all for `mask_bounds`; `graphics_state.rs` and `resources.rs` have none. The behaviour is pinned by integration tests that read pixels (about 40 files under `tests/`; measured 2026-10-30: `cargo test -p justpdf-render --features parallel` passes at `88b2a08`).
- Rendering intent (`ri`, `/RI`), flatness (`i`, `/FL`) and text render modes other than 0 and 3 are not applied (inferred, above).
- Tiles and soft mask forms do not count toward `xobject_depth`; a long chain of distinct ones nests without limit (inferred, not measured).
- Type3 glyphs are not drawn. Tracked: #226
- Vertical writing (WMode 1) is drawn as a horizontal run. Tracked: #324
- Word spacing is added to every code 32, including a two-byte code of a Type0 font (inferred, `render_text_string`). Tracked: #314
