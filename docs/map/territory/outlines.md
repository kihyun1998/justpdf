# 아웃라인 (북마크)

## What it is
`/Outlines` 트리와 이름 있는 목적지를 읽고, 아웃라인을 설정·제거한다.

## Governing decisions
**None.**

## Design model
- `/Title`은 손실 디코드(UTF-16BE 미처리)되고 UTF-8 바이트로 쓰인다 — [텍스트 문자열 인코딩](../invariant/text-string-encoding.md).
- `visited`가 형제 체인 하나만 막고 자식 재귀는 막지 않는다(추론: 순환 위험).
- `/A`는 `/D`만 읽는다.

## Code
- `justpdf-core/src/outline/parse.rs` — `read_outlines`, `read_outline_siblings`, `read_named_destinations`
- `justpdf-core/src/outline/builder.rs` — `set_outlines`, `remove_outlines`
- `justpdf-core/src/outline/types.rs` — `Destination`, `OutlineItem`, `OutlineStyle`

## Reference behaviour
**None.** 비교 대상 조항: ISO 32000-2 §12.3.3.

## Cross-cutting invariants
- [텍스트 문자열 인코딩](../invariant/text-string-encoding.md)

## Blast radius
- [액션](actions.md) — `Destination` 공유, `/A` 중복 파싱.
- [문서 수정기](document-modifier.md) — 설정 경로.
- [병합](document-modifier.md) — 병합 시 아웃라인을 옮기는지 확인한 기록 없음.
- [파사드](facade.md) — `outlines`.

## Known holes / open
- 한글 제목 북마크를 만들면 다른 뷰어에서 깨진다(추론, 위 불변식).
- Tracked: #33 (텍스트 문자열 인코딩), #52 (순환 방어)
