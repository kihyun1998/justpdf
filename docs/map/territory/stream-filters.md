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
- Decoded-size limit (read 2026-10-07): MuPDF `fz_read_best` refuses output past `max(200 × encoded length, 100 MB)`, or for an image `((W × nc × bpc + 7) >> 3) × H` floored at 100 MB, checked while reading ("compression bomb detected"). qpdf caps each filter only when the caller sets a limit (off by default; fuzz mode sets Flate to 200,000). pdf.js `flate_stream.js`/`decode_stream.js` have no cap.

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
- CCITT Group 4 decodes every line as white: `decode_2d_line` loops `while (a0 as usize) < columns` from `a0 = -1`, and `-1 as usize` is `usize::MAX`, so no bit is read. Measured 2026-10-07: `26 B0` (`/K -1 /Columns 8 /Rows 1`, three black pixels) decodes to eight `FF`. Mixed G3 2D lines share the function (inferred). `test_group4_all_white_line` asserts all white, so it cannot fail on this. Tracked: #246
- CCITT output is `/Columns × /Rows` whatever the data: undecoded lines are padded white, and a line that consumes no bits runs the loop to `/Rows` or the 100,000 cap. Measured 2026-10-07: 0 bytes with `/Columns 20000 /Rows 20000` → 400 MB; 1 byte `FF` with `/Columns 10000000` and no `/Rows` → allocation failure, process abort. Decided in #160: return only decoded lines and let `decode_image` complete the height. Tracked: #160.
- No filter bounds its output: Flate reads to the end, and a 62.6 KB file inflates an object stream to 64 MB (measured 2026-09-30). Decided in #161: MuPDF's rule, always on — a limit of `max(200 × encoded length, 100 MB)` (image: from `/Width`, `/Height`, components and bpc, at least `W × H` with CCITT), applied to every filter's output while decoding, with its own error kind. Tracked: #161
