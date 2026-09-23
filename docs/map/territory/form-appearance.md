# 폼 필드 외관 생성

## What it is
텍스트·선택·버튼 필드의 외관 스트림을 생성한다(`/DA`가 있으면 그 폰트, 없으면 `/Helvetica 10 Tf`).

## Governing decisions
**None.**

## Design model
- 자체 `escape_pdf_string`(세 사본 중 하나): `( ) \`만 이스케이프, CR/LF는 그대로, 비 ASCII는 UTF-8 바이트로 WinAnsi 폰트에 들어간다 — [콘텐츠 텍스트 인코딩](../invariant/content-text-encoding.md), [객체 구문 왕복](../invariant/object-syntax-roundtrip.md).
- `/Resources`를 선언하지 않고 `/DR`을 연결하지 않는다. `{da}`는 원문 그대로 삽입된다.
- 값은 `value_as_string`(`from_utf8_lossy`)을 거치므로 UTF-16BE 값은 U+FFFD가 된다.
- 서명 필드는 `None`을 돌려준다.

## Code
- `justpdf-core/src/form/appearance.rs` — `generate_field_appearance`, `escape_pdf_string`, `radio_appearance`

## Reference behaviour
**None.** 비교 대상 조항: ISO 32000-2 §12.7.4.3(가변 텍스트).

## Cross-cutting invariants
- [콘텐츠 텍스트 인코딩](../invariant/content-text-encoding.md)
- [객체 구문 왕복](../invariant/object-syntax-roundtrip.md)
- [텍스트 문자열 인코딩](../invariant/text-string-encoding.md) — 필드 값 디코드.

## Blast radius
- [폼 채우기](form-fill.md) — 이 생성기를 불러야 할 쪽(현재 안 부름).
- [폼 평탄화](form-flatten.md) — 외관을 페이지에 넣는 쪽.
- [주석 외관](annotation-appearance.md), [서명 외관](signature-appearance.md) — 다른 두 생성기.

## Known holes / open
- 제품 코드 호출처가 없다(자기 테스트뿐).
- Tracked: #29 (손으로 쓰는 구문 이스케이프), #33 (텍스트 문자열 인코딩), #34 (비 ASCII 콘텐츠 텍스트)
