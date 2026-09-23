# 색공간과 ICC

## What it is
PDF 색공간 모델(Device*, CalGray/CalRGB, Lab, Indexed, Separation, DeviceN, ICCBased)과 색 값을 RGB로 바꾸는 변환, ICC 프로파일 파서와 sRGB 변환, OutputIntent·오버프린트 파서.

## Governing decisions
**None.**

## Design model
- `from_array`는 ICCBased를 항상 `num_components: 3`, `profile: None`으로 만든다(주석: "actual value requires reading the stream").
- `Color::to_rgb`는 Lab·Indexed·Separation·DeviceN·프로파일 없는 ICCBased에 대해 검정을 돌려준다. CalGray/CalRGB는 Device로 취급한다.
- **ICC·OutputIntent·오버프린트는 제품 경로에서 도달 불가**다: `parse_icc_profile`, `read_output_intents`, `parse_overprint`의 호출처는 테스트뿐이고, `icc_to_srgb`는 프로파일이 세팅된 `to_rgb`로만 도달하는데 프로파일은 세팅되지 않는다.
- CMYK→RGB 변환이 이 모듈(`cmyk_to_rgb`) 외에 렌더러·SVG·셰이딩·압축에 각자 사본으로 있다 — [이미지 픽셀 레이아웃](../invariant/image-pixel-layout.md).

## Code
- `justpdf-core/src/color/mod.rs` — `ColorSpace`, `from_pdf_object`, `from_array`, `num_components`, `Color`, `to_rgb`, `cmyk_to_rgb`, `rgb_to_cmyk`, `OutputIntent`, `read_output_intents`, `parse_overprint`
- `justpdf-core/src/color/icc.rs` — `IccProfile`, `parse_icc_profile`, `icc_to_srgb`, `RenderingIntent`

## Reference behaviour
**None.** 코드는 ICC.1:2004 / ISO 15076-1, OutputIntent §14.11.5, Overprint §8.6.7을 인용한다(비교 기록 아님). 비교 대상 조항: ISO 32000-2 §8.6.

## Cross-cutting invariants
- [이미지 픽셀 레이아웃](../invariant/image-pixel-layout.md) — 색 변환 사본의 원본.

## Blast radius
- [렌더 인터프리터](render-interpreter.md) — `cs`/`CS`가 로컬 `cs_from_name`(이름만)을 써서 이 모듈의 배열 색공간을 우회한다. 색공간 지원을 늘리면 거기부터.
- [SVG 렌더러](svg-renderer.md), [렌더 셰이딩](render-shading.md) — 각자의 이름 전용 사본.
- [이미지 디코딩](image-decoding.md) — 이미지 색공간을 이름으로만 읽는다.
- [compress-images](compress-images.md) — CMYK는 건너뛰고 `to_rgb_pixels`로 자체 변환.

## Known holes / open
- `test_indexed`는 성분 수·base·hival만 확인한다. Indexed 팔레트에서 실제 색을 조회하는 코드가 없다.
- Rendering Intent는 파싱만 되고 어디서도 적용되지 않는다.
