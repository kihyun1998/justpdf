# Aggregate — Compress

**This note owns no detail.** How the techniques held in the single file `justpdf-core/src/writer/compress.rs` (4,667 lines, the largest `.rs` file in the repo — measured with `wc -l`, 2026-10-30) relate to each other and to the surfaces that expose them as a product.

| Concept | Note |
|---|---|
| What the four presets and ten knobs promise, and how each surface exposes them | [Compress presets](compress-presets.md) |
| `compress_pdf` stage order, `analyze_pdf`, refusing encryption, the disabled object-stream stage | [Compress pipeline](compress-pipeline.md) |
| JPEG re-encoding and CTM-based DPI downscaling of image XObjects | [Compress images](compress-images.md) |
| Grayscale images and colour-operator rewriting | [Compress grayscale](compress-grayscale.md) |
| TrueType subsetting that keeps glyph IDs | [Font subsetting](font-subsetting.md) |
| Flate for unfiltered streams, Flate recompression at the best level | [Compress stream recompression](compress-stream-recompression.md) |
| Merging streams whose dictionary and data are equal | [Compress dedup](compress-dedup.md) |
| Removing page resource entries the content does not name | [Compress unused resources](compress-unused-resources.md) |
| Removing metadata, structure and extras from the catalog and pages | [Compress stripping](compress-stripping.md) |
| The browser product and the CLI subcommand | [Compress WASM](compress-wasm.md), [CLI](cli.md) |

## Why they sit together
Every technique rewrites one `DocumentModifier`, in order, inside a single `compress_pdf` call. The techniques do not call each other, but **each stage reads what the stages before it wrote**: grayscale re-decodes the JPEG that image recompression just wrote, stream recompression re-encodes the FontFile2 that subsetting wrote, dedup runs after every rewrite, and GC runs after resource removal and stripping (`compress_pdf`, read at 88b2a08). So changing one technique's output (for example, the shape of a replaced image dictionary) means checking the later stages. The order itself is owned by [Compress pipeline](compress-pipeline.md).

Every stage that reads page content finds pages the same way and has its own copy of that code: it scans every object for a dictionary with `/Type /Page` (not a page-tree walk), and it reads `/Contents` as a stream reference or a direct array. The sites are `collect_image_display_sizes`, `rewrite_color_operators_to_gray`, `subset_embedded_fonts`, `fonts_reachable_outside_page_resources`, `remove_unused_resources` and `strip_non_essential` (inferred, read at 88b2a08). Fixing that in one stage leaves the others as they were — [Page content assembly](../invariant/page-content-assembly.md), Tracked: #138.

## Why it is split by concept, not by file
`compress.rs` is one module, but each technique has a different blast radius. Font subsetting reaches [Font loading](font-loading.md) and render glyph selection. Grayscale reaches [Content stream parsing](content-stream-parsing.md) and [Object syntax roundtrip](../invariant/object-syntax-roundtrip.md). The presets reach the browser product, the CLI and the external repo just-pdf-web. One note for all of it would make every edge read "check all of compression".
