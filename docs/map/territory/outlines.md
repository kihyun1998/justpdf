# 아웃라인 (북마크)

## What it is
`/Outlines` 트리와 이름 있는 목적지를 읽고, 아웃라인을 설정·제거한다.

## Governing decisions
**None.**

## Design model
- `/Title`은 손실 디코드(UTF-16BE 미처리)되고 UTF-8 바이트로 쓰인다 — [텍스트 문자열 인코딩](../invariant/text-string-encoding.md).
- 항목이 자기 조상으로 돌아가면(`/First`든 `/Next`든) `CircularReference`다. 같은 층의 `/Next` 고리는 조용히 멈춘다. 서로 다른 항목이 같은 `/First`를 공유하면 그 자식은 두 번 읽히고, 방문 예산을 넘기면 `LimitExceeded`다. 이름 있는 목적지 트리는 순환 kid와 예산을 넘긴 kid를 건너뛴다 — [트리 순회 순환](../invariant/tree-traversal-cycles.md).
- `/A`는 `/D`만 읽는다.

## Code
- `justpdf-core/src/outline/parse.rs` — `read_outlines`, `read_outline_siblings`, `read_named_destinations`
- `justpdf-core/src/outline/builder.rs` — `set_outlines`, `remove_outlines`
- `justpdf-core/src/outline/types.rs` — `Destination`, `OutlineItem`, `OutlineStyle`

## Reference behaviour
**None.** 비교 대상 조항: ISO 32000-2 §12.3.3.

## Cross-cutting invariants
- [텍스트 문자열 인코딩](../invariant/text-string-encoding.md)
- [트리 순회 순환](../invariant/tree-traversal-cycles.md)

## Blast radius
- [액션](actions.md) — `Destination` 공유, `/A` 중복 파싱.
- [문서 수정기](document-modifier.md) — 설정 경로.
- [병합](document-modifier.md) — 병합 시 아웃라인을 옮기는지 확인한 기록 없음.
- [파사드](facade.md) — `outlines`.

## Known holes / open
- 한글 제목 북마크를 만들면 다른 뷰어에서 깨진다(추론, 위 불변식).
- 깊이 제한이 없다 — 순환 없는 극단적 깊이는 스택을 넘길 수 있다(추론).
- Tracked: #33 (텍스트 문자열 인코딩), #122 (깊이 제한)
