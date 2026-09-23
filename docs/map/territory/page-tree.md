# 페이지 트리

## What it is
Catalog → `/Pages` → `/Kids`를 걸어 `PageInfo` 목록을 만들고, 상속 속성(MediaBox, CropBox, Rotate, Resources)을 내려보낸다. `get_page`는 `/Count`로 서브트리를 건너뛴다. 페이지 단위 기능(렌더, 텍스트, 주석, 편집)의 공통 입구다.

## Governing decisions
**None.**

## Design model
코드에서 읽어낸 규칙이다.
- 상속되는 것은 MediaBox·CropBox·Rotate·Resources뿐이다. Bleed/Trim/Art는 상속되지 않는다.
- MediaBox가 없으면 612×792.
- `/Type /Page`이거나 MediaBox(상속 포함)가 있으면 페이지로 본다.
- 간접 배열로 된 박스는 무시된다(`get_array`가 참조를 따르지 않음 — [객체 모델](object-model.md)).
- `page_count`는 루트 `/Count`를 믿는다.
- 두 워커(`walk_page_tree`, `walk_page_tree_find`)가 거의 중복이다.

## Code
- `justpdf-core/src/page/mod.rs` — `Rect`, `PageInfo`, `collect_pages`, `page_count`, `get_page`, `walk_page_tree`, `walk_page_tree_find`, `InheritedAttrs`

## Reference behaviour
**None.** 비교 대상 조항: ISO 32000-2 §7.7.3(페이지 트리, 상속 속성).

## Cross-cutting invariants
**None.**

## Blast radius
- [렌더 API](render-api.md), [렌더 인터프리터](render-interpreter.md) — `PageInfo`/`Rect`를 그대로 쓴다.
- [텍스트 추출](text-extraction.md), [주석](annotations.md), [AcroForm](acroform.md), [페이지 레이블](page-labels.md), [optional content](optional-content.md) — 페이지 조회에 기댄다.
- [문서 수정기](document-modifier.md) — 페이지 삽입·삭제·재정렬이 이 구조를 다시 쓴다.
- [파사드](facade.md), [CLI](cli.md), [언어 바인딩](language-bindings.md) — `collect_pages`/`get_page`/`page_count`를 직접 부른다.

## Known holes / open
- `/Kids` 순환에 대한 방어(visited, 깊이 제한)가 없다 — 순환 트리는 무한 재귀(추론).
- Tracked: #52 (순환 방어)
