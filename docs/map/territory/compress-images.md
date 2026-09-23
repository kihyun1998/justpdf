# 압축 — 이미지 재인코딩과 다운스케일

## What it is
이미지 XObject를 디코드해 RGB로 바꾸고, 페이지에서 실제로 그려지는 크기(CTM)로 목표 DPI를 계산해 줄이고, JPEG로 다시 인코딩한다. 결과가 더 크면 교체하지 않는다.

## Governing decisions
**None.**

## Design model
- 마스크·SMask가 있는 이미지·CMYK는 건너뛴다(`should_skip_image`).
- 여러 곳에서 쓰인 이미지는 **가장 크게 그려지는 크기**를 기준으로 한다. CTM 정보가 없으면 `max_dpi*14` 픽셀 예산으로 떨어진다.
- 교체된 이미지 사전은 DeviceRGB/DCTDecode로 **새로 만든다** — 원래 사전의 다른 키(Decode, Intent 등)는 사라진다.
- `jpeg_quality`가 없으면 75로 둔다. 그래서 DPI만 설정해도 재인코딩이 일어난다.
- 픽셀 해석은 `to_rgb_pixels`(CMYK→RGB 네 번째 사본)가 하며, 8비트/성분을 가정한다 — [이미지 픽셀 레이아웃](../invariant/image-pixel-layout.md).

## Code
- `justpdf-core/src/writer/compress.rs` — `recompress_images`, `should_skip_image`, `compute_target_dimensions_with_ctm`, `compute_target_dimensions`, `collect_image_display_sizes`, `multiply_matrix`, `extract_xobject_map`, `to_rgb_pixels`, `encode_jpeg_rgb`

## Reference behaviour
**None.** Ghostscript 다운샘플링과 결과를 비교한 기록이 없다.

## Cross-cutting invariants
- [이미지 픽셀 레이아웃](../invariant/image-pixel-layout.md) — `decode_image` 출력을 자기 방식으로 해석하는 사이트.

## Blast radius
- [이미지 디코딩](image-decoding.md) — `image_info`/`decode_image` 출력 형태가 바뀌면 여기가 먼저 깨진다.
- [콘텐츠 스트림 파싱](content-stream-parsing.md) — CTM 수집이 연산자 파싱에 기댄다.
- [compress-dedup](compress-dedup.md) — 교체 뒤에 돈다. 같은 원본을 공유한 이미지가 각각 재인코딩된 뒤 합쳐질 수 있다.
- [compress-grayscale](compress-grayscale.md) — 별도 이미지 경로(고정 q65)를 가진다. 한쪽 규칙을 바꾸면 다른 쪽도 본다.
- [프리셋](compress-presets.md) — `jpeg_quality`/`max_image_dpi`/`skip_below_bytes`.

## Known holes / open
- `compute_target_dimensions`는 `#[allow(dead_code)]`이고 테스트만 부른다.
- 16비트·1/2/4비트·Indexed 이미지 처리가 가정 밖이다(위 불변식).
