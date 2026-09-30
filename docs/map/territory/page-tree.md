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
- `page_count`는 루트 `/Count`를 믿는다. 음수이거나 없으면 0이다.
- `get_page`는 음이 아닌 정수 `/Count`로만 서브트리를 건너뛴다. 음수이거나 없는 `/Count`는 아무것도 건너뛰지 않는다 — 그런 서브트리도 `collect_pages`와 같은 페이지를 찾는다.
- 두 워커(`walk_page_tree`, `walk_page_tree_find`)가 거의 중복이다.
- `get_page` fails with `PageOutOfRange { index, count }` when the index is at or past the root `/Count` (`count` is that `/Count`), or, when the root has no usable `/Count`, when the walk ends without reaching the index (`count` is what the walk counted, a pruned subtree counting as its `/Count`). An index below the root `/Count` that the walk does not reach is `InvalidObject` — the `/Count` overstates the tree. The bindings use the variant to tell an index out of range from a damaged file (#134; measured by `integration.rs` `test_get_page_*`).
- An overstating `/Count` is file damage, not an index out of range — the **maintainer's judgement** (2026-09-30, #134). Shown: MuPDF `pdf-page.c` throws `FZ_ERROR_ARGUMENT` "invalid page number" only in `pdf_load_page` when the number is at or past `pdf_count_pages` (the root `/Count`; a missing one counts as 0), and `FZ_ERROR_FORMAT` "cannot find page %d in page tree" when `pdf_lookup_page_loc` walks past the tree. Options were that split, reporting both as `PageOutOfRange` (the brief), or deferring to #133. Chosen: the split, accepting that iterating a damaged document in Python raises `RuntimeError` at the first missing page instead of stopping, and that it answers, for `get_page` only, one case of the question #133 asks (which of `/Count` and the walk to believe) — not the two cases #133 lists.
- `/Pages` 노드가 자기 조상을 kid로 가지면 `CircularReference`다. 같은 노드를 두 번 나열한 것은 두 번 걷되, 방문 예산을 넘기면 `LimitExceeded`다 — [트리 순회 순환](../invariant/tree-traversal-cycles.md). 예산은 `/Count`를 보지 않으므로 거짓 `/Count`로 가지치기를 끈 `get_page`도 멈춘다.

## Code
- `justpdf-core/src/page/mod.rs` — `Rect`, `PageInfo`, `collect_pages`, `page_count`, `get_page`, `walk_page_tree`, `walk_page_tree_find`, `InheritedAttrs`, `subtree_count`
- `justpdf-core/src/tree_walk.rs` — `VisitBudget` (여덟 트리 워커가 함께 쓴다 — [트리 순회 순환](../invariant/tree-traversal-cycles.md))

## Reference behaviour
**None.** 비교 대상 조항: ISO 32000-2 §7.7.3(페이지 트리, 상속 속성).

## Cross-cutting invariants
- [트리 순회 순환](../invariant/tree-traversal-cycles.md)

## Blast radius
- [렌더 API](render-api.md), [렌더 인터프리터](render-interpreter.md) — `PageInfo`/`Rect`를 그대로 쓴다.
- [텍스트 추출](text-extraction.md), [주석](annotations.md), [AcroForm](acroform.md), [페이지 레이블](page-labels.md), [optional content](optional-content.md) — 페이지 조회에 기댄다.
- [문서 수정기](document-modifier.md) — 페이지 삽입·삭제·재정렬이 이 구조를 다시 쓴다.
- [파사드](facade.md), [CLI](cli.md), [언어 바인딩](language-bindings.md) — `collect_pages`/`get_page`/`page_count`를 직접 부른다.

## Known holes / open
- 깊이 제한이 없다 — 순환 없는 극단적 깊이는 스택을 넘길 수 있다(추론).
- 같은 파일에 `get_page`와 `collect_pages`·`page_count`가 다르게 답할 수 있다 — [트리 순회 순환](../invariant/tree-traversal-cycles.md)의 "다루지 않는 것".
- Tracked: #122 (깊이 제한), #133 (페이지 API 불일치)
