# Stream filters

## What it is
Applies a stream's `/Filter` (a name or an array) in order and passes each filter the `/DecodeParms` entry at the same index. Flate, LZW (each with an optional predictor), ASCII85, ASCIIHex, RunLength and CCITT are decoded here. DCT, JPX, JBIG2 and Crypt pass their bytes through unchanged: pixel decoding belongs to [Image decoding](image-decoding.md), and Crypt has already been decrypted at the document level.

## Governing decisions
**None.** The decoded-size limit and the CCITT output rule were decided on issues (#161, #160), not in a record.

## Design model
Rules read from the code (*inferred* unless marked).
- **Three entry points.** Strict `decode_stream` fails on the first filter error. Tolerant `decode_stream_tolerant` retries Flate with partial decoding (`flate::decode_partial`) and skips any other filter that fails. Zero-copy `decode_stream_cow` borrows the input when there is no filter or every filter is a pass-through, else calls `decode_stream`. Product code uses the strict path everywhere and the zero-copy path in `image::decode_image` (the DCT, JPX and JBIG2 branches); the tolerant path is called only by tests (`rg -n 'decode_stream_tolerant|decode_stream_cow' -g '*.rs'`, 2026-10-30).
- A `/Filter` array entry that is not a name is dropped (`get_filters`, `filter_map`), which shifts the index pairing of every later filter with `/DecodeParms`. A `/DecodeParms` entry that is not a dictionary keeps its slot as `None` (`get_decode_params`). Neither follows a reference.
- Flate tries zlib, then raw deflate. A predictor runs only after Flate or LZW (`predictor::apply`): 1 none, 2 TIFF (8 bits per component only), 10–15 PNG; any other value is an error.
- LZW hard-codes early change 1 and ignores `/EarlyChange`. It runs until code 257: past the end of the data `read_bits` returns zeros, which are valid codes, so input without an end-of-data code decodes forever. *Measured* 2026-10-30, release `justpdf text` on a page whose content stream is `/Filter /LZWDecode` with the single byte `00`: still running after 5 s at 2.3 GB resident, killed by the timer.
- Passing JPX and JBIG2 through is the same split as the DCT branch: the image layer handles pixel decoding (commit 53799fc).
- CCITT output is one byte per pixel; by default black is `0x00` and white `0xFF`, reversed with `/BlackIs1`. Without `/Rows` it decodes up to 100,000 rows.
- **Input bound:** no filter bounds its output. Flate reads to the end: a 62.6 KB file inflated an object stream to 64 MB (*measured* 2026-09-30, #161). LZW without an end code does not stop (above). The PNG predictor allocates a row of `/Columns × /Colors × /BitsPerComponent` bits before reading data: `/Columns -1` wraps to a 2^61-byte allocation and aborts the process (*measured* 2026-10-30, release `justpdf text`, exit 134, "memory allocation of 2305843009213693951 bytes failed"). CCITT output is sized by `/Columns × /Rows`, not by the data (#160).

## Code
- `justpdf-core/src/stream/mod.rs` — `decode_stream`, `decode_stream_tolerant`, `decode_single_tolerant`, `decode_stream_cow`, `is_passthrough_filter`, `decode_single`, `get_filters`, `get_decode_params`, `lzw_decode`, `read_bits`
- `justpdf-core/src/stream/flate.rs` — `decode`, `decode_zlib`, `decode_raw_deflate`, `decode_partial`
- `justpdf-core/src/stream/predictor.rs` — `apply`, `apply_tiff_predictor`, `apply_png_predictor`, `paeth_predictor`
- `justpdf-core/src/stream/ccitt.rs` — `CcittParams`, `CcittParams::from_dict`, `decode`, `decode_group3_1d`, `decode_2d_line`, `test_group4_all_white_line`
- `justpdf-core/src/stream/dct.rs` — `decode`, `jpeg_dimensions`, `DecodedImage`
- `justpdf-core/src/stream/ascii85.rs` — `decode`
- `justpdf-core/src/stream/ascii_hex.rs` — `decode`
- `justpdf-core/src/stream/run_length.rs` — `decode`

## Reference behaviour
- Decoded-size limit (read 2026-10-07, #161): MuPDF `fz_read_best` refuses output past `max(200 × encoded length, 100 MB)`, or for an image `((W × nc × bpc + 7) >> 3) × H` floored at 100 MB, checked while reading ("compression bomb detected"). qpdf caps each filter only when the caller sets a limit (off by default; fuzz mode sets Flate to 200,000). pdf.js `flate_stream.js` / `decode_stream.js` have no cap.
- The code cites ITU-T T.4/T.6 tables for CCITT; not compared. ISO 32000-2 §7.4 not compared.

## Cross-cutting invariants
**None.**

## Blast radius
- [Image decoding](image-decoding.md) — the other half of the pass-through rule: it expects DCT/JPX/JBIG2 bytes untouched and every earlier filter applied. Making a codec decode here would make the image side decode twice (the renderer's mask path already decodes twice — [Render images](render-images.md)).
- [Object streams](object-streams.md), [Xref](xref.md) — ObjStm and xref-stream data is decoded here; their tables are bounded only by what this layer returns, so a size limit here (#161) is their bound.
- [Document access](document-access.md) — the font-program and object-stream caches keep decoded output for the document's lifetime; unbounded output here is unbounded memory there.
- [PDF functions](pdf-functions.md), [Color spaces](color-spaces.md) — decode function and Indexed lookup streams through `PdfDocument::decode_stream`.
- [Text extraction](text-extraction.md), [Render interpreter](render-interpreter.md), [Redaction](redaction.md), [Embedded files](embedded-files.md) — content and file streams; a strict-path error here drops the whole stream there.
- [Compress stream recompression](compress-stream-recompression.md) — re-encodes after decoding; a filter decoded wrongly here is written back wrongly.

## Known holes / open
- LZW and the TIFF predictor have no tests (`rg -n 'lzw|LZW' justpdf-core/src justpdf-core/tests` finds only the implementation, 2026-10-30).
- Tolerant decoding exists only as API; no feature uses it. The renderer's tolerance is swallowing errors (`unwrap_or_default`).
- LZW without an end-of-data code decodes without end (measured above). Tracked: #412.; the #161 limit would stop it.
- The PNG predictor sizes a row allocation from `/DecodeParms` before reading data (measured above). Tracked: #413.
- CCITT Group 4 decodes every line as white: `decode_2d_line` loops `while (a0 as usize) < columns` from `a0 = -1`, and `-1 as usize` is `usize::MAX`, so no bit is read. *Measured* 2026-10-07: `26 B0` (`/K -1 /Columns 8 /Rows 1`, three black pixels) decodes to eight `FF`. Mixed G3 2D lines share the function (inferred). `test_group4_all_white_line` asserts all white, so it cannot fail on this. Tracked: #246
- CCITT output is `/Columns × /Rows` whatever the data: undecoded lines are padded white, and a line that consumes no bits runs the loop to `/Rows` or the 100,000 cap. *Measured* 2026-10-07: 0 bytes with `/Columns 20000 /Rows 20000` → 400 MB; 1 byte `FF` with `/Columns 10000000` and no `/Rows` → allocation failure, process abort. Decided in #160: return only decoded lines and let `decode_image` complete the height. Tracked: #160
- No filter bounds its output. Decided in #161: MuPDF's rule, always on — a limit of `max(200 × encoded length, 100 MB)` (image: from `/Width`, `/Height`, components and bpc, at least `W × H` with CCITT), applied to every filter's output while decoding, with its own error kind. Tracked: #161
