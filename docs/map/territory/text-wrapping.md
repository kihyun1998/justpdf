# 텍스트 줄바꿈 (쓰기용 레이아웃)

## What it is
텍스트를 주어진 폭에 맞춰 줄바꿈·정렬하고 폭을 측정하는 쓰기용 유틸리티. 이름은 `text/`에 있지만 추출이 아니라 생성 쪽이다.

## Governing decisions
**None.**

## Design model
- `char_width`는 유니코드 스칼라를 문자 코드로 쓴다 — Identity가 아닌 인코딩 폰트에서는 폭이 틀린다(추론).

## Code
- `justpdf-core/src/text/text_layout.rs` — `layout_text`, `measure_text_width`, `char_width`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [문서 빌더](document-builder.md) — 줄바꿈을 쓸 쪽(현재 연결 없음).
- [포맷 변환 계약](format-document.md) — 각 입력 포맷이 자체 줄 나누기를 한다.

## Known holes / open
- `text/` 밖에 호출처가 없다.
