# 단순 폰트 인코딩

## What it is
단순 폰트의 1바이트 코드를 유니코드로 바꾸는 인코딩 표(Standard, WinAnsi, MacRoman, PDFDoc, Identity)와 `/Encoding` 해석. 텍스트 추출이 ToUnicode가 없을 때 쓰는 경로이며, PDF 텍스트 문자열(PDFDocEncoding / UTF-16BE BOM) 디코더도 여기 있다.

## Governing decisions
**None.**

## Design model
- **`/Differences`를 처리하지 않는다**: 인코딩 사전은 `StandardEncoding`으로 떨어진다(`parse_encoding`의 TODO). `/Differences` 파서는 Type3 전용 `parse_type3_font`에만 있고, 그 함수는 호출처가 없다.
- MacRoman은 WinAnsi 표를 쓴다("simplified — uses same table for now"). StandardEncoding도 WinAnsi로 디코드한다("close enough for display").
- 렌더러는 이 인코딩을 쓰지 않는다: `char_code_to_glyph_id`는 바이트를 유니코드 스칼라로 보고 폰트 cmap을 찾는다.
- PDF 텍스트 문자열을 제대로 디코드하는 함수(`decode_text`, UTF-16BE BOM 처리)가 여기 있지만, 주석·아웃라인·폼·서명 읽기 쪽은 이것을 쓰지 않고 `from_utf8_lossy`를 쓴다 — [텍스트 문자열 인코딩](../invariant/text-string-encoding.md).

## Code
- `justpdf-core/src/font/encoding.rs` — `Encoding`, `from_name`, `decode_text`, `decode_winansi`, `decode_mac_roman`, `decode_pdfdoc`, `decode_utf16be`
- `justpdf-core/src/font/mod.rs` — `parse_encoding`

## Reference behaviour
**None.** 비교 대상: ISO 32000-2 Annex D(문자 집합과 인코딩), §9.6.5.

## Cross-cutting invariants
- [텍스트 문자열 인코딩](../invariant/text-string-encoding.md) — 유일한 올바른 디코더의 위치.
- [폰트 해석 경로](../invariant/font-resolution.md) — 텍스트는 이 표를, 렌더는 폰트 cmap을 쓴다.

## Blast radius
- [텍스트 추출](text-extraction.md) — `show_string`이 `decode_text`를 부른다.
- [글리프 렌더링](glyph-rendering.md) — 인코딩을 쓰지 않는 쪽. `/Differences`를 지원하면 렌더에도 따로 넣어야 한다.
- [Type3 폰트](type3-fonts.md) — `/Differences` 파서가 있는 곳.
- [ToUnicode](tounicode.md) — 우선순위가 더 높은 매핑.

## Known holes / open
- `/Differences` 미처리는 비표준 인코딩 폰트의 텍스트 추출을 틀리게 만든다(가장 흔한 실사용 파일 유형 중 하나 — 추론).
- Tracked: #33 (텍스트 문자열 인코딩), #45 (/Differences·CID 폭·CFF)
