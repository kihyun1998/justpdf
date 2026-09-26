# SVG 입력

## What it is
SVG를 자체 픽셀 래스터라이저로 그린 뒤 이미지로 PDF에 넣는다(벡터가 아니다).

## Governing decisions
- [ADR-0001](../../adr/0001-crates-split-by-dependency-layer.md) — render 없이 빌드된다.

## Design model
- 곡선을 직선으로 그리고("Simplified"), 텍스트는 "a small marker"가 되며, 퍼센트 길이는 건너뛴다.
- `to_pdf`는 72dpi로 렌더한 래스터를 `embed_rgb`로 이미지 XObject(Flate)로 넣고 `draw_image`로 페이지 전체에 그린다(`test_to_pdf_draws_the_raster_as_an_image_object`, #88). #88 전에는 `draw_inline_image`를 `cm` 없이 불러 이미지가 1pt×1pt로 놓였고, 픽셀에 공백+`EI`+공백이 있으면 이미지가 잘렸다.

## Code
- `justpdf-formats/src/svg/mod.rs` — `SvgDocument`, `render_element`, `to_pdf`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [문서 빌더](document-builder.md) — `embed_rgb`, `draw_image`.
- [렌더 이미지](render-images.md) — 결과 PDF가 justpdf 자신에게는 빈 페이지로 렌더된다.
- [SVG 렌더러](svg-renderer.md) — 이름만 같고 방향이 반대(PDF → SVG)인 별개 코드.

## Known holes / open
- 결과 PDF의 이미지 배치(위).
