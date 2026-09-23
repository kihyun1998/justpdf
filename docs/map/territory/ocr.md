# OCR (검색 가능한 PDF)

## What it is
외부 `tesseract` 프로그램으로 이미지·PDF 페이지를 OCR하고, 스캔 이미지 위에 보이지 않는 텍스트 층을 얹은 검색 가능한 PDF를 만든다. PDF 페이지는 render로 PNG를 만든 뒤 OCR한다.

## Governing decisions
- [ADR-0001](../../adr/0001-crates-split-by-dependency-layer.md) — `ocr` 기능만 render를 끌어온다(#1).

## Design model
- `make_searchable_pdf`는 이미지 배치용 `content_prefix`(`q w 0 0 h 0 0 cm`)를 만들고 **쓰지 않는다** — 이미지가 단위 정사각형에 놓인다(추론).
- 주석은 "render mode 3 = invisible"이라 하지만 `Tr`을 쓰지 않고 `PageBuilder`에는 그럴 방법도 없다. "보이지 않는" OCR 텍스트가 1pt Helvetica로 한 자리에 겹쳐 보인다(추론). 비 ASCII는 [콘텐츠 텍스트 인코딩](../invariant/content-text-encoding.md) 문제를 공유한다.
- 임시 파일로 tesseract와 통신한다.

## Code
- `justpdf-special/src/ocr/mod.rs` — `is_tesseract_available`, `tesseract_version`, `ocr_image`, `ocr_pdf_page`, `make_searchable_pdf`

## Reference behaviour
**None.**

## Cross-cutting invariants
- [콘텐츠 텍스트 인코딩](../invariant/content-text-encoding.md)

## Blast radius
- [문서 빌더](document-builder.md) — 텍스트 렌더 모드·이미지 `cm`을 지원해야 이 기능이 약속대로 된다.
- [렌더 API](render-api.md) — 페이지 래스터화.
- [크레이트 레이어링](crate-layering.md) — `ocr` 기능 게이트.

## Known holes / open
- 테스트는 tesseract가 있을 때와 없을 때 각각 한쪽만 단언한다(`test_tesseract_not_found_error`는 없을 때만).
- Tracked: #34 (비 ASCII 콘텐츠 텍스트), #43 (인라인 이미지 cm 누락)
