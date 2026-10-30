# Compress stream recompression

## What it is
Two stages behind the `compress_streams` knob. `compress_raw_streams` Flate-encodes streams that have no `/Filter` and more than 128 bytes. `recompress_flate_streams` decodes streams whose filter is a single FlateDecode and re-encodes them at the best level. Either replaces a stream only when the result is strictly smaller.

## Governing decisions
**None.**

## Design model
- **Raw streams** (`compress_raw_streams`): any stream with no `/Filter` and `data.len() > 128`, image XObjects included (an image recompression skipped stays raw until here). It encodes at the default level (`encode_flate`) and sets `/Filter /FlateDecode`. It does not count what it replaced: no `CompressStats` field moves (read at 88b2a08).
- **Flate streams** (`recompress_flate_streams`): only when `/Filter` is the *name* `FlateDecode` or `Fl` (`is_single_flate`) — a one-element array `[/FlateDecode]` is not recompressed — and there is no `/DecodeParms` (a predictor and so on). Image XObjects are skipped. It re-encodes with `encode_flate_best` and counts each replacement in `streams_recompressed`.
- The raw stage runs first, so every stream it just encoded at the default level is decoded and re-encoded at the best level by the second stage (inferred, `compress_pdf` order).
- "Equal size is not replaced" is pinned by the second pass of `test_recompress_flate_already_best_no_growth` (input already at best → 0).
- **Test input trap** (measured 2026-09-23, one stream): content streams `PageBuilder` produces are already at the zlib default level (6), and below a few tens of thousands of bytes a stream is the same size at level 9, so "strictly smaller" replaces 0 streams — 1,000 chars 347 ↔ 347, 5,000 chars 1,227 ↔ 1,227, 50,000 chars 10,180 ↔ 10,007. Even saved at level 1 (`fast`), short streams ("Test page N", 100 chars) give 0, and it takes about 1,000 chars to get 5 of 5. So the tests that check recompression use `create_fast_flate_text_pdf` (about 1,000 chars at level 1) as input and first assert that recompression happened (the stream count matches).
- **Input bounds**: every stream in the file is a candidate, and a Flate stream is decoded in full before re-encoding; nothing bounds its decoded size (Tracked: #161).

## Code
- `justpdf-core/src/writer/compress.rs` — `compress_raw_streams`, `recompress_flate_streams`, `is_single_flate`, `create_fast_flate_text_pdf`, `test_recompress_flate_output_not_larger`, `test_recompress_flate_roundtrip_identical`, `test_recompress_flate_already_best_no_growth`, `test_recompress_flate_text_pdf_improvement`
- `justpdf-core/src/writer/encode.rs` — `encode_flate`, `encode_flate_best`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [Stream filters](stream-filters.md) — the second stage trusts `decode_stream` to return the exact bytes the stream holds; a decoder that drops or adds bytes would be written back as the stream's content.
- [Compress dedup](compress-dedup.md) — runs later and compares encoded bytes, so two streams equal before this stage stay equal only if both are re-encoded the same way.
- [Font subsetting](font-subsetting.md) — runs earlier and writes each subset FontFile2 already at the best level, so the second stage decodes it again and normally keeps it.
- [Compress presets](compress-presets.md) — the `compress_streams` knob gates both stages, and every preset turns it on.

## Known holes / open
- `compress_raw_streams` reports nothing in `CompressStats`, so the surfaces cannot show how many streams it encoded (inferred).
- A single Flate filter written as a one-element array is not recompressed (inferred, `is_single_flate`).
