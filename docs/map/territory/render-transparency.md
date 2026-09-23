# 렌더 — 투명도 그룹·소프트 마스크·블렌드

## What it is
ExtGState(`gs`)의 알파·블렌드 모드·소프트 마스크를 적용하고, 투명도 그룹 Form XObject를 임시 픽스맵에 그려 합성한다.

## Governing decisions
**None.**

## Design model
- 소프트 마스크는 Luminosity·Alpha만. 다른 서브타입은 조기 반환.
- `/BC`(배경색)는 무시하고 검정을 쓴다.
- `restore_clip_after_soft_mask` 본문이 비어 있다("we'll just leave the combined mask in place").
- 휘도 가중치는 Rec.709. [compress-grayscale](compress-grayscale.md)는 Rec.601.
- ExtGState의 `/Font`는 무시한다(텍스트 추출도 마찬가지).

## Code
- `justpdf-render/src/interpreter.rs` — `render_form_xobject`, `render_transparency_group`, `render_form_xobject_direct`, `apply_extgstate`, `apply_soft_mask`, `pixmap_to_mask`, `apply_soft_mask_to_device`, `restore_clip_after_soft_mask`
- `justpdf-render/src/graphics_state.rs` — `SoftMask`, `PdfBlendMode`

## Reference behaviour
**None.** 비교 대상 조항: ISO 32000-2 §11.

## Cross-cutting invariants
**None.**

## Blast radius
- [클리핑](render-clipping.md) — 마스크와 클립이 같은 장치 필드를 쓴다.
- [래스터 장치](raster-device.md) — `clip_mask` 직접 수정.
- [렌더 이미지](render-images.md) — SMask.

## Known holes / open
- 소프트 마스크 뒤 클립 복원 미구현(위).
- Tracked: #50 (클립 복원)
