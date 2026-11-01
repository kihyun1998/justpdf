# CBZ input

## What it is
Turns the images in a comic-book ZIP archive into PDF pages, one image per page, in natural filename order.

## Governing decisions
- [ADR-0001](../../adr/0001-crates-split-by-dependency-layer.md) — `cbz` (`zip`) builds without render (#2).

## Design model
Status: read from the source; the preview cap measured on #453.
- Entries: every non-directory whose name ends in `.jpg`, `.jpeg`, `.png`, `.gif`, `.bmp` or `.webp`, sorted by `natural_sort_key` (digit runs padded to 20 places, letters lower-cased). An entry whose dimensions cannot be read is dropped silently. The `image` dependency is built with `png` and `jpeg` only, so GIF, BMP and WebP pages are dropped this way (inferred from `Cargo.toml` and `image_dimensions`). No valid image is an error.
- Page size: the image's pixel size in points (1 px = 1 pt).
- `to_pdf` decodes each image to RGB with the `image` crate (alpha dropped), embeds it as a Flate DeviceRGB image XObject with `embed_rgb`, and draws it over the page with `draw_image` (`test_cbz_to_pdf_draws_each_image_as_an_image_object` checks the `q cm Do Q` operators, the `cm` operands and the decoded pixels, #88). Before #88 it called `draw_inline_image` without a `cm`, so the image sat at 1 pt × 1 pt — with a real page PNG the render came out all white (poppler, 2026-09-25).
- justpdf's renderer draws the result (measured 2026-10-30 at 88b2a08: a one-PNG CBZ converted and rendered back with `justpdf convert` shows the image).
- Preview: `render_page` sizes the output as the header's width and height × DPI / 72, rounded up, and refuses it with `FormatError::Format` before decoding when core `image::rgba_len` does (a side of 0, which a DPI of 0, a negative DPI or NaN gives, or more than `i32::MAX` bytes of RGBA). Otherwise it decodes the image and resizes it with Lanczos3 to that size. `page_text` is always empty. Decided on #453 (maintainer, 2026-11-01): #426's rule for SVG and XPS, an error rather than a downscaled preview, and a DPI giving a side of 0 an error rather than the 0 × 0 page it returned. The decode is bounded by the `image` crate's default `Limits` (`max_alloc` 512 MiB, `image` 0.25.10 `io/limits.rs`, `ImageReader::new`), but `resize_exact` allocates its output and an intermediate `Rgba32F` image with no limit of its own: `imageops::resize` samples vertically first, into source width × output height at 16 bytes per pixel (`imageops/sample.rs`, `resize` and `vertical_sample`, read in the local crate on #453). Before #453 a 1 × 1 PNG at 72 × 23171 dpi was resized to 23171² and returned after 7.9 s (release test); now it is refused in 0.01 s (*measured* 2026-11-01). Pinned by `a_preview_of_no_or_too_large_size_is_refused`; replacing the cap with a zero-side check alone, or dropping the zero-side check, failed it (*measured* 2026-11-01). Checking after the decode instead would fail no test: the result is the same and only the decode's cost differs (inferred). When it resizes, `render_page` also refuses a preview whose intermediate (source width × output height × 16 bytes) exceeds `i32::MAX` bytes, since the output cap alone allowed about four times that: a 50,000,000 × 1 gray PNG at 216 dpi has a 150,000,000 × 3 output inside the cap but a 2.4 GB intermediate, and was resized for 25.6 s before #453 (release test, *measured* 2026-11-01). Decided on #453 (maintainer, 2026-11-01). Pinned by `a_preview_whose_resize_pass_is_too_large_is_refused`, which failed with the check removed or its factor lowered to 4 (*measured*). Applying the check at 72 dpi too, where nothing is resized, would fail no test: pinning it takes an image of more than 1.34 × 10^8 pixels, about 1 GB to decode and convert in a test (inferred).
- Input bound: none. Every image entry is read whole into memory at `from_bytes`, and each is fully decoded again by `to_pdf` and by every preview; nothing limits entry count, compressed or decoded size (inferred).

## Code
- `justpdf-formats/src/cbz/mod.rs` — `CbzDocument`, `CbzDocument::open`, `CbzDocument::from_bytes`, `is_image_extension`, `image_dimensions`, `natural_sort_key`, `impl FormatDocument for CbzDocument` (`render_page`, `render_page_png`, `to_pdf`)

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [Document builder](document-builder.md) — assumes `embed_rgb` takes three bytes per pixel, rows from the top, and that `draw_image` maps the unit square to the page box.
- [Render images](render-images.md) — a producer of Flate DeviceRGB image XObjects that justpdf's renderer must decode exactly once (see [SVG input](svg-input.md) for the blank-page history before #229).
- [Format document](format-document.md) — the contract.
- [Image decoding](image-decoding.md) — assumes `rgba_len` is the RGBA byte length with its `i32::MAX` cap; `render_page` refuses what it refuses.

## Known holes / open
- GIF, BMP and WebP entries are accepted by name and then dropped without a message (inferred).
- No bound on entry count or size (Design model).
