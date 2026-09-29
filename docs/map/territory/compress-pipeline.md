# Compress pipeline

## What it is
`compress_pdf` opens the document, unpacks it into a `DocumentModifier`, applies the techniques in a fixed order, runs GC and writes it back. `analyze_pdf` reports the page count, image count, image bytes and whether the file is encrypted, before compression. This note covers the stage order and how each stage is wired to the knob that enables it — each individual technique is owned by a member note of the [Compress](compress.md) aggregate.

## Governing decisions
**None.**

## Design model
- **Encrypted input is refused** ("decrypt first"). The error borrows the `StreamDecode` variant (inferred: an error of the wrong category).
- **Only GC runs, not `clean_objects`**: renumbering would invalidate catalog_ref (stage 4 comment).
- **The object stream stage (5) is disabled**: "xref stream implementation has compatibility issues with some PDF viewers". The writing side of [Object streams](object-streams.md) stops here.
- The stage order is owned by the body of `compress_pdf`. When adding or moving a stage, keep in mind that each stage's output is the next stage's input ([Compress](compress.md#why-they-sit-together)).

## Code
- `justpdf-core/src/writer/compress.rs` — `compress_pdf`, `analyze_pdf`, `AnalyzeResult`, `is_image_xobject`, `pack_into_object_streams`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- Every member technique — [Compress images](compress-images.md), [Compress grayscale](compress-grayscale.md), [Font subsetting](font-subsetting.md), [Compress stream recompression](compress-stream-recompression.md), [Compress dedup](compress-dedup.md), [Compress unused resources](compress-unused-resources.md), [Compress stripping](compress-stripping.md).
- [Compress presets](compress-presets.md) — knob → stage wiring.
- [Document modifier](document-modifier.md) — `from_document`/`garbage_collect`/`build`.
- [Document access](document-access.md) — the `is_encrypted` check.
- [Compress wasm](compress-wasm.md), [CLI](cli.md) — expose the `CompressStats` and `AnalyzeResult` fields as they are. Adding or removing a field reaches the getters/output of both surfaces.

## Known holes / open
- core still refuses encrypted input. CLI `--password` authenticates, then passes bytes re-serialized through `DocumentModifier`, so the reported original size is the decrypted, re-serialized size, not the file size (inferred).
- `justpdf-core/tests/` has no `compress_pdf` integration test (unit tests live only inside `compress.rs`).
