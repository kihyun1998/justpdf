# Compress pipeline

## What it is
`compress_pdf` parses the input, refuses an encrypted file, copies the document into a `DocumentModifier`, runs the enabled techniques in a fixed order, runs GC and writes the result. `analyze_pdf` reports the page count, image count, image bytes and whether the file is encrypted, without compressing. This note owns the stage order and which knob gates each stage. Each technique is owned by a member note of the [Compress](compress.md) aggregate.

## Governing decisions
**None.** `dev/pdf-compress-wasm-design.md` §7 records two choices this note relies on: an encrypted PDF returns an error, and `clean_objects` renumbering invalidates `catalog_ref`, so only GC runs. It is a design doc, not a decision record; the code follows both (re-verified by reading `compress_pdf`, 2026-10-30).

## Design model
- **Stage order** (`compress_pdf`, read at 88b2a08), each gated by its knob:
  1. image display sizes (`collect_image_display_sizes`), when `max_image_dpi` is set;
  2. image recompression (`recompress_images`), when `jpeg_quality` or `max_image_dpi` is set;
  3. grayscale (`convert_images_to_grayscale`, then `rewrite_color_operators_to_gray`), when `grayscale`;
  4. font subsetting (`subset_embedded_fonts`), when `font_subsetting`;
  5. Flate for unfiltered streams (`compress_raw_streams`), then Flate recompression (`recompress_flate_streams`), when `compress_streams`;
  6. unused resources (`remove_unused_resources`), when `remove_unused_resources`;
  7. stripping (`strip_non_essential`), when `strip_metadata` or `strip_extras`;
  8. stream dedup (`dedup_streams`), when `structural`;
  9. GC (`DocumentModifier::garbage_collect`), when `structural`;
  10. `DocumentModifier::build`.
- Each stage's output is the next stage's input ([Compress](compress.md#why-they-sit-together)). When adding or moving a stage, check the stages after it.
- **Encrypted input is refused** ("cannot compress encrypted PDF — decrypt first"), even when the document authenticated with an empty user password. The error is `JustPdfError::StreamDecode` with filter `"compress"`, although `JustPdfError::EncryptedDocument` exists (`DocumentModifier::from_document` uses it). So a caller cannot tell this refusal from a decode failure by variant (inferred, read at 88b2a08).
- **Only GC runs, not `clean_objects`**: renumbering would invalidate the modifier's catalog reference (comment in `compress_pdf`, step 4). GC keeps every object number.
- **The object-stream stage is disabled**: "xref stream implementation has compatibility issues with some PDF viewers" (comment in `compress_pdf`, step 5). `pack_into_object_streams` is `#[allow(dead_code)]`, and only the `test_pack_object_streams_*` tests reach the packing. The writing side of [Object streams](object-streams.md) stops here.
- **No whole-file size check.** Each technique replaces an object only when its new bytes are smaller, but `compress_pdf` returns the rebuilt file even when it is larger than the input. Measured 2026-10-30: a hand-built two-page file of 0.94 KB came out at 0.95 KB with `--preset low` (`justpdf compress`).
- **`analyze_pdf` counts image streams in the object table**, not images drawn: every stream with `/Subtype /Image` counts, an SMask or an unreferenced image included, and duplicates count once each. So dedup and GC lower the count on a correct output. Measured 2026-10-30 with `justpdf compress --analyze` before and after `justpdf compress`: `testpdf.pdf` (repo root) goes from 2,489 to 2,469 images at every preset, the 20 "Duplicates removed".
- **Input bounds.** Nothing bounds the input's size or object count. `compress_pdf` copies the input (`data.to_vec()`) into the parsed document, `DocumentModifier::from_document` copies every object, and each stage clones the streams it collects before rewriting them, so peak memory is a multiple of the input. Decoded stream sizes are bounded only by the decoders, and Flate has no limit (Tracked: #161).

## Code
- `justpdf-core/src/writer/compress.rs` — `compress_pdf`, `analyze_pdf`, `AnalyzeResult`, `CompressStats`, `is_image_xobject`, `pack_into_object_streams`, `find_object_dict`, `get_stream_raw_data`, `get_stream_decoded_data`, `test_analyze_text_pdf`, `test_analyze_pdf_with_image`, `test_compress_roundtrip_valid`, `test_pack_object_streams_basic`, `test_pack_object_streams_metadata`, `test_pack_object_streams_skips_streams`
- `justpdf-core/tests/image_decode.rs` — `compression_recompresses_a_flate_wrapped_jpeg` (the one compress_pdf test in core outside compress.rs)
- `justpdf-render/tests/render_image_colour.rs` — `compress`, `compressing_a_sixteen_bit_image_does_not_panic`, `a_compressed_indexed_image_keeps_its_colours`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.** The page-enumeration fact every content-reading stage shares is stated in the [Compress](compress.md#why-they-sit-together) aggregate.

## Blast radius
- Every member technique — [Compress images](compress-images.md), [Compress grayscale](compress-grayscale.md), [Font subsetting](font-subsetting.md), [Compress stream recompression](compress-stream-recompression.md), [Compress dedup](compress-dedup.md), [Compress unused resources](compress-unused-resources.md), [Compress stripping](compress-stripping.md) — each assumes what the stages before it left in the modifier.
- [Compress presets](compress-presets.md) — each knob's meaning is the stage it gates here.
- [Document modifier](document-modifier.md) — the pipeline assumes `from_document` copies every object and `garbage_collect` keeps object numbers; a modifier that renumbers breaks the catalog reference the stages share.
- [Document access](document-access.md) — `is_encrypted` decides the refusal, and `analyze_pdf` trusts `page_count` (an error reads as 0 pages).
- [Compress WASM](compress-wasm.md), [CLI](cli.md) — both copy the `CompressStats` and `AnalyzeResult` fields by hand (`stats_to_result`, `print_compress_breakdown`, `cmd_compress_analyze`). A new or renamed field is missing on a surface until it is copied there.

## Known holes / open
- Core refuses encrypted input. The CLI's `--password` path authenticates, re-serializes through `DocumentModifier::from_document(…).build()` and passes those bytes to `compress_pdf`, so the reported original size is the decrypted, re-serialized size, not the file's (inferred, `cmd_compress`).
- An object `DocumentModifier::from_document` cannot resolve is dropped without an error, so compress can write a file missing it and report success. Tracked: #97.
- The output can be larger than the input, and the surfaces show it as a negative reduction (measured above). Open question for the maintainer: should `compress_pdf` return the input when it does not shrink?
- just-pdf-web's sanity check compares `analyze()` image counts before and after compression (`compareSanity`, `src/features/compress/lib/sanity.ts`, d4410b8, 2026-10-06). Since dedup and GC lower `analyze_pdf`'s count on a correct output (measured above), that check reports a false failure on files with duplicate image streams. Open question for the maintainer: should `analyze_pdf` count drawn images, or should the web compare something else?
- Unit tests live in `compress.rs`; outside it, core has one `compress_pdf` test (`compression_recompresses_a_flate_wrapped_jpeg`) and the render crate two. 128 tests under `writer::compress`, `font::subset` and `writer::clean` pass at 88b2a08 (measured, `cargo test -p justpdf-core --lib`, 2026-10-30).
