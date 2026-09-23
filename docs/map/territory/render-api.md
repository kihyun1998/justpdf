# 렌더 API (진입점·크기·병렬)

## What it is
렌더 크레이트의 공개 진입점: 페이지 → PNG/JPEG/RGBA/SVG, 파일 저장, DPI·배경·형식 옵션, 페이지 변환 계산, `parallel` 기능의 페이지 단위 병렬 렌더.

## Governing decisions
**None.** [ADR-0001](../../adr/0001-crates-split-by-dependency-layer.md)이 이 크레이트를 선택적 레이어로 둔다.

## Design model
- `render_page_to_pixmap`이 `render_page_info`의 크기 계산·검증(16384px 상한)을 중복한다.
- `compute_page_transform`은 회전을 처리하지만 픽셀 폭·높이는 회전 전 상자에서 나온다 — 90/270° 페이지의 크기가 뒤바뀔 수 있다(추론; 회전 테스트 없음).
- `parallel`은 rayon으로 `render_page_info`를 페이지마다 돌린다. 타일 분할 아님.

## Code
- `justpdf-render/src/render.rs` — `render_page`, `render_page_info`, `render_page_to_file`, `render_page_to_pixmap`, `render_page_to_svg`, `compute_page_transform`, `RenderOptions`, `OutputFormat`, `RenderedPixmap`, `render_pages_parallel`, `render_all_pages_parallel`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- 소비처 — [파사드](facade.md), [CLI](cli.md), [plaintext](plaintext-input.md)·[mobi](mobi-input.md)·[fb2](fb2-input.md) 미리보기, [epub](epub-input.md)·[office](office-input.md)(plaintext 경유), [OCR](ocr.md), [언어 바인딩](language-bindings.md). 공개 시그니처를 바꾸면 전부 닿는다. 소비처는 명령으로 확인: `rg -l 'justpdf_render::' --glob '*.rs' --glob '!target' .`.
- [렌더 인터프리터](render-interpreter.md), [SVG 렌더러](svg-renderer.md) — 내부.
- [페이지 트리](page-tree.md) — `PageInfo`.

## Known holes / open
- 렌더 테스트(`tests/render_test.rs`)는 저장소에 추적된 `testpdf.pdf`가 없으면 조용히 통과하는 조건부 탈출을 가진다(현재는 파일이 있어 발동하지 않음). 회전 0만 테스트한다.
- Tracked: #54 (회전 페이지 렌더 크기)
