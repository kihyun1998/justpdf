# 래스터 장치 (tiny-skia)

## What it is
`PixmapDevice`가 tiny-skia 픽스맵에 경로 채우기·선 긋기·이미지·패턴을 그리고, 클립 마스크를 들고, PNG/JPEG/RGBA로 인코딩한다.

## Governing decisions
**None.**

## Design model
- `clip_mask`가 `pub(crate)`이고 인터프리터가 소프트 마스크 적용 시 직접 바꾼다 — 장치 경계를 인터프리터가 넘는다.
- `encode_png`는 이 내부 타입에만 있다. 공개 반환형(`RenderedPixmap`, `Vec<u8>`)에는 없다(README 예제가 이것을 부른다 — [게시 문서](published-docs.md)).

## Code
- `justpdf-render/src/device.rs` — `PixmapDevice`, `fill_path`, `stroke_path`, `draw_image`, `draw_pixmap`, `fill_path_with_pattern`, `set_clip_path`, `intersect_clip_path`, `clear_clip`, `encode_png`, `encode_jpeg`, `raw_rgba`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [렌더 인터프리터](render-interpreter.md) — 유일한 호출자.
- [클리핑](render-clipping.md), [투명도](render-transparency.md) — 클립 마스크 공유.
- [렌더 API](render-api.md) — 인코딩 출력.

## Known holes / open
- 테스트가 없다.
