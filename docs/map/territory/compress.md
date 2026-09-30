# Aggregate — Compress

**This note owns no detail.** How the distinct techniques held in the single file `writer/compress.rs` (the largest file in the repo) relate to each other and to the surfaces that expose them as a product.

| Concept | Note |
|---|---|
| What the four presets and ten knobs promise, agreement across surfaces | [Compress presets](compress-presets.md) |
| `compress_pdf` stage order, `analyze_pdf`, refusing encryption, disabled stages | [Compress pipeline](compress-pipeline.md) |
| JPEG re-encoding, CTM-based DPI downscaling | [Compress images](compress-images.md) |
| Grayscale conversion and color operator rewriting | [Compress grayscale](compress-grayscale.md) |
| TrueType subsetting and Widths/CIDToGIDMap updates | [Font subsetting](font-subsetting.md) |
| Flate recompression, compressing uncompressed streams | [Compress stream recompression](compress-stream-recompression.md) |
| Stream deduplication (dict and data both equal) | [Compress dedup](compress-dedup.md) |
| Removing unused resources | [Compress unused resources](compress-unused-resources.md) |
| Stripping metadata, structure and auxiliary data | [Compress stripping](compress-stripping.md) |
| Browser product / CLI subcommand | [Compress WASM](compress-wasm.md), [CLI](cli.md) |

## Why they sit together
Every technique rewrites one `DocumentModifier` state, in order, inside a single `compress_pdf` call. The techniques do not call each other, but **each stage's output is the next stage's input**: dedup runs after image replacement, and GC runs after resource removal. So changing one technique's output shape (for example, the shape of a replaced image dict) means checking the later stages — the order is owned by [Compress pipeline](compress-pipeline.md).

## Why it is split by concept, not by file
`compress.rs` is one line in the module tree, but each technique has a different blast radius. Font subsetting reaches [Font loading](font-loading.md) and render glyph mapping, grayscale reaches [Content stream parsing](content-stream-parsing.md) and [Object syntax roundtrip](../invariant/object-syntax-roundtrip.md), and the presets reach the browser product, the CLI and an external web repo. One note for all of it would thicken every edge into "check all of compression".
