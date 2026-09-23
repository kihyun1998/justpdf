# 글리프 렌더링

## What it is
텍스트 표시 연산자의 문자 코드를 글리프 ID로 바꾸고, 임베드 폰트에서 윤곽을 꺼내 그리고, 윤곽을 캐시한다.

## Governing decisions
**None.**

## Design model
- 폰트 데이터는 FontFile2 → FontFile3 → FontFile 순으로 찾아 전부 `ttf_parser::Face::parse`에 넘긴다. 순수 CFF·Type1 프로그램은 실패해 **자리표시 사각형**이 그려진다(추론). core의 [CFF 파서](cff.md)는 쓰지 않는다.
- 코드 → GID: `char_code_to_glyph_id`는 바이트를 유니코드 스칼라로 보고 폰트 cmap을 찾은 뒤, 없으면 코드 == GID. 폰트 `/Encoding`·`/Differences`를 쓰지 않는다([폰트 인코딩](font-encodings.md)).
- CID 폰트는 `/CIDToGIDMap`을 쓰지만 `/W`를 쓰지 않는다 — [폰트 해석 경로](../invariant/font-resolution.md).
- 텍스트는 패턴으로 채워지지 않는다(`fill_color_rgba`만 — 추론).
- 캐시 키는 폰트 FNV 해시 + GID, 기본 용량 4096.

## Code
- `justpdf-render/src/interpreter.rs` — `resolve_page_fonts`, `extract_font_data`, `get_font_descriptor`, `render_text_string`, `render_glyph`, `adjust_text_position`
- `justpdf-render/src/glyph.rs` — `glyph_outline`, `char_code_to_glyph_id`, `units_per_em`
- `justpdf-render/src/glyph_cache.rs` — `GlyphCache`

## Reference behaviour
**None.** 비교 대상 조항: ISO 32000-2 §9.6.6(단순 폰트 글리프 선택), §9.7.4.

## Cross-cutting invariants
- [폰트 해석 경로](../invariant/font-resolution.md)

## Blast radius
- [폰트 로딩](font-loading.md), [CID 폰트](cid-fonts.md), [폰트 인코딩](font-encodings.md) — 입력(일부 우회).
- [폰트 서브세팅](font-subsetting.md) — 서브셋 폰트의 복사된 cmap이 옛 GID를 가리킨다.
- [폰트 대체](font-recovery.md), [CFF 파서](cff.md) — 자리표시 사각형 대신 쓸 수 있는 연결 안 된 모듈.
- [SVG 렌더러](svg-renderer.md) — 별도 텍스트 경로.

## Known holes / open
- 폰트 없는 표준 14 텍스트·CFF·Type1·Type3가 사각형으로 그려진다(추론; 픽셀 검증 테스트 없음).
- Tracked: #45 (/Differences·CID 폭·CFF)
