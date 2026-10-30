# Render API (entry points, size, parallelism)

## What it is
The render crate's public entry points: page → PNG/JPEG/raw RGBA/SVG, saving to a file, the DPI/background/format options, the page transform, and page-by-page parallel rendering under the `parallel` feature.

## Governing decisions
- [ADR-0001](../../adr/0001-crates-split-by-dependency-layer.md) — this crate is the optional rasterization layer above core, pulling in `tiny-skia`.
- [ADR-0006](../../adr/0006-breaking-changes-are-allowed-before-1-0.md) — before 1.0 a signature here may break for a better design; each break is recorded under Changed in `CHANGELOG.md` (#54, #134 were). [The out-of-scope record on narrowing the render API](../../../.out-of-scope/narrowing-render-public-api.md) (2026-10-24, #371) applies it: the crate's internal modules stay `pub`.

## Design model
Read from the source and the issues cited.

- **One private `page_frame` sizes every entry point** (`render_page_info` and so `render_page` and the parallel variants, `render_page_to_pixmap`, `render_page_to_svg`): the visible box (CropBox, else MediaBox), its size after `/Rotate` and the transform. A box with no width or height is `RenderError::InvalidDimensions`. `render_to_device` holds the raster size check for both raster paths (re-verified, `render.rs`). Until #54 the three entry points each sized the output from the unrotated box.
- **What bounds the input**: the caller's `dpi` is bounded only through the output size — `render_to_device` rejects a pixmap wider or taller than 16384 px (or 0) with `RenderError::InvalidDimensions`, so one page is at most 16384 × 16384 × 4 bytes before the clips, groups and soft masks it allocates at the same size (re-verified, `render_to_device`).
- **`/Rotate` turns the page clockwise when displayed** (ISO 32000-2 §7.7.3.3). It is reduced with `rem_euclid(360)`, so `-90` is 270 and `450` is 90; a value that is not a multiple of 90 is treated as 0. For 90 and 270 the output is the visible box's height wide and its width tall. `compute_page_transform` maps the corner that ends up top-left to the origin; its 90 and 270 matrices have determinant −s², like 0 and 180 (they include the y flip) (re-verified, `compute_page_transform`). Until #54 they had +s² and reflected the page across a diagonal: a top-left marker landed bottom-right at 90 and top-left at 270 (measured, #54). `tests/render_rotation.rs` covers 0, 90, 180, 270, −90 and 450, the 600×200 size through pixmap, PNG and SVG `viewBox`, and an offset CropBox at 0 and 90.
- **An index past the last page is `RenderError::Core(JustPdfError::PageOutOfRange { index, count })`** from `render_page`, `render_page_to_pixmap`, `render_page_to_svg` and `render_pages_parallel`; `count` is the length of `collect_pages` (#134; re-verified, and pinned by `render_test.rs` and the `parallel` unit test `test_render_pages_parallel_out_of_range`). `compute_page_bbox` does not follow this rule ([BBox device](bbox-device.md)).
- **Every entry point but `render_page_info` collects the whole page tree** (`collect_pages`) to look up one page (re-verified).
- `render_page_to_file` writes whatever `options.format` produces, despite its local name `png_data` (re-verified).
- `render_page_to_svg` takes no options: one SVG unit is one point and the background is always white ([SVG renderer](svg-renderer.md)).
- `OutputFormat::RawRgba` and `RenderedPixmap::data` are the device's premultiplied pixels ([Raster device](raster-device.md)).
- **`parallel` runs `render_page_info` per page with rayon**; it does not split a page into tiles. `render_all_pages_parallel` returns a single-element `Vec` holding the error when `collect_pages` fails (re-verified).
- The per-document font program cache (`PdfDocument::font_program`, #240) is what parallel rendering shares between pages ([Glyph rendering](glyph-rendering.md)).

## Code
- `justpdf-render/src/render.rs` — `render_page`, `render_page_info`, `render_page_to_file`, `render_page_to_pixmap`, `render_page_to_svg`, `render_pages_parallel`, `render_all_pages_parallel`, `compute_page_transform`, `normalize_rotation`, `page_frame`, `PageFrame`, `render_to_device`, `RenderOptions`, `OutputFormat`, `RenderedPixmap`
- `justpdf-render/src/lib.rs` — the crate root's re-exports
- `justpdf-render/src/error.rs` — `RenderError`, `Result`
- `justpdf-render/tests/render_rotation.rs` — rotation and sizes
- `justpdf-render/tests/render_test.rs` — page lookup and smoke tests on the repo-tracked `testpdf.pdf`

## Reference behaviour
ISO 32000-2 §7.7.3.3 for `/Rotate` (#54). No output of these entry points has been compared with a reference renderer as a whole.

## Cross-cutting invariants
**None.**

## Blast radius
- Consumers — [Facade](facade.md), [CLI](cli.md), [plaintext](plaintext-input.md)/[mobi](mobi-input.md)/[fb2](fb2-input.md) previews, [epub](epub-input.md)/[office](office-input.md) (through plaintext), [OCR](ocr.md), [Language bindings](language-bindings.md). They call these functions and match on `RenderError`, so a changed signature, error kind or output size reaches all of them. Check them with a command: `rg -l 'justpdf_render::' --glob '*.rs' --glob '!target' .`.
- [Language bindings](language-bindings.md) — report page width and height from the unrotated box while rendering returns the rotated size (#263); a change to how `page_frame` sizes pages widens or closes that gap.
- [Render interpreter](render-interpreter.md), [SVG renderer](svg-renderer.md) — take the page transform from `page_frame` as their only mapping to output space.
- [Page tree](page-tree.md) — `PageInfo`'s boxes and `rotate` decide the output size; `collect_pages` decides the page count and its errors.

## Known holes / open
- `tests/render_test.rs` returns early and passes when the repo-tracked `testpdf.pdf` is missing (re-verified; it does not fire today, since the file is there).
- `render_pages_parallel` stringifies a `collect_pages` failure into `RenderError::InvalidDimensions`. Decided in #150: look the page up with `get_page`, inheriting #134's `PageOutOfRange`/`InvalidObject` rule per index inside the parallel map, so each result keeps its core error kind without cloning `JustPdfError`. Tracked: #150
- The bindings' page size is the unrotated box while rendering uses the rotated size. Tracked: #263
