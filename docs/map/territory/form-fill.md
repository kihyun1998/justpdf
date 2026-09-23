# 폼 채우기

## What it is
필드에 값을 설정한다(`/V`, 버튼은 `/AS`). 읽기 전용 필드는 거부한다.

## Governing decisions
**None.**

## Design model
- **외관을 다시 만들지 않고 `NeedAppearances`도 세우지 않는다**: 값을 바꿔도 뷰어는 옛 외관을 보여준다(추론). 외관 생성기 `generate_field_appearance`는 제품 코드 호출처가 없다.
- 체크박스 켜짐 상태 이름을 `/Yes`로 하드코딩한다(실제 파일은 다른 이름을 쓸 수 있다).

## Code
- `justpdf-core/src/form/fill.rs` — `set_field_value`, `toggle_checkbox`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [폼 외관](form-appearance.md) — 연결되어야 할 쪽.
- [AcroForm](acroform.md) — 필드 모델.
- [문서 수정기](document-modifier.md) — 저장 경로.

## Known holes / open
- 채운 값이 화면에 보이지 않는다(위, 추론; 렌더로 확인하는 테스트 없음).
- Tracked: #40 (폼 채우기·평탄화)
