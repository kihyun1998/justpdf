# Bit widths

## The fact
A bit width read from a PDF dictionary is bounded before any shift or bit read uses it. Unchecked, a width of 0 or above the reader's range overflows a shift (a debug panic), underflows a byte offset, or makes a bit reader return a value without consuming input, so a loop reading until the data runs out never ends. Each site settles an out-of-range value its own way, after the prior art it follows:
- **Mesh shadings** (`/BitsPerCoordinate`, `/BitsPerComponent`, `/BitsPerFlag`): a value outside ISO 32000's set reads as 8, as MuPDF does.
- **Sampled functions** (`/BitsPerSample`): a value outside 1, 2, 4, 8, 12, 16, 24, 32 fails the function.
- **The predictor** (`/DecodeParms /BitsPerComponent`): a value below 1 reads as 8, and another value outside 1, 2, 4, 8, 16 is `StreamDecode`.
- **Images** (`/BitsPerComponent`): absent or 0 reads as 8, an image mask is 1, and outside 1..=16 `decode_image` refuses the image. JPX is exempt because its codec sets the depth. Any depth from 1 to 16 is read, including the ones the spec does not list (3, 12), as MuPDF and pdf.js do.

## Why it is cross-cutting
Four sites read a width and none calls another (*inferred*, read at `bbf06c0` plus #459):
- `justpdf-render/src/shading.rs` — `mesh_bits`
- `justpdf-core/src/function.rs` — the `BitsPerSample` match in the sampled-function parser
- `justpdf-core/src/stream/predictor.rs` — the `/BitsPerComponent` check
- `justpdf-core/src/image/mod.rs` — `image_info` and the check in `decode_image`, with `read_sample` reading any depth

## Territories it holds in
- [Render shading](../territory/render-shading.md) — `mesh_bits`.
- [PDF functions](../territory/pdf-functions.md) — `BitsPerSample`.
- [Stream filters](../territory/stream-filters.md) — the predictor's `/BitsPerComponent`.
- [Image decoding](../territory/image-decoding.md) — `/BitsPerComponent`, `read_sample`.

## What a violation looks like
Each of these was *measured* on 2026-11-01:
- A mesh shading with `/BitsPerCoordinate 64` panicked with `attempt to shift left with overflow`.
- A type-4 mesh with all three widths 0 grew to 4.8 GB in 30 s in release without finishing (#433).
- An image with `/BitsPerComponent 12` panicked with `attempt to subtract with overflow` in `read_sample`.
- `/BitsPerComponent 32` was clamped to 16 and drew two pixels that should be white and black as both white (#459).

## Discovery history
- The predictor's check came with #413 (a `/Columns -1` abort).
- The mesh widths were #433, found while working #413.
- The image depth was #459, found while working #433. Its survey listed the four sites above. The other three already checked, so the fact held everywhere once #459 landed. That is when the note was written (2026-11-01).
- The direction at each site was the maintainer's call where one was asked: #433 chose 8 over skipping the shading, and #459 chose MuPDF's 1..=16 over the spec's set.

## Where it will recur
**A function that reads an integer from a dictionary and uses it as a number of bits.** Command: `rg -n 'BitsPer|b"BPC"' justpdf-*/src`. Check the value against the allowed set or range before it reaches a shift, `1 << bits` or a bit reader, and test 0, a negative value and one above the range. Inline images' `/BPC` is not read today, because `render_inline_image` draws nothing. It becomes a fifth site when inline images are drawn (inferred).
