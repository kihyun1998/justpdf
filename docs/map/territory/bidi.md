# BiDi (양방향 텍스트)

## What it is
`unicode-bidi`로 문자열의 방향 런을 계산하고 RTL 포함 여부를 판단한다.

## Governing decisions
- [ADR-0001](../../adr/0001-crates-split-by-dependency-layer.md)

## Design model
- 어떤 쓰기 경로도 이것을 쓰지 않는다 — RTL 텍스트를 PDF로 낼 때 재배열되지 않는다.

## Code
- `justpdf-special/src/bidi/mod.rs` — `resolve_bidi`, `contains_rtl`, `Direction`, `BidiRun`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [문서 빌더](document-builder.md), [텍스트 추출](text-extraction.md) — 연결될 후보(현재 없음).

## Known holes / open
- 저장소 안 소비처가 없다.
