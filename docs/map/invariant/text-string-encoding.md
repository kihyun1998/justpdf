# 텍스트 문자열 인코딩

## The fact
사람이 읽는 PDF 문자열 값(Info의 Title·Author, 아웃라인 제목, 주석 내용, 폼 값·필드 이름, 서명자 이름·사유, 페이지 레이블 접두사, 첨부 파일명 등 — 스펙의 "text string")은 **PDFDocEncoding 또는 BOM이 붙은 UTF-16BE**(PDF 2.0은 BOM 붙은 UTF-8도)여야 한다. 읽을 때는 이 인코딩을 디코드하고, 쓸 때는 Rust `&str`을 이 인코딩으로 바꿔야 한다. BOM 없는 UTF-8 바이트는 다른 리더가 PDFDocEncoding으로 읽어 깨진다.

## Why it is cross-cutting
텍스트 문자열을 읽고 쓰는 곳이 문서 수준 기능마다 따로 있고, 공유 인코더가 없다. 올바른 디코더는 두 곳에 있지만([폰트 인코딩](../territory/font-encodings.md)의 `decode_text`, [첨부파일](../territory/embedded-files.md)의 `obj_to_string`) 다른 모듈이 재사용하지 않으며, **UTF-16BE 인코더는 저장소 어디에도 없다**. 사이트끼리 호출하지 않으므로 간선이 아니라 노드로만 이 사실을 실을 수 있다. [객체 구문 왕복](object-syntax-roundtrip.md)과 다른 사실이다: 그쪽은 바이트 보존, 이쪽은 바이트의 의미다 — 바이트가 완벽히 왕복해도 이 불변식은 깨질 수 있다.

## Territories it holds in
쓰기(`&str` → BOM 없는 UTF-8):
- [문서 빌더](../territory/document-builder.md) — `set_title` 등 Info.
- [문서 수정기](../territory/document-modifier.md) — `set_info`.
- [아웃라인](../territory/outlines.md) — 제목.
- [주석](../territory/annotations.md) — `build_dict`.
- [액션](../territory/actions.md) — `build_action`.
- [첨부파일](../territory/embedded-files.md) — `/F`·`/UF`.
- [서명](../territory/signing.md) — `/Name`·`/Reason`·`/Location`(literal로 직접).
- [페이지 레이블](../territory/page-labels.md) — 접두사.

읽기(`from_utf8_lossy`):
- [주석](../territory/annotations.md), [아웃라인](../territory/outlines.md), [AcroForm](../territory/acroform.md)(`/T`, 값), [폼 외관](../territory/form-appearance.md)(`value_as_string`), [서명 감지](../territory/signature-detection.md), [페이지 레이블](../territory/page-labels.md).

올바른 디코더의 위치:
- [폰트 인코딩](../territory/font-encodings.md) — `decode_text`, `decode_utf16be`.
- [첨부파일](../territory/embedded-files.md) — `obj_to_string`.

바이트 층:
- [객체 직렬화](../territory/object-serialization.md) — 바이트는 보존(hex 경로)하지만 인코딩을 선택하지 않는다.

다시 찾는 명령: `rg -n 'from_utf8_lossy|\.as_bytes\(\)\.to_vec\(\)' justpdf-core/src`의 결과 중 PDF 문자열 값을 다루는 줄.

## What a violation looks like
- justpdf → justpdf 왕복은 멀쩡하다(쓴 UTF-8을 UTF-8로 되읽으므로). 그래서 자체 테스트로는 보이지 않는다.
- 다른 뷰어(Acrobat, 브라우저, MuPDF)에서 한글·일본어·악센트 문자 제목·북마크·서명자 이름이 모지바케로 보인다.
- 다른 도구가 만든 UTF-16BE 제목을 justpdf가 읽으면 U+FFFD 투성이가 된다.

## Discovery history
기록된 사고가 없다. 2026-09-23 맵 작성 중 연구 에이전트 세 개(쓰기·대화형 기능·서명)가 서로 독립적으로 같은 결함을 각자의 영역에서 보고했다 — 한 번의 읽기에서 14개 사이트. 연구 에이전트 프로브: `set_title("한글")` → `<ED959CEAB880>`(BOM 없는 UTF-8).

- Tracked: #33 (텍스트 문자열 인코딩)

## Where it will recur
**`PdfObject::String`에 사람이 읽을 텍스트를 넣거나, 사람이 읽을 텍스트를 `PdfObject::String`에서 꺼내는 함수는 이 불변식의 대상이다.** 확인할 것: 쓰기에서 비 ASCII가 가능하면 UTF-16BE+BOM으로 인코딩하는가, 읽기에서 BOM을 보고 디코드하는가. 공유 인코더/디코더 한 쌍이 생기기 전까지는 사이트마다 다시 구현될 것이다.
