# 이미지 픽셀 레이아웃

## The fact
`decode_image`가 돌려주는 픽셀 버퍼의 의미 — 성분 수, 성분당 비트 수, 색공간, 마스크의 비트 패킹 — 는 이미지 사전과 디코드 경로(DCT/JPX/JBIG2/CCITT/원시)에 따라 다르다. 버퍼를 RGB(A)로 해석하는 모든 곳은 **같은 규칙**으로 해석해야 하며, 그 규칙은 `/BitsPerComponent`, 배열 색공간(Indexed, ICCBased, Separation), `/Decode`, 그리고 경로별 출력 형태(예: CCITT·JBIG2는 픽셀당 1바이트)를 따라야 한다.

## Why it is cross-cutting
디코드는 한 곳이지만 **해석은 여러 곳에 각자 사본**으로 있고, 서로 호출하지 않는다. CMYK→RGB 변환만 해도 core `cmyk_to_rgb` 외에 렌더러·SVG·셰이딩·압축에 사본이 있다. 모두가 암묵적으로 "8비트, 성분 1/3/4, 이름 색공간"을 가정한다. 한 사본을 고쳐도 나머지는 그대로다.

## Territories it holds in
- [이미지 디코딩](../territory/image-decoding.md) — 출발점(`decode_image`, 색공간은 이름만 읽음).
- [렌더 이미지](../territory/render-images.md) — `image_to_rgba`, 마스크 1비트 풀이(CCITT·JBIG2 출력과 불일치).
- [SVG 렌더러](../territory/svg-renderer.md) — `image_to_rgba` 사본, `cs_from_name` 사본.
- [렌더 셰이딩](../territory/render-shading.md) — `components_to_color`(색 변환 사본).
- [compress-images](../territory/compress-images.md) — `to_rgb_pixels`(CMYK 사본, f32).
- [compress-grayscale](../territory/compress-grayscale.md) — 휘도식(Rec.601; 렌더러 마스크는 Rec.709).
- [색공간](../territory/color-spaces.md) — 원본 `cmyk_to_rgb`와 쓰이지 않는 배열 색공간 모델.

다시 찾는 명령: `rg -n 'fn image_to_rgba|fn to_rgb_pixels|fn components_to_color|fn cs_from_name|fn cmyk_to_rgb' --glob '*.rs' --glob '!target' .`

## What a violation looks like
- Indexed 이미지가 렌더에서 엉뚱한 색으로, 압축에서는 조용히 건너뛰어진다.
- 16비트·1/2/4비트 원시 이미지가 줄무늬나 잘린 이미지로 보인다.
- CCITT·JBIG2 이미지 마스크가 잘못 풀린다.
- 같은 이미지가 렌더와 압축 후 렌더에서 다른 색이다.
조건: 8비트 DeviceRGB/Gray JPEG에서는 전부 멀쩡하다 — 흔한 테스트 입력이 이 조건이라 보이지 않는다.

## Discovery history
기록된 사고가 없다. 2026-09-23 맵 작성 중 두 연구 에이전트(스트림/이미지/색, 렌더)가 사본들을 독립적으로 보고했다. 모두 코드 읽기에 의한 추론이며 `decode_image`에는 단위 테스트가 없다.

- Tracked: #42 (렌더 인라인 이미지·마스크), #46 (이미지 디코드 색공간·Decode)

## Where it will recur
**`DecodedImage`의 바이트를 인덱싱하거나 색 성분을 RGB로 바꾸는 함수는 이 불변식의 대상이다.** 새로 쓰기 전에 위 명령으로 기존 사본을 찾고, 가능하면 [색공간](../territory/color-spaces.md)의 함수를 쓴다. 테스트 입력에 Indexed·16비트·CMYK·CCITT를 넣지 않으면 이 불변식은 검사되지 않는다.
