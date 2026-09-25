# 압축 — 그레이스케일 변환

## What it is
이미지를 회색조 JPEG(고정 품질 65)로 바꾸고, 페이지 콘텐츠 스트림의 색 연산자를 회색 연산자로 다시 쓴다.

## Governing decisions
**None.**

## Design model
- `rg`/`RG`/`k`/`K`만 재작성한다. `sc`/`SC`/`scn`/`SCN`은 그대로 둔다.
- 페이지 `/Contents`만 다시 쓴다. Form XObject 안의 색은 그대로다.
- 색 연산자가 아닌 연산자는 `ContentOp::write_to`로 다시 쓴다 — 되읽으면 같다(`test_grayscale_keeps_other_operators_unchanged`, #29). #29 전에는 자체 `write_operand`가 이름을 이스케이프하지 않고, 실수를 소수 4자리로 반올림하고, 인라인 이미지를 `"BI "`만 남겼다. 새 회색 값은 `write_gray`로 쓴다 — 소수 4자리로 반올림한 뒤 객체 직렬화의 `write_real`로(항상 소수점, NaN → `0.0`, ±Inf → ±`f32::MAX`). #29 전에는 정수에 가까우면 정수로, 아니면 `{:.4}`로 써서 inf 입력이 `inf g`가 되었다 — [객체 구문 왕복](../invariant/object-syntax-roundtrip.md).
- 회색 가중치는 Rec.601이고, 렌더러의 소프트 마스크는 Rec.709다 — 두 사이트가 서로 다른 휘도식을 쓴다.
- 어떤 프리셋도 이 단계를 켜지 않는다([프리셋](compress-presets.md)).

## Code
- `justpdf-core/src/writer/compress.rs` — `convert_images_to_grayscale`, `encode_jpeg_gray`, `rewrite_color_operators_to_gray`, `rgb_to_gray_from_operands`, `cmyk_to_gray_from_operands`, `write_gray`

## Reference behaviour
**None.**

## Cross-cutting invariants
- [객체 구문 왕복](../invariant/object-syntax-roundtrip.md) — `ContentOp::write_to`.
- [이미지 픽셀 레이아웃](../invariant/image-pixel-layout.md) — 이미지 경로.

## Blast radius
- [콘텐츠 스트림 파싱](content-stream-parsing.md) — 파싱하고 그쪽의 `write_to`로 다시 쓴다.
- [compress-images](compress-images.md) — 별도 이미지 인코딩 경로.
- [렌더 투명도](render-transparency.md) — 휘도식 불일치의 다른 쪽.

## Known holes / open
- `test_grayscale_conversion_reduces_size`는 `if stats.images_grayscaled > 0` 조건부 탈출이 있어 변환이 0건이어도 통과한다.
- 일부 이미지에서 패닉한다("Invalid buffer length", 저장소 루트의 brochure·test_medium PDF). Tracked: #89
