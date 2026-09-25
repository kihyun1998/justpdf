# CBZ 입력

## What it is
만화책 ZIP 아카이브의 이미지를 자연 정렬해 페이지로 만든다.

## Governing decisions
- [ADR-0001](../../adr/0001-crates-split-by-dependency-layer.md)

## Design model
- `image` 크레이트로 RGB로 디코드한 뒤 `embed_rgb`로 이미지 XObject(Flate)를 만들고 `draw_image`로 페이지(이미지 픽셀 크기) 전체에 그린다(`test_cbz_to_pdf_draws_each_image_as_an_image_object`, #88). #88 전에는 `draw_inline_image`를 `cm` 없이 불러 이미지가 1pt×1pt로 놓였다 — 실제 페이지 PNG를 넣으면 렌더 결과가 전부 흰색이었다(poppler, 2026-09-25).

## Code
- `justpdf-formats/src/cbz/mod.rs` — `CbzDocument`, `to_pdf`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [문서 빌더](document-builder.md) — `embed_rgb`, `draw_image`. [렌더 이미지](render-images.md) — 이미지 XObject 경로.

## Known holes / open
- `test_cbz_to_pdf`는 `%PDF` 시작만 확인한다.
