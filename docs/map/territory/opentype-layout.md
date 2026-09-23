# OpenType 레이아웃 테이블 (GSUB/GPOS)

## What it is
GSUB(타입 1–4)와 GPOS(페어 조정, 타입 2)를 파싱해 치환 룩업과 커닝 쌍을 만든다.

## Governing decisions
**None.**

## Design model
- GPOS 페어 조정 Format 2(클래스 기반)는 건너뛴다("skip for now, store nothing").

## Code
- `justpdf-core/src/font/opentype.rs` — `parse_opentype_layout`, `GsubLookup`, `GposLookup`, `KerningPair`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [텍스트 줄바꿈](text-wrapping.md), [문서 빌더](document-builder.md) — 커닝·합자를 적용할 쓰기 쪽 후보(현재 연결 없음).

## Known holes / open
- 제품 코드 소비처가 없다.
