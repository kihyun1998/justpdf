# 폼 평탄화

## What it is
필드 위젯의 외관을 페이지 콘텐츠에 그려 넣고 `/AcroForm`을 제거해 폼을 정적 내용으로 만든다.

## Governing decisions
**None.**

## Design model
- `/AP /N`이 참조일 때만 쓴다. 체크박스의 상태 사전(`/Yes`/`/Off`)은 건너뛰는데 위젯은 소모된다.
- `q w 0 0 h llx lly cm /Fm{objnum} Do Q`를 쓰지만 **`/Fm{n}`을 페이지 `/Resources /XObject`에 등록하지 않는다** — 정의되지 않은 이름을 참조한다.
- `cm`이 이미 w×h인 BBox 위에 (w, h) 스케일을 한 번 더 건다(추론).

## Code
- `justpdf-core/src/form/flatten.rs` — `flatten_form`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [폼 외관](form-appearance.md) — 그려 넣는 외관의 좌표 규칙.
- [렌더 인터프리터](render-interpreter.md) — 평탄화 결과를 그리는 쪽(정의되지 않은 XObject 이름).
- [AcroForm](acroform.md), [문서 수정기](document-modifier.md).

## Known holes / open
- 인라인 테스트 모듈이 비어 있다. `test_flatten_form`은 `/AcroForm`이 사라졌는지만 확인한다.
- Tracked: #40 (폼 채우기·평탄화)
