# 렌더 — 타일링 패턴

## What it is
`scn`/`SCN`으로 지정된 패턴을 해석해, 타일링 패턴은 타일을 그려 반복하고 셰이딩 패턴은 셰이딩으로 넘긴다.

## Governing decisions
**None.**

## Design model
- `scn`/`SCN` resolves the pattern in the current scope and puts the object (`PatternSelection`) in the graphics state. The tile is drawn in the pattern's `/Resources` scope — [resource name scope](../invariant/resource-scope.md).
- 경로 채우기·선 긋기만 패턴을 쓴다. 텍스트는 패턴으로 채우지 않는다(추론).
- 타일은 실행 중인 스트림으로 기록된 채 그려지고, 타일 안에서 같은 패턴을 다시 칠하면 패턴 없이 채우기 색으로 칠한다. 타일은 페이지의 소프트 마스크 없이 그린다 — [콘텐츠 스트림 재귀](../invariant/content-stream-recursion.md).

## Code
- `justpdf-render/src/resources.rs` — `select_pattern`
- `justpdf-render/src/interpreter.rs` — `render_tiling_pattern`, `try_fill_with_pattern`, `try_stroke_with_pattern`, `render_pattern`
- `justpdf-render/src/device.rs` — `fill_path_with_pattern`, `stroke_path_with_pattern`

## Reference behaviour
**None.** 비교 대상 조항: ISO 32000-2 §8.7.3.

## Cross-cutting invariants
- [콘텐츠 스트림 재귀](../invariant/content-stream-recursion.md)
- [resource name scope](../invariant/resource-scope.md)

## Blast radius
- [셰이딩](render-shading.md) — 셰이딩 패턴.
- [래스터 장치](raster-device.md) — 패턴 채우기.
- [글리프 렌더링](glyph-rendering.md) — 패턴 텍스트 미지원.

## Known holes / open
- 페이지에서 패턴으로 채우거나 그을 때(`try_fill_with_pattern`, `try_stroke_with_pattern`) 소프트 마스크를 적용하지 않는다 — 단색 채우기만 `apply_soft_mask_to_device`를 거친다(2026-09-29 `tests/render_recursion.rs` 작성 중 관찰). Tracked: #129
