# Type3 폰트

## What it is
Type3 폰트 사전(`/CharProcs`, `/FontMatrix`, `/Encoding /Differences`, 폭)을 파싱하고 문자 코드 → 글리프 이름 → CharProc 스트림을 찾는다.

## Governing decisions
**None.**

## Design model
- 저장소에서 유일한 `/Differences` 파서가 여기 있다(Type3 전용).

## Code
- `justpdf-core/src/font/type3.rs` — `parse_type3_font`, `Type3Font`, `glyph_name`, `char_proc`, `resolve_char_proc`, `DEFAULT_FONT_MATRIX`

## Reference behaviour
**None.** 비교 대상 조항: ISO 32000-2 §9.6.4.

## Cross-cutting invariants
**None.**

## Blast radius
- [렌더 인터프리터](render-interpreter.md) — `d0`/`d1`이 no-op이고 CharProc를 실행하지 않는다. Type3 렌더링을 넣을 곳.
- [텍스트 추출](text-extraction.md) — 폭을 1000으로 나누고 `/FontMatrix`를 무시한다(추론).
- [폰트 인코딩](font-encodings.md) — `/Differences` 파서를 일반 단순 폰트로 옮길 후보.

## Known holes / open
- 제품 코드 소비처가 없다.
- Tracked: #45 (/Differences·CID 폭·CFF)
