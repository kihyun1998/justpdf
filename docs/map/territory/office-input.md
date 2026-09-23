# Office 입력 (DOCX/XLSX/PPTX)

## What it is
OOXML 문서에서 텍스트를 읽어 PDF로 바꾼다.

## Governing decisions
- [ADR-0001](../../adr/0001-crates-split-by-dependency-layer.md)

## Design model
- **`office` 기능 단독으로는 컴파일되지 않는다**: [EPUB](epub-input.md)과 같은 이유(`render_page`가 `crate::plaintext` 호출).
- 넘치는 텍스트를 버린다("Would need pagination here for real use").

## Code
- `justpdf-formats/src/office/mod.rs` — `OfficeDocument`, `OfficeType`, `to_pdf`, `render_page`

## Reference behaviour
**None.**

## Cross-cutting invariants
- [콘텐츠 텍스트 인코딩](../invariant/content-text-encoding.md)

## Blast radius
- [plaintext 입력](plaintext-input.md) — 숨은 의존.
- [크레이트 레이어링](crate-layering.md), [포맷 변환 계약](format-document.md).

## Known holes / open
- 페이지 넘김 없음, 단독 빌드 실패.
- Tracked: #35 (epub·office 단독 빌드)
