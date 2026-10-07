# Render API (entry points, size, parallelism)

## What it is
The render crate's public entry points: page → PNG/JPEG/RGBA/SVG, saving to a file, DPI/background/format options, computing the page transform, and page-by-page parallel rendering under the `parallel` feature.

## Governing decisions
**None.** [ADR-0001](../../adr/0001-crates-split-by-dependency-layer.md) makes this crate an optional layer.

## Design model
- One private `page_frame` gives every entry point (`render_page_info` and so `render_page` and the parallel variants, `render_page_to_pixmap`, `render_page_to_svg`) the visible box (CropBox, else MediaBox), its size after `/Rotate` and the transform; `render_to_device` holds the raster size check (16384px cap) for both raster paths. Until #54 the three entry points each sized the output from the unrotated box.
- `/Rotate` turns the page clockwise when displayed (ISO 32000-2 §7.7.3.3). It is reduced with `rem_euclid(360)`, so `-90` is 270 and `450` is 90; a value that is not a multiple of 90 is treated as 0. For 90 and 270 the output is the visible box's height wide and its width tall. `compute_page_transform` maps the corner that ends up top-left to the origin; its 90 and 270 matrices have determinant −s², like 0 and 180 (they include the y flip). Until #54 they had +s² and reflected the page across a diagonal: a top-left marker landed bottom-right at 90 and top-left at 270 (measured, `tests/render_rotation.rs`, which covers 0, 90, 180, 270, −90 and 450, the 600×200 size through pixmap, PNG and SVG `viewBox`, and an offset CropBox at 0 and 90).
- `compute_page_bbox` (bbox device) returns PDF user space, as its doc comment says: points go through the CTM only, not the page transform, so `/Rotate` does not change the result (measured 2026-10-07: a `20 350 60 40 re f` fill on a 200×400 page gives 20,350–80,390 at `/Rotate 0` and `90`; #262, closed as not reproduced).
- An index past the last page is `RenderError::Core(JustPdfError::PageOutOfRange { index, count })` from `render_page`, `render_page_to_pixmap`, `render_page_to_svg` and `render_pages_parallel`; `count` is the length of `collect_pages` (#134; measured by `render_test.rs` and the `parallel` unit test).
- `parallel` runs `render_page_info` per page with rayon. It does not split into tiles.

## Code
- `justpdf-render/src/render.rs` — `render_page`, `render_page_info`, `render_page_to_file`, `render_page_to_pixmap`, `render_page_to_svg`, `compute_page_transform`, `RenderOptions`, `OutputFormat`, `RenderedPixmap`, `render_pages_parallel`, `render_all_pages_parallel`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- Consumers — [Facade](facade.md), [CLI](cli.md), [plaintext](plaintext-input.md)/[mobi](mobi-input.md)/[fb2](fb2-input.md) previews, [epub](epub-input.md)/[office](office-input.md) (through plaintext), [OCR](ocr.md), [Language bindings](language-bindings.md). Changing a public signature reaches all of them. Check the consumers with a command: `rg -l 'justpdf_render::' --glob '*.rs' --glob '!target' .`.
- [Render interpreter](render-interpreter.md), [SVG renderer](svg-renderer.md) — internal.
- [Page tree](page-tree.md) — `PageInfo`.

## Known holes / open
- The render test (`tests/render_test.rs`) has a conditional escape that passes silently when the repo-tracked `testpdf.pdf` is missing (it does not fire today, since the file is there).
- Tracked: #150 (`render_pages_parallel` stringifies a `collect_pages` failure; decided in #150: look the page up with `get_page`, inheriting #134's `PageOutOfRange`/`InvalidObject` rule per index inside the parallel map, so each result keeps its core error kind without cloning `JustPdfError`)
