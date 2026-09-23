# 렌더 — 주석 그리기

## What it is
페이지 콘텐츠를 그린 뒤 각 주석의 `/AP /N` 외관 스트림을 그린다.

## Governing decisions
**None.**

## Design model
- Hidden(0x02)·NoView(0x20) 플래그를 존중한다.
- `/N`이 상태 사전이면 건너뛴다. `/AS`를 보지 않는다.
- `render_form_xobject`를 **Rect 매핑과 BBox 클립 없이** 부른다(추론: 외관이 잘못된 위치에 그려짐). 스펙은 BBox를 Matrix로 변환해 Rect에 맞추는 변환을 요구한다.
- 주석 `/OC`를 확인하지 않는다.

## Code
- `justpdf-render/src/interpreter.rs` — `render_annotations`, `render_form_xobject`

## Reference behaviour
**None.** 비교 대상 조항: ISO 32000-2 §12.5.5(외관 스트림 → 주석 사각형 매핑 알고리즘).

## Cross-cutting invariants
**None.**

## Blast radius
- [주석 외관](annotation-appearance.md) — 여기서 그리는 XObject를 만드는 쪽. 좌표 규칙 양쪽을 함께 고친다.
- [주석](annotations.md) — 입력.
- [optional content](optional-content.md) — 주석 `/OC`.
- [투명도](render-transparency.md) — Form XObject 경로 공유.

## Known holes / open
- 주석 렌더 테스트가 없다.
- Tracked: #41 (주석 외관 좌표)
