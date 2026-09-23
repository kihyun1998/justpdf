# 폰트 해석 경로

## The fact
같은 폰트는 텍스트 추출과 렌더링에서 **같은 방식으로 해석되어야** 한다: 문자 코드 분할(1/2바이트), 코드 → 글리프 매핑, 코드 → 유니코드 매핑, 글리프 폭. 두 경로가 폭을 다르게 계산하면 추출한 텍스트의 위치와 렌더된 글리프 위치가 어긋나고, 검색 하이라이트·리댁션 영역·레이아웃 분석이 화면과 맞지 않는다.

## Why it is cross-cutting
두 소비처가 core `parse_font_info`에서 출발하지만 그 위를 **각자** 보강하고, 서로 호출하지 않는다:
- 텍스트는 `/W`·`/DW`를 읽고(렌더는 안 읽음), 인코딩 표로 디코드한다(렌더는 안 씀).
- 렌더는 `/CIDToGIDMap`을 읽고(텍스트는 필요 없음), 바이트를 유니코드 스칼라로 보고 폰트 cmap을 찾는다.
- ToUnicode 해석 코드가 두 벌이다.
- core에는 두 경로 어느 쪽도 쓰지 않는 폰트 모듈이 여럿 있다(CFF, Type3, recovery, OpenType 레이아웃).

## Territories it holds in
- [폰트 로딩](../territory/font-loading.md) — 공통 출발점(CID 폭을 채우지 않음).
- [폰트 인코딩](../territory/font-encodings.md) — 텍스트만 쓰는 인코딩 표.
- [ToUnicode](../territory/tounicode.md) — 두 벌의 해석.
- [CID 폰트](../territory/cid-fonts.md) — 폭은 텍스트에만, GID 매핑은 렌더에만.
- [텍스트 추출](../territory/text-extraction.md) — `resolve_fonts`, `resolve_to_unicode`.
- [렌더 인터프리터](../territory/render-interpreter.md) — `resolve_page_fonts`.
- [글리프 렌더링](../territory/glyph-rendering.md) — `char_code_to_glyph_id`.
- [SVG 렌더러](../territory/svg-renderer.md) — 세 번째 `resolve_page_fonts`.
- [폰트 서브세팅](../territory/font-subsetting.md) — 서브셋 결과를 두 경로가 다르게 읽는다(렌더는 복사된 cmap, 텍스트는 ToUnicode).

## What a violation looks like
- Type0(CJK) 폰트 페이지에서 렌더된 글자 간격과 추출된 텍스트 좌표가 다르다(렌더는 1000 고정 폭 — 추론).
- 서브셋된 폰트가 렌더에서 엉뚱한 글리프를 그리는데 추출 텍스트는 멀쩡하다 — 압축 테스트가 추출만 보므로 통과한다.
- `/Differences` 폰트에서 추출과 렌더가 서로 다른 방식으로 틀린다.

## Discovery history
기록된 사고가 없다. 2026-09-23 맵 작성 중 폰트/텍스트·렌더 연구 에이전트가 보고했다. 모두 코드 읽기에 의한 추론이다.

- Tracked: #45 (/Differences·CID 폭·CFF), #51 (서브세팅 GID 매핑)

## Where it will recur
**폰트 사전에서 코드·글리프·폭·유니코드 중 하나를 얻는 함수를 텍스트나 렌더 한쪽에 추가하면 이 불변식의 대상이다.** 확인할 것: 다른 쪽에도 같은 정보가 필요한가? 공유 폰트 해석 타입이 core에 생기기 전까지, 한쪽 수정은 다른 쪽 수정 여부를 명시적으로 판단해야 한다.
