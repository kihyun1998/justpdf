# 렌더 — 셰이딩

## What it is
`sh` 연산자와 셰이딩 패턴으로 타입 1–7 셰이딩(함수 기반, 축, 방사, 메시)을 래스터화한다.

## Governing decisions
**None.**

## Design model
- 함수 기반(타입 1)만 core `PdfFunction`을 쓴다. 축·방사는 `C0`/`C1`/`Bounds`를 손으로 다시 읽는다(타입 2·3 함수만) — [PDF 함수](pdf-functions.md)의 지원 범위와 별개다.
- 패치 메시는 베지어 제어점을 무시한다("approximation").
- 색 변환은 이름 전용 사본(`components_to_color`, `color_space_components`) — [이미지 픽셀 레이아웃](../invariant/image-pixel-layout.md)의 색 변환 사본 중 하나.
- 모르는 타입은 "unsupported"로 무시된다.

## Code
- `justpdf-render/src/shading.rs` — `render_shading`, `parse_gouraud_triangles`, `parse_lattice_vertices`, `parse_patch_mesh`, `rasterize_triangle`, `extract_stops_from_function`, `extract_colors_from_function`, `components_to_color`, `color_space_components`
- `justpdf-render/src/interpreter.rs` — `render_shading`, `render_shading_pattern`

## Reference behaviour
**None.** 비교 대상 조항: ISO 32000-2 §8.7.4.5.

## Cross-cutting invariants
- [이미지 픽셀 레이아웃](../invariant/image-pixel-layout.md) — 색 변환 사본.

## Blast radius
- [PDF 함수](pdf-functions.md) — 함수 파서(스트림 미디코드로 Type 4 실패 추론).
- [색공간](color-spaces.md) — 우회 중.
- [타일링 패턴](render-tiling-patterns.md) — 패턴 해석을 공유한다.
- [SVG 렌더러](svg-renderer.md) — 셰이딩을 건너뛴다.

## Known holes / open
- Tracked: #47 (함수 Type 0·Type 4)
