# 렌더 인터프리터 (연산자 디스패치·그래픽 상태)

## What it is
`RenderInterpreter`가 페이지 콘텐츠를 가져와 연산자를 하나씩 실행하며 그래픽 상태(CTM, 색, 선, 텍스트 상태, 블렌드)를 유지하고 그리기 요청을 래스터 장치에 보낸다. `interpreter.rs`는 한 파일이지만 여러 개념을 담는다 — 이미지·클리핑·투명도·셰이딩·패턴·글리프·주석·OCG는 각자 노트가 있고, 이 노트는 드라이버와 디스패치, 그래픽 상태만 다룬다.

## Governing decisions
**None.** [ADR-0001](../../adr/0001-crates-split-by-dependency-layer.md)은 렌더가 `tiny-skia`를 끌어오는 별도 레이어라는 것만 정한다.

## Design model
- **장치 추상화가 없다**: `RenderInterpreter`는 `&mut PixmapDevice`를 직접 들고 있다(`trait` 정의 없음). SVG 출력은 장치가 아니라 인터프리터의 사본이다([SVG 렌더러](svg-renderer.md)).
- `execute_op`는 연산자 문자열에 대한 큰 `match` 하나다. 없는 연산자는 `_ => {}`로 조용히 무시된다(예: `ri`, `i`). Rendering Intent는 적용되지 않는다.
- `Tr`은 모드 3(보이지 않음)만 구분한다 — 윤곽선·클리핑 텍스트가 없다(추론).
- `d0`/`d1`은 no-op이다 — [Type3 폰트](type3-fonts.md) 미지원.
- `v`(베지어)는 현재점 추적 없이 근사한다("lossy without current point tracking").
- 색 연산자(`cs`/`CS`/`sc`/`scn`…)는 로컬 `cs_from_name`(Device Gray/RGB/CMYK만, 나머지는 RGB)을 쓴다 — 리소스의 색공간·ICC·Indexed·Separation은 해석하지 않는다.
- 페이지 콘텐츠를 자체 함수로 조립한다 — [페이지 콘텐츠 조립](../invariant/page-content-assembly.md).

## Code
- `justpdf-render/src/interpreter.rs` — `RenderInterpreter`, `render_page`, `get_page_content`, `concat_content_streams`, `resolve_object`, `execute_ops`, `execute_op`, `effective_transform`, `cs_from_name`
- `justpdf-render/src/graphics_state.rs` — `GraphicsState`, `TextState`, `Matrix`, `PdfBlendMode`, `fill_color_rgba`

## Reference behaviour
**None.** 비교 대상 조항: ISO 32000-2 §8.4(그래픽 상태), Annex A. MuPDF(example)와 렌더 결과를 비교한 기록 없음.

## Cross-cutting invariants
- [페이지 콘텐츠 조립](../invariant/page-content-assembly.md)
- [폰트 해석 경로](../invariant/font-resolution.md) — `resolve_page_fonts`가 텍스트 추출과 별도로 폰트를 푼다.

## Blast radius
- [래스터 장치](raster-device.md) — 유일한 출력 대상.
- [SVG 렌더러](svg-renderer.md), [bbox 장치](bbox-device.md) — 같은 디스패치의 사본. 연산자 처리를 고치면 사본들도 본다.
- [렌더 이미지](render-images.md), [클리핑](render-clipping.md), [투명도](render-transparency.md), [셰이딩](render-shading.md), [타일링 패턴](render-tiling-patterns.md), [글리프 렌더링](glyph-rendering.md), [렌더 주석](render-annotations.md), [optional content](optional-content.md) — 같은 파일 안의 하위 개념들.
- [콘텐츠 스트림 파싱](content-stream-parsing.md) — 입력.
- [색공간](color-spaces.md) — 우회하고 있는 core 모듈.
- [렌더 API](render-api.md) — 호출자.

## Known holes / open
- `interpreter.rs`, `graphics_state.rs`에 테스트가 없다. 렌더 통합 테스트는 PNG 매직 바이트·길이만 확인하고 픽셀 내용을 보지 않는다.
