# 읽기 순서 (레이아웃 분석)

## What it is
추출된 줄을 단(column)으로 나누고 블록으로 묶고(하이픈 연결 해제 포함), 읽기 순서로 정렬한다.

## Governing decisions
**None.**

## Design model
- 단 경계 간격 기준: `(page_width * 0.15).max(30.0)`.
- 블록 경계: 평균 글꼴 크기의 2배.

## Code
- `justpdf-core/src/text/layout.rs` — `detect_columns_and_reorder`, `detect_column_boundaries`, `group_into_blocks_with_dehyphenation`, `build_block_dehyphenated`, `reading_order_sort`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [텍스트 추출](text-extraction.md) — 호출자.
- [출력 형식](text-output-formats.md) — 블록 구조를 그대로 출력한다.

## Known holes / open
**None.** 이 노트를 쓰며 발견된 구멍은 없다.
