# 텍스트 입력 (plaintext)

## What it is
평문 텍스트를 페이지로 나눠 Courier로 PDF를 만든다. 미리보기 렌더 구현을 EPUB·Office가 빌려 쓴다.

## Governing decisions
- [ADR-0001](../../adr/0001-crates-split-by-dependency-layer.md) — `plaintext`는 render를 끌어온다(#2).

## Design model
- 글자 폭을 `font_size * 0.6`으로 하드코딩한다(Courier 600과 일치하지만 core 표를 부르지 않음 — [폰트 로딩](font-loading.md)).
- `render_page`는 한 페이지 PDF를 만들어 render로 그린다. EPUB·Office가 이 모듈을 호출하므로 **이 모듈의 기능 게이트가 세 포맷의 빌드 가능성을 좌우한다**.

## Code
- `justpdf-formats/src/plaintext/mod.rs` — `PlainTextDocument`, `to_pdf`, `render_page`

## Reference behaviour
**None.**

## Cross-cutting invariants
- [콘텐츠 텍스트 인코딩](../invariant/content-text-encoding.md)

## Blast radius
- [EPUB](epub-input.md), [Office](office-input.md) — 숨은 호출자.
- [크레이트 레이어링](crate-layering.md) — 기능 게이트.
- [렌더 API](render-api.md).

## Known holes / open
- Tracked: #35 (epub·office 단독 빌드)
