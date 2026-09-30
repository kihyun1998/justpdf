# Deskew

## What it is
Estimates skew from a projection profile over a grayscale buffer and rotates the image.

## Governing decisions
- [ADR-0001](../../adr/0001-crates-split-by-dependency-layer.md)

## Design model
- Pure image processing; it does not touch PDF structure.

## Code
- `justpdf-special/src/deskew/mod.rs` — `detect_skew`, `deskew_image`, `rotate_image`, `SkewResult`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [OCR](ocr.md) — a candidate to connect to (none today).

## Known holes / open
- Nothing in the repo consumes it.
