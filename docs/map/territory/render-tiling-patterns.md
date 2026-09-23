# 렌더 — 타일링 패턴

## What it is
`scn`/`SCN`으로 지정된 패턴을 해석해, 타일링 패턴은 타일을 그려 반복하고 셰이딩 패턴은 셰이딩으로 넘긴다.

## Governing decisions
**None.**

## Design model
- 경로 채우기·선 긋기만 패턴을 쓴다. 텍스트는 패턴으로 채우지 않는다(추론).

## Code
- `justpdf-render/src/interpreter.rs` — `resolve_pattern`, `render_tiling_pattern`, `try_fill_with_pattern`, `try_stroke_with_pattern`, `resolve_and_render_pattern`
- `justpdf-render/src/device.rs` — `fill_path_with_pattern`, `stroke_path_with_pattern`

## Reference behaviour
**None.** 비교 대상 조항: ISO 32000-2 §8.7.3.

## Cross-cutting invariants
**None.**

## Blast radius
- [셰이딩](render-shading.md) — 셰이딩 패턴.
- [래스터 장치](raster-device.md) — 패턴 채우기.
- [글리프 렌더링](glyph-rendering.md) — 패턴 텍스트 미지원.

## Known holes / open
**None.** 이 노트를 쓰며 발견된 구멍은 없다.
