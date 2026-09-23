# 폰트 사전 로딩과 폭

## What it is
폰트 사전을 `FontInfo`(BaseFont, Subtype, Encoding, 폭, 디스크립터)로 바꾼다. 표준 14 폰트의 폭 표를 기본값으로 쓴다. 텍스트 추출과 렌더러 둘 다 이 함수에서 출발하지만, 그 위에서 각자 다르게 보강한다.

## Governing decisions
**None.**

## Design model
- ToUnicode는 여기서 풀지 않는다("Resolved later by the document").
- 기본 폭: 표준 14면 600, 아니면 1000. 표준 14 폭 표는 근사다("Simplified Helvetica widths", Symbol/ZapfDingbats는 전부 500).
- **CID 폭(`/W`)은 여기서 채워지지 않는다**. `FontWidths::CID`는 텍스트 추출 쪽 `parse_cid_widths`만 채운다. 렌더러는 하위 폰트에 `parse_font_info`를 부르므로 `FontWidths::None`을 받는다 — 같은 Type0 폰트가 렌더와 텍스트에서 다르게 전진한다([폰트 해석 경로](../invariant/font-resolution.md)).

## Code
- `justpdf-core/src/font/mod.rs` — `FontInfo`, `FontDescriptor`, `FontWidths`, `CIDWidthEntry`, `get_width`, `parse_font_info`, `parse_font_descriptor`, `parse_widths`
- `justpdf-core/src/font/standard14.rs` — `is_standard14`, `standard14_widths`, `strip_subset_prefix`

## Reference behaviour
**None.** 코드가 "PDF spec 7.6", "Table 123"을 인용한다. 비교 대상 조항: ISO 32000-2 §9.6(단순 폰트), §9.7(복합 폰트), §9.8(디스크립터).

## Cross-cutting invariants
- [폰트 해석 경로](../invariant/font-resolution.md)

## Blast radius
- [텍스트 추출](text-extraction.md), [글리프 렌더링](glyph-rendering.md), [SVG 렌더러](svg-renderer.md) — 세 소비처가 이 결과 위에 각자 보강을 얹는다.
- [폰트 인코딩](font-encodings.md) — `parse_encoding`이 같은 파일에 있다.
- [CID 폰트](cid-fonts.md) — CID 폭의 실제 출처.
- [폰트 서브세팅](font-subsetting.md) — `/Widths` 인덱싱 규칙을 공유한다.
- [plaintext 입력](plaintext-input.md) — Courier 폭 0.6을 이 표를 부르지 않고 하드코딩한다.

## Known holes / open
- "CID font: /W array (not yet fully parsed)" 주석은 이 함수에 대해서는 여전히 참이다.
