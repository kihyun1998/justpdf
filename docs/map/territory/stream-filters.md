# Stream filters

## What it is
Applies a stream's `/Filter` (a name or an array) in order and passes each filter the `/DecodeParms` paired with it by index. Flate, LZW (+predictor), ASCII85, ASCIIHex, RunLength and CCITT are decoded here. DCT, JPX, JBIG2 and Crypt pass the raw bytes through unchanged — pixel decoding belongs to [Image decoding](image-decoding.md), and Crypt has already been decrypted at the document level.

## Governing decisions
**None.**

## Design model
- Three entry points: strict (`decode_stream`), tolerant (`decode_stream_tolerant` — skips unknown filters and partially recovers Flate only), and zero-copy (`decode_stream_cow`). **Product code uses only the strict path.** Tolerant and zero-copy are called only from tests and integration tests.
- The predictor runs only after Flate/LZW. The TIFF predictor accepts only 8 bits per component.
- LZW hardcodes early change to 1 (it does not read `/EarlyChange`).
- Passing JPX/JBIG2 through is the same deliberate split as the DCT branch: the image layer (53799fc) handles pixel decoding.
- CCITT output is one byte per pixel; by default black is 0x00 and white 0xFF (reversed with `/BlackIs1`).

## Code
- `justpdf-core/src/stream/mod.rs` — `decode_stream`, `decode_stream_tolerant`, `decode_single_tolerant`, `decode_stream_cow`, `is_passthrough_filter`, `decode_single`, `get_filters`, `get_decode_params`, `lzw_decode`
- `justpdf-core/src/stream/flate.rs` — `decode`, `decode_partial`
- `justpdf-core/src/stream/predictor.rs` — `apply`, `apply_tiff_predictor`, `apply_png_predictor`
- `justpdf-core/src/stream/ccitt.rs` — `CcittParams`, `decode`, `test_group4_all_white_line`
- `justpdf-core/src/stream/dct.rs` — `decode`, `jpeg_dimensions`
- `justpdf-core/src/stream/ascii85.rs` — `decode`
- `justpdf-core/src/stream/ascii_hex.rs` — `decode`
- `justpdf-core/src/stream/run_length.rs` — `decode`

## Reference behaviour
**None.** The code cites ITU-T T.4/T.6 tables for CCITT. Clause to compare against: ISO 32000-2 §7.4.

## Cross-cutting invariants
**None.**

## Blast radius
- [Image decoding](image-decoding.md) — receives the passed-through DCT/JPX/JBIG2 bytes. Changing the pass-through rule makes the image-side branches decode twice (the renderer's mask path already does — [Render images](render-images.md)).
- [Object streams](object-streams.md), [Xref](xref.md) — ObjStm and xref stream decoding.
- [Text extraction](text-extraction.md), [Render interpreter](render-interpreter.md), [Redaction](redaction.md), [Embedded files](embedded-files.md) — content and file stream decoding.
- [Compress stream recompression](compress-stream-recompression.md) — re-encodes after decoding.

## Known holes / open
- LZW and the TIFF predictor have no tests.
- Tolerant decoding (recovering broken streams) exists only as API, and no feature uses it. The renderer's tolerance is only swallowing errors (`unwrap_or_default`).
