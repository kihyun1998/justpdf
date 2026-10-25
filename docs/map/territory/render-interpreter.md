# Render interpreter (operator dispatch, graphics state)

## What it is
`RenderInterpreter` takes the page content and executes it operator by operator, keeping the graphics state (CTM, color, line, text state, blend) and sending drawing requests to the raster device. `interpreter.rs` is one file but holds several concepts — images, clipping, transparency, shading, patterns, glyphs, annotations and OCG each have their own note, and this note covers only the driver, the dispatch and the graphics state.

## Governing decisions
**None.** [ADR-0001](../../adr/0001-crates-split-by-dependency-layer.md) decides only that rendering is a separate layer that pulls in `tiny-skia`.

## Design model
- **There is no device abstraction**: `RenderInterpreter` holds a `&mut PixmapDevice` directly (no `trait` defined). SVG output is not a device but a copy of the interpreter ([SVG renderer](svg-renderer.md)).
- `execute_op` is one big `match` on the operator string. An unknown operator is silently ignored by `_ => {}` (e.g. `ri`, `i`). Rendering Intent is not applied.
- `Tr` distinguishes only mode 3 (invisible) — there is no outline or clipping text (inferred).
- `d0`/`d1` are no-ops — [Type3 fonts](type3-fonts.md) are not supported.
- `v` (Bézier) is approximated without tracking the current point ("lossy without current point tracking").
- **`cs`/`CS` resolve a colour space by name** (`colour_space`, #231): DeviceGray/RGB/CMYK by name, `/Pattern` as the pattern space, any other name looked up in the current scope's `/Resources /ColorSpace` (forms, patterns and annotation appearances included) and resolved with `ColorSpace::resolve`; a name with no resource is DeviceRGB. Setting a space sets its initial colour (`ColorSpace::initial_color`, ISO 32000-2 §8.6.8: 0 per component, CMYK `0 0 0 1`, Indexed 0, Separation/DeviceN tint 1); `sc`/`scn` components convert through `Color::to_rgb`. A resource that names a pattern space (`[/Pattern]`) is resolved like any other: the `scn` that names the pattern selects it again, so coloured patterns paint the same (measured by mutation: treating it as `/Pattern` changed no test; uncoloured patterns are out of scope). Until #231 every name went through a local `cs_from_name` (Device names only, anything else DeviceRGB, measured 2026-10-06: an Indexed `1 scn` drew red instead of blue, a Separation tint was read as R, and `cs` left the previous colour). Pinned by `tests/render_colour_spaces.rs`.
  - Real files (measured 2026-10-24, 18 local PDFs, 100 pages, pixel-compared against master): 4 pages changed, all colours master misread. In one file, table rows painted in a resource colour space came out solid red on master; they now come out white with the intended highlights (69% and 41% of the page). In another, an `[/ICCBased <</N 1 /Alternate /DeviceGray>>]` gray was read as RGB `(g,0,0)` and tinted a blue band (470 pixels per page). The other 96 pages are pixel-identical.
- Nested content streams run through `nested_stream`, which floors the graphics-state stack and restores the caller's state and path ([Content stream recursion](../invariant/content-stream-recursion.md), #128).
- Other content streams (Form XObject, tile, soft mask `/G`) are entered through `with_running_stream`: a stream met again while it is running is skipped — [Content stream recursion](../invariant/content-stream-recursion.md). Separately, a Form XObject is skipped past depth 10.
- The device color operators (`g`/`rg`/`k`, `G`/`RG`/`K`) clear that side's pattern selection.
- Page content is assembled by its own function — [Page content assembly](../invariant/page-content-assembly.md).
- Resource names resolve in the `/Resources` of the streams being executed, innermost first. `Tf` and `scn`/`SCN` put the resolved object in the graphics state. A font is resolved when first selected and cached by `FontKey` — [resource name scope](../invariant/resource-scope.md).
- `Tf` records the `FontKey` a name selected in the innermost scope (`ResourceScopes::select_font`) and reuses it until that scope is left. Measured 2026-09-30 on a local 392-page document at 72 dpi (minimum of 6 alternating runs): walking the scopes on every `Tf` rendered it in 9.64 s against 7.15 s before #127; with the per-scope record, 6.47 s against 6.49 s (10 runs).
- `gs` applies an ExtGState's `/Font [font size]` as `Tf` would (#238): `apply_extgstate` takes the font by reference through `select_font_ref`, keyed `FontKey::Object(ref)` like a `/Font` resource reached by reference, so one font object reached through `Tf` and through `gs` shares a `fonts` entry, and a font named in no `/Font` dictionary is resolved too. The text state lives in `GraphicsState`, so `q`/`Q` restore it. A malformed entry (not a two-element array, a first element that is not a reference, a size that is not a number, or a reference that does not resolve to a dictionary) changes neither font nor size and raises no error. The SVG renderer does the same ([SVG renderer](svg-renderer.md)). Until #238 `gs` read only `LW`, `LC`, `LJ`, `ML`, `ca`, `CA`, `BM` and `SMask`: `/GS2 gs (ABC) Tj` with `/GS2` setting `/Font [<Courier> 12]` and no prior `Tf` drew nothing, and after `/F1 12 Tf` it kept F1 (measured 2026-10-07). Text extraction still ignores it (#235, blocked by #207), so such text renders right and extracts wrong until then ([Font resolution](../invariant/font-resolution.md)).

## Code
- `justpdf-render/src/interpreter.rs` — `RenderInterpreter`, `render_page`, `get_page_content`, `concat_content_streams`, `execute_ops`, `execute_op`, `with_running_stream`, `with_stream_resources`, `select_font`, `select_font_ref`, `apply_extgstate`, `resolve_font`, `resolve_xobject`, `effective_transform`, `colour_space`
- `justpdf-render/src/resources.rs` — `ResourceScopes`, `Resource`, `FontSelection`, `select_font`, `select_pattern`, `entry`
- `justpdf-render/tests/render_extgstate_font.rs` — `svg_text_after_gs_alone_uses_its_font_and_size`, `svg_gs_font_need_not_be_in_the_font_resources`, `svg_a_malformed_gs_font_changes_nothing`, `raster_text_after_gs_alone_is_drawn`
- `justpdf-render/src/graphics_state.rs` — `GraphicsState`, `TextState`, `FontKey`, `PatternSelection`, `Matrix`, `PdfBlendMode`, `fill_color_rgba`

## Reference behaviour
**None.** Clauses to compare against: ISO 32000-2 §8.4 (graphics state), Annex A. No record of comparing render results with MuPDF (example).

## Cross-cutting invariants
- [Page content assembly](../invariant/page-content-assembly.md)
- [Font resolution](../invariant/font-resolution.md) — `resolve_font` resolves fonts apart from text extraction.
- [resource name scope](../invariant/resource-scope.md)
- [Content stream recursion](../invariant/content-stream-recursion.md)

## Blast radius
- [Raster device](raster-device.md) — the only output target.
- [SVG renderer](svg-renderer.md), [BBox device](bbox-device.md) — copies of the same dispatch. When fixing operator handling, look at the copies too.
- [Render images](render-images.md), [Render clipping](render-clipping.md), [Render transparency](render-transparency.md), [Render shading](render-shading.md), [Render tiling patterns](render-tiling-patterns.md), [Glyph rendering](glyph-rendering.md), [Render annotations](render-annotations.md), [optional content](optional-content.md) — sub-concepts inside the same file.
- [Content stream parsing](content-stream-parsing.md) — input.
- [Color spaces](color-spaces.md) — the core module it bypasses.
- [Render API](render-api.md) — caller.

## Known holes / open
- `interpreter.rs` and `graphics_state.rs` have no unit tests. Render tests that read pixels are `tests/render_recursion.rs` (recursion paths) and `tests/render_resources.rs` (resource name scope); the other integration tests check only the PNG magic bytes and length.
