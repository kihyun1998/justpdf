# 텍스트 추출

## What it is
페이지 콘텐츠를 해석해 글리프 위치를 계산하고(텍스트 상태, 폰트 폭), 문자를 유니코드로 바꾸고(ToUnicode → 인코딩), 단어·줄로 묶은 뒤 [읽기 순서](reading-order.md)로 재배열한다. CLI `text`, 파사드 `Page::text`, 모든 바인딩의 `extract_*_text`가 이 파이프라인이다.

## Governing decisions
**None.**

## Design model
- 텍스트 상태와 글리프 전진은 스펙 §9.3 공식을 따른다(코드 주석 인용).
- 폰트 해석을 렌더러와 **따로** 한다: 이쪽은 `/W`를 읽고 ToUnicode·인코딩으로 유니코드를 만든다 — [폰트 해석 경로](../invariant/font-resolution.md).
- ExtGState(`gs`)의 `/Font`를 해석하지 않는다("We don't resolve ExtGState for now" — 문서 접근이 없음).
- **`Do`를 처리하지 않는다**: Form XObject 안의 텍스트는 추출되지 않는다. 렌더러는 재귀한다.
- BDC/OC(optional content)를 보지 않는다 — 숨긴 레이어의 텍스트도 추출된다.
- 페이지 콘텐츠를 자체 private 함수로 이어 붙인다 — [페이지 콘텐츠 조립](../invariant/page-content-assembly.md).
- 페이지는 빈 줄(`"\n\n"`)로 잇는다(`extract_all_text_string`).

## Code
- `justpdf-core/src/text/mod.rs` — `extract_page_text`, `extract_all_text`, `extract_page_text_string`, `extract_all_text_string`, `resolve_fonts`, `resolve_to_unicode`, `get_page_content_data`, `TextInterpreter`, `show_string`, `show_tj_array`, `group_into_words`, `group_into_lines`, `PageText`

## Reference behaviour
**None.** 비교 대상 조항: ISO 32000-2 §9.3, §9.10. MuPDF(example)와 추출 결과를 비교한 기록 없음 — `justpdf-core/examples/compare_mupdf.rs`는 성능 비교다.

## Cross-cutting invariants
- [폰트 해석 경로](../invariant/font-resolution.md)
- [페이지 콘텐츠 조립](../invariant/page-content-assembly.md)

## Blast radius
- [폰트 로딩](font-loading.md), [폰트 인코딩](font-encodings.md), [ToUnicode](tounicode.md), [CID 폰트](cid-fonts.md) — 입력.
- [콘텐츠 스트림 파싱](content-stream-parsing.md) — 연산자 입력.
- [읽기 순서](reading-order.md), [출력 형식](text-output-formats.md), [검색](text-search.md) — 이 출력(`PageText`)의 소비처. 구조체를 바꾸면 셋 다 본다.
- [폰트 서브세팅](font-subsetting.md) — 압축 테스트의 판정자로 쓰인다.
- [optional content](optional-content.md) — 숨김 레이어 무시.
- [CLI](cli.md), [파사드](facade.md), [언어 바인딩](language-bindings.md) — 공개 추출 API 소비처.

## Known holes / open
- Form XObject 안의 텍스트 미추출.
- Tracked: #48 (Form XObject 텍스트)
