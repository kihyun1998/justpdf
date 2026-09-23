# 서명 외관 (visible signature)

## What it is
보이는 서명의 Form XObject를 만든다: Helvetica Type1(`/F1`)로 서명자·사유·날짜를 쓰고 테두리를 그린다.

## Governing decisions
**None.**

## Design model
- `build_pdf_with_placeholder`가 `date=None`으로 부른다 — 날짜는 표시되지 않는다.
- 자체 `escape_pdf_string`(세 사본 중 하나)으로 Rust `&str`을 literal `Tj`에 넣는다. 비 ASCII 서명자 이름은 WinAnsi Helvetica에서 엉뚱한 글자로 그려진다(추론) — [콘텐츠 텍스트 인코딩](../invariant/content-text-encoding.md).
- 폼·주석 외관 생성기와 규칙이 다르다: 이쪽만 `/Resources`를 선언한다.

## Code
- `justpdf-core/src/sign/appearance.rs` — `generate_signature_appearance`, `escape_pdf_string`

## Reference behaviour
**None.**

## Cross-cutting invariants
- [콘텐츠 텍스트 인코딩](../invariant/content-text-encoding.md)
- [객체 구문 왕복](../invariant/object-syntax-roundtrip.md) — 자체 이스케이프 사본.

## Blast radius
- [서명](signing.md) — 호출자.
- [폼 외관](form-appearance.md), [주석 외관](annotation-appearance.md) — 같은 일을 하는 다른 두 생성기. 한쪽의 이스케이프·폰트 규칙을 고치면 셋을 함께 본다.

## Known holes / open
- Tracked: #29 (손으로 쓰는 구문 이스케이프), #34 (비 ASCII 콘텐츠 텍스트)
