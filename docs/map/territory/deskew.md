# Deskew

## What it is
Estimates the skew of a grayscale image with a projection profile and rotates the image back. Pure image processing: it does not touch PDF structure, and nothing in the repo calls it.

## Governing decisions
- [ADR-0001](../../adr/0001-crates-split-by-dependency-layer.md) — places it in `justpdf-special` (`deskew` feature, no dependencies).

## Design model
Status: read from the source only.
- `detect_skew` tries angles from −5° to +5° in 0.1° steps, projects every pixel's darkness (255 − value) onto rotated rows, and keeps the angle with the largest variance of row sums. Skew outside ±5° is not found.
- `confidence` is `best / (best + 1)`, which is close to 1 for any image with variance, while the comment says "ratio of best variance to mean of all variances" (inferred).
- `deskew_image` returns the input unchanged below 0.05° and otherwise calls the private `rotate_image` with the negated angle: bilinear, same size, white outside.
- Input bound: the buffer is the caller's. The length check computes `width * height` in `u32`, which overflows for large images (a panic in a debug build — inferred, same pattern as [SVG input](svg-input.md)). Cost is about 100 passes over every pixel (one per angle).

## Code
- `justpdf-special/src/deskew/mod.rs` — `SkewResult`, `detect_skew`, `deskew_image`, private `rotate_image`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
**None.** No code in the repo calls this module (`git grep -n "deskew::" -- '*.rs' ':!justpdf-special'` finds nothing, 2026-10-30); [OCR](ocr.md) does not deskew before recognising.

## Known holes / open
- Nothing in the repo consumes it.
- `confidence` does not measure what its comment says (inferred).
- `width * height` overflows `u32` for large images (inferred).
