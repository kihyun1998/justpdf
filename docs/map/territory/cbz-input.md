# CBZ 입력

## What it is
만화책 ZIP 아카이브의 이미지를 자연 정렬해 페이지로 만든다.

## Governing decisions
- [ADR-0001](../../adr/0001-crates-split-by-dependency-layer.md)

## Design model
- `image` 크레이트로 디코드한 뒤 `draw_inline_image`를 `cm` 없이 부른다 — [SVG 입력](svg-input.md)과 같은 배치 문제(추론).

## Code
- `justpdf-formats/src/cbz/mod.rs` — `CbzDocument`, `to_pdf`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [문서 빌더](document-builder.md), [렌더 이미지](render-images.md) — 인라인 이미지 경로.

## Known holes / open
- `test_cbz_to_pdf`는 `%PDF` 시작만 확인한다.
- Tracked: #43 (인라인 이미지 cm 누락)
