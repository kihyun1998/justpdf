# 압축 — 그레이스케일 변환

## What it is
이미지를 회색조 JPEG(고정 품질 65)로 바꾸고, 페이지 콘텐츠 스트림의 색 연산자를 회색 연산자로 다시 쓴다.

## Governing decisions
**None.**

## Design model
- `rg`/`RG`/`k`/`K`만 재작성한다. `sc`/`SC`/`scn`/`SCN`은 그대로 둔다.
- 페이지 `/Contents`만 다시 쓴다. Form XObject 안의 색은 그대로다.
- 콘텐츠를 다시 쓸 때 `write_operand`로 연산자 텍스트를 손으로 만든다: 이름을 `#XX`로 이스케이프하지 않고, literal 문자열의 CR을 그대로 쓰며, 인라인 이미지는 `"BI "`만 남긴다("rare in practice") — [객체 구문 왕복](../invariant/object-syntax-roundtrip.md).
- 회색 가중치는 Rec.601이고, 렌더러의 소프트 마스크는 Rec.709다 — 두 사이트가 서로 다른 휘도식을 쓴다.
- 어떤 프리셋도 이 단계를 켜지 않는다([프리셋](compress-presets.md)).

## Code
- `justpdf-core/src/writer/compress.rs` — `convert_images_to_grayscale`, `encode_jpeg_gray`, `rewrite_color_operators_to_gray`, `rgb_to_gray_from_operands`, `cmyk_to_gray_from_operands`, `write_real`, `write_operand`

## Reference behaviour
**None.**

## Cross-cutting invariants
- [객체 구문 왕복](../invariant/object-syntax-roundtrip.md) — `write_operand`.
- [이미지 픽셀 레이아웃](../invariant/image-pixel-layout.md) — 이미지 경로.

## Blast radius
- [콘텐츠 스트림 파싱](content-stream-parsing.md) — `Operand` 구조를 읽고 다시 쓴다. `Operand` 변형이 늘면 `write_operand`도 늘려야 한다.
- [compress-images](compress-images.md) — 별도 이미지 인코딩 경로.
- [렌더 투명도](render-transparency.md) — 휘도식 불일치의 다른 쪽.

## Known holes / open
- 인라인 이미지가 있는 페이지는 재작성 후 깨진다(추론, 테스트 없음).
- `test_grayscale_conversion_reduces_size`는 `if stats.images_grayscaled > 0` 조건부 탈출이 있어 변환이 0건이어도 통과한다.
- Tracked: #29 (손으로 쓰는 구문 이스케이프)
