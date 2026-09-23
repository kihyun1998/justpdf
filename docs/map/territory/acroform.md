# AcroForm 필드 모델

## What it is
`/AcroForm /Fields` 트리를 걸어 필드(텍스트·체크박스·라디오·선택·서명)를 모델로 만든다. 상속 속성(`FT`, `Ff`, `DA`)을 내려보낸다.

## Governing decisions
**None.**

## Design model
- 순환 방어가 없다. `/T`는 손실 디코드된다 — [텍스트 문자열 인코딩](../invariant/text-string-encoding.md).
- `page_obj_num`은 항상 `None`.
- `/T` 없는 위젯 자식이 부모 이름을 가진 별도 "필드"가 되고 `/V`가 없다(추론).
- `/Opt` 쌍은 표시 문자열을 택한다.

## Code
- `justpdf-core/src/form/types.rs` — `FieldType`, `FieldFlags`, `FormField`, `AcroForm`, `value_as_string`
- `justpdf-core/src/form/parse.rs` — `parse_acroform`, `walk_field_tree`

## Reference behaviour
**None.** 비교 대상 조항: ISO 32000-2 §12.7.

## Cross-cutting invariants
- [텍스트 문자열 인코딩](../invariant/text-string-encoding.md)

## Blast radius
- [폼 채우기](form-fill.md), [폼 평탄화](form-flatten.md), [폼 외관](form-appearance.md) — 이 모델 위의 기능.
- [서명 감지](signature-detection.md) — 같은 트리를 따로 걷는다.
- [파사드](facade.md) — `form_fields`.

## Known holes / open
- 필드 트리 순환 시 무한 재귀(추론).
- Tracked: #33 (텍스트 문자열 인코딩), #52 (순환 방어)
