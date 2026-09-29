# Compress stream recompression

## What it is
Two stages: compress streams with no filter and more than 128 bytes with Flate (`compress_raw_streams`), and recompress streams that already carry a single FlateDecode at the highest level (`recompress_flate_streams`). A stream is replaced only when the result is strictly smaller.

## Governing decisions
**None.**

## Design model
- Streams with DecodeParms (a predictor and so on) and image XObjects are not recompressed.
- Only when the filter chain is a single Flate (`is_single_flate`).
- **Test input trap**: content streams `PageBuilder` produces are already at the zlib default level (6), and below a few tens of thousands of bytes a stream is the same size at the highest level (9) → the "replace only when strictly smaller" rule applies and 0 streams are recompressed. Measured 2026-09-23, one stream: 1,000 chars 347↔347, 5,000 chars 1,227↔1,227, 50,000 chars 10,180↔10,007. Even saved at level 1 (`fast`), short streams ("Test page N", 100 chars) give 0, and it takes ~1000 chars to get 5/5. So the tests that verify recompression (the four Phase B tests) use `create_fast_flate_text_pdf` (~1000 chars + level 1) as input, and first assert that recompression actually happened (the stream count matches).
- "Equal size is not replaced" is pinned by the second pass of `test_recompress_flate_already_best_no_growth` (input already at best → 0).

## Code
- `justpdf-core/src/writer/compress.rs` — `compress_raw_streams`, `recompress_flate_streams`, `is_single_flate`
- `justpdf-core/src/writer/encode.rs` — `encode_flate`, `encode_flate_best`
- `justpdf-core/src/writer/compress.rs` (tests) — `create_fast_flate_text_pdf`, `test_recompress_flate_output_not_larger`, `test_recompress_flate_roundtrip_identical`, `test_recompress_flate_already_best_no_growth`, `test_recompress_flate_text_pdf_improvement`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [Stream filters](stream-filters.md) — the decode side. If the Flate decoder's behaviour changes, verifying the recompression result changes.
- [Compress dedup](compress-dedup.md) — streams whose bytes become equal after recompression become dedup candidates.
- [Compress presets](compress-presets.md) — the `compress_streams` knob.

## Known holes / open
**None.**
