# 이미지 디코딩

## What it is
이미지 XObject의 사전(`image_info`)과 데이터를 받아 픽셀(`DecodedImage`)로 바꾼다. DCT는 `jpeg-decoder`, JPX는 `justjp2`, JBIG2는 `justbig2`, CCITT는 core 디코더, 그 외는 스트림 디코드 결과를 그대로. 렌더러와 압축기가 공유하는 **유일한** 이미지 디코드 경로다.

## Governing decisions
**None.**

## Design model
- 분기는 필터 체인의 **마지막 필터**로 고르고, 원바이트를 해당 디코더에 그대로 넘긴다. `[/FlateDecode /DCTDecode]` 같은 체인이면 압축된 바이트가 JPEG 디코더로 간다(추론).
- `/ColorSpace`는 이름일 때만 읽는다. 배열·참조(Indexed, ICCBased 등)는 DeviceRGB 3성분으로 떨어진다. [색공간](color-spaces.md)의 `from_pdf_object`를 쓰지 않는다.
- `/Decode`는 적용하지 않는다. `SMask`/`ImageMask`는 플래그만 세운다.
- JBIG2: `JBIG2Globals`를 읽지 않는다. 출력은 8비트 회색으로 펼친다(1 → 0x00).
- JPX: 성분을 i32 → u8로 자르고 비트 깊이를 무시한다. `SMaskInData` 미처리.
- 출력 픽셀의 의미(성분 수, 비트 깊이)는 호출자마다 다르게 해석된다 — [이미지 픽셀 레이아웃](../invariant/image-pixel-layout.md).

## Code
- `justpdf-core/src/image/mod.rs` — `ImageInfo`, `image_info`, `DecodedImage`, `ImageFormat`, `decode_image`, `extract_jpeg_bytes`

## Reference behaviour
**None.** 비교 대상 조항: ISO 32000-2 §8.9(이미지), §8.9.5(Decode 배열), §11.6.5.3(SMask).

## Cross-cutting invariants
- [이미지 픽셀 레이아웃](../invariant/image-pixel-layout.md) — 이 함수의 출력이 불변식의 출발점이다.

## Blast radius
- [렌더 이미지](render-images.md), [SVG 렌더러](svg-renderer.md) — `decode_image` 소비처. 출력 형태를 바꾸면 각자의 `image_to_rgba`를 본다.
- [compress-images](compress-images.md), [compress-grayscale](compress-grayscale.md) — 같은 출력의 다른 해석자.
- [스트림 필터](stream-filters.md) — 통과 규칙의 짝.
- [색공간](color-spaces.md) — 배열 색공간을 지원하려면 여기서 `from_pdf_object`를 써야 한다.

## Known holes / open
- `decode_image`에 단위 테스트가 없다(JPX/JBIG2/CCITT/마스크 전부).
- JPX·JBIG2는 스트림 계층에서 원바이트로 통과하고 이미지 계층에서만 픽셀로 디코드된다. `decode_stream`을 부르는 쪽은 픽셀을 받지 못한다.
- Tracked: #46 (이미지 디코드 색공간·Decode)
