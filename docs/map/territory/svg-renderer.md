# SVG 렌더러

## What it is
페이지를 SVG 문서로 출력한다. 장치가 아니라 **두 번째 인터프리터**다: 자체 연산자 디스패치·폰트 해석·콘텐츠 조립·XObject·이미지·ExtGState 처리를 가지고, 래스터 쪽과는 `graphics_state` 타입만 공유한다.

## Governing decisions
**None.**

## Design model
- 인라인 이미지(`BI`)는 건너뛰고, 셰이딩(`sh`)은 "skip for now", marked content는 전부 무시(OCG 없음), 패턴 이름은 기록만 하고 쓰지 않는다. 주석·소프트 마스크 없음. `defs` 주석은 "gradients"를 말하지만 그라디언트 코드가 없다.
- 텍스트는 ToUnicode로 `<text>`를 내고, 없으면 ASCII(<128)만, 윤곽이 없으면 "rectangle placeholder".
- `image_to_rgba`·`cs_from_name`의 사본을 가진다 — [이미지 픽셀 레이아웃](../invariant/image-pixel-layout.md).

## Code
- `justpdf-render/src/svg_device.rs` — `SvgRenderer`, `execute_op`, `resolve_page_fonts`, `get_page_content`, `concat_content_streams`, `do_xobject`, `render_image`, `render_form_xobject`, `apply_extgstate`, `image_to_rgba`, `cs_from_name`, `render_text_string`

## Reference behaviour
**None.**

## Cross-cutting invariants
- [이미지 픽셀 레이아웃](../invariant/image-pixel-layout.md)
- [페이지 콘텐츠 조립](../invariant/page-content-assembly.md)
- [폰트 해석 경로](../invariant/font-resolution.md)

## Blast radius
- [렌더 인터프리터](render-interpreter.md) — 원본. 래스터 쪽에서 고친 연산자 동작은 여기로 전파되지 않는다.
- [이미지 디코딩](image-decoding.md), [ToUnicode](tounicode.md), [색공간](color-spaces.md) — 입력.
- [렌더 API](render-api.md) — `render_page_to_svg`.
- [CLI](cli.md) — `render -F svg`, `convert` SVG 출력.

## Known holes / open
- 테스트는 `<svg` 포함 여부만 확인한다.
