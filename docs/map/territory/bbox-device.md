# bbox 계산

## What it is
페이지에 실제로 그려지는 내용의 경계 상자를 계산한다. 세 번째 연산자 순회기(단순화된 버전)다.

## Governing decisions
**None.**

## Design model
- 자체 콘텐츠 조립("simplified version")과 연산자 순회를 가진다.

## Code
- `justpdf-render/src/bbox_device.rs` — `BBoxDevice`, `process_ops`, `get_page_content`, `concat_streams`, `compute_page_bbox`

## Reference behaviour
**None.**

## Cross-cutting invariants
- [페이지 콘텐츠 조립](../invariant/page-content-assembly.md)

## Blast radius
- [렌더 인터프리터](render-interpreter.md) — 디스패치 원본.

## Known holes / open
- `compute_page_bbox`로 공개되지만 워크스페이스·바인딩 어디서도 호출되지 않는다.
