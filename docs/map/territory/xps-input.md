# XPS 입력

## What it is
XPS/OpenXPS 패키지의 FixedDocumentSequence에서 글리프 텍스트를 읽어 PDF로 바꾼다.

## Governing decisions
- [ADR-0001](../../adr/0001-crates-split-by-dependency-layer.md) — `xps` 기능은 render 없이 빌드된다.

## Design model
- `render_page`는 빈 흰 페이지를 돌려준다("v1: return white page… Full rendering… would go here").
- 페이지를 넘치는 텍스트는 버린다(`break`).
- 텍스트만 옮긴다 — 레이아웃·이미지·벡터는 옮기지 않는다.

## Code
- `justpdf-formats/src/xps/mod.rs` — `XpsDocument`, `to_pdf`, `render_page`

## Reference behaviour
**None.**

## Cross-cutting invariants
- [콘텐츠 텍스트 인코딩](../invariant/content-text-encoding.md)

## Blast radius
- [포맷 변환 계약](format-document.md), [문서 빌더](document-builder.md).

## Known holes / open
- 미리보기가 빈 페이지다.
