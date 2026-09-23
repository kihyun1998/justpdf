# 콘텐츠 텍스트 인코딩

## The fact
콘텐츠 스트림의 텍스트 표시 연산자(`Tj`/`TJ`)에 들어가는 바이트는 **선택된 폰트의 인코딩으로 된 문자 코드**다. Rust 문자열의 UTF-8 바이트를 그대로 넣으면, 인코딩 없는 표준 Type1 폰트(WinAnsi 계열)에서는 ASCII 밖의 모든 문자가 엉뚱한 글리프가 된다. 비 ASCII 텍스트를 올바르게 쓰려면 글리프를 가진 폰트를 임베드하고(CID 폰트 등) 그 폰트의 코드로 인코딩해야 한다.

## Why it is cross-cutting
텍스트를 PDF 페이지에 쓰는 기능이 여럿이고, 각자 표준 Type1 폰트와 UTF-8 바이트라는 같은 가정을 **따로** 한다. 올바른 재료([CJK 폰트 임베딩](../territory/cjk-font-embedding.md))는 존재하지만 어떤 쓰기 경로에도 연결되어 있지 않다. [텍스트 문자열 인코딩](text-string-encoding.md)과 다른 사실이다 — 그쪽은 사전 값(메타데이터), 이쪽은 페이지에 그려지는 글자다.

## Territories it holds in
- [문서 빌더](../territory/document-builder.md) — `PageBuilder::show_text` + `add_standard_font`. 대부분의 사이트가 이 한 쌍을 거친다.
- [포맷 변환 계약](../territory/format-document.md) — 텍스트 포맷 공통 경로(Courier 10pt).
- 입력 포맷별: [xps](../territory/xps-input.md), [epub](../territory/epub-input.md), [office](../territory/office-input.md), [mobi](../territory/mobi-input.md), [fb2](../territory/fb2-input.md), [plaintext](../territory/plaintext-input.md).
- [OCR](../territory/ocr.md) — 검색용 텍스트 층.
- 외관 생성기: [폼 외관](../territory/form-appearance.md), [서명 외관](../territory/signature-appearance.md), [주석 외관](../territory/annotation-appearance.md)(스탬프).
- [CJK 폰트 임베딩](../territory/cjk-font-embedding.md) — 해결 재료(미연결).

## What a violation looks like
- EPUB·DOCX·텍스트 파일의 한글을 `justpdf convert`하면 PDF에 라틴 확장 문자 조각이 찍힌다(추론 — 저장소에 비 ASCII 변환 테스트가 없다).
- 한글 서명자 이름의 보이는 서명, 한글 폼 값의 외관이 깨진다.
- 텍스트 추출은 ToUnicode가 없으므로 WinAnsi로 디코드해 역시 깨진 문자열을 돌려준다.

## Discovery history
기록된 사고가 없다. 2026-09-23 맵 작성 중 네 연구 에이전트(쓰기·대화형·서명·formats/special)가 각 영역에서 독립적으로 보고했다. 모든 판단은 코드 읽기에 의한 추론이며 비 ASCII 출력을 렌더해 확인한 테스트는 없다.

- Tracked: #34 (비 ASCII 콘텐츠 텍스트)

## Where it will recur
**`show_text`나 `Tj`를 쓰는 새 기능, 또는 외관 스트림에 텍스트를 넣는 함수는 이 불변식의 대상이다.** 확인할 것: 입력이 ASCII 밖일 수 있는가? 그렇다면 표준 폰트가 아닌 임베드 폰트와 그 인코딩이 필요하다. 테스트는 비 ASCII 입력을 쓰고 결과를 렌더하거나 ToUnicode 기반 추출로 되읽어야 한다.
