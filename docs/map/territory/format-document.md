# 포맷 변환 계약 (`FormatDocument`)

## What it is
PDF가 아닌 입력(XPS, EPUB, Office, SVG, CBZ, MOBI, FB2, 텍스트)이 구현하는 공통 트레이트: 메타데이터·페이지 수·페이지 텍스트·미리보기 렌더·`to_pdf`. 각 입력 포맷 노트는 이 계약의 구현이다.

## Governing decisions
- [ADR-0001](../../adr/0001-crates-split-by-dependency-layer.md) — formats는 `zip`·`roxmltree`를 더하는 별도 레이어이며, render는 기능 플래그 뒤에서만 끌어온다.

## Design model
- **PDF 생성은 전부 core [문서 빌더](document-builder.md)를 거친다.** 포맷 코드가 PDF 구문을 손으로 쓰지 않는다.
- 텍스트 포맷(xps, epub, office, plaintext, mobi, fb2)은 `add_standard_font("Courier")` 10pt에 줄 단위 `show_text` — 비 ASCII·CJK는 깨진다(추론) — [콘텐츠 텍스트 인코딩](../invariant/content-text-encoding.md).
- 이미지 포맷(cbz, svg)은 래스터화 후 `draw_inline_image` — `cm`을 쓰지 않아 약 1pt×1pt로 놓인다(추론).
- 미리보기 렌더(`render_page`)는 한 페이지 PDF를 만들어 render 크레이트로 그리는 포맷과(plaintext·mobi·fb2, 그리고 plaintext를 빌려 쓰는 epub·office), 자체 래스터(svg·cbz), 빈 페이지(xps)로 나뉜다.
- 모듈은 기능 플래그로만 컴파일된다(`lib.rs`).

## Code
- `justpdf-formats/src/common.rs` — `FormatDocument`, `FormatPage`, `FormatMetadata`, `RenderedPage`
- `justpdf-formats/src/lib.rs` — `FormatDocument`
- `justpdf-formats/Cargo.toml` — `plaintext`, `mobi`, `fb2`, `all`

## Reference behaviour
**None.** MuPDF(example)가 같은 입력 포맷을 지원한다(`docs/mupdf-feature-analysis.md`). 변환 결과를 비교한 기록은 없다.

## Cross-cutting invariants
- [콘텐츠 텍스트 인코딩](../invariant/content-text-encoding.md)

## Blast radius
- 각 입력 포맷 — [xps](xps-input.md), [epub](epub-input.md), [office](office-input.md), [svg](svg-input.md), [cbz](cbz-input.md), [mobi](mobi-input.md), [fb2](fb2-input.md), [plaintext](plaintext-input.md). 트레이트 시그니처 변경은 여덟 구현 모두에 닿는다.
- [문서 빌더](document-builder.md) — 모든 `to_pdf`의 하층.
- [렌더 API](render-api.md) — 미리보기.
- [크레이트 레이어링](crate-layering.md) — 어떤 기능이 render를 끄는지.
- [CLI](cli.md) — 유일한 저장소 내 소비처(`convert`).

## Known holes / open
- `to_pdf` 테스트 대부분이 `starts_with(b"%PDF")`만 확인한다.
- Tracked: #34 (비 ASCII 콘텐츠 텍스트)
