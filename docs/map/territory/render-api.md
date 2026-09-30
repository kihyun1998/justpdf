# Render API (entry points, size, parallelism)

## What it is
The render crate's public entry points: page → PNG/JPEG/RGBA/SVG, saving to a file, DPI/background/format options, computing the page transform, and page-by-page parallel rendering under the `parallel` feature.

## Governing decisions
**None.** [ADR-0001](../../adr/0001-crates-split-by-dependency-layer.md) makes this crate an optional layer.

## Design model
- `render_page_to_pixmap` duplicates the size computation and validation (16384px cap) of `render_page_info`.
- `compute_page_transform` handles rotation, but the pixel width and height come from the unrotated box — a 90/270° page's size can come out swapped (inferred; no rotation test).
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
- The render test (`tests/render_test.rs`) has a conditional escape that passes silently when the repo-tracked `testpdf.pdf` is missing (it does not fire today, since the file is there). Only rotation 0 is tested.
- Tracked: #54 (render size of rotated pages), #150 (`render_pages_parallel` stringifies a `collect_pages` failure)
