# 기울기 보정 (deskew)

## What it is
회색조 버퍼에서 투영 프로파일로 기울기를 추정하고 이미지를 회전한다.

## Governing decisions
- [ADR-0001](../../adr/0001-crates-split-by-dependency-layer.md)

## Design model
- 순수 이미지 처리이며 PDF 구조와 닿지 않는다.

## Code
- `justpdf-special/src/deskew/mod.rs` — `detect_skew`, `deskew_image`, `rotate_image`, `SkewResult`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [OCR](ocr.md) — 연결될 후보(현재 없음).

## Known holes / open
- 저장소 안 소비처가 없다.
