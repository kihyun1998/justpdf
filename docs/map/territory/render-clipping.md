# 렌더 — 클리핑

## What it is
`W`/`W*` 클리핑 경로를 장치의 클립 마스크에 적용하고 q/Q에서 복원한다.

## Governing decisions
**None.**

## Design model
- 클립은 그래픽 상태가 아니라 **장치에 산다**. `GraphicsState`는 `has_clip: bool`만 가진다.
- `Q`는 복원된 상태에 클립이 없을 때만 장치 클립을 지운다("simplified: just clear for now"). 부모도 클립이 있었다면 안쪽 클립이 `Q` 뒤에 살아남는다(추론).

## Code
- `justpdf-render/src/interpreter.rs` — `apply_clip`, `fill_current_path`, `stroke_current_path`
- `justpdf-render/src/device.rs` — `set_clip_path`, `intersect_clip_path`, `clear_clip`

## Reference behaviour
**None.** 비교 대상 조항: ISO 32000-2 §8.5.4.

## Cross-cutting invariants
**None.**

## Blast radius
- [래스터 장치](raster-device.md) — 클립 보관 위치.
- [투명도](render-transparency.md) — 소프트 마스크 적용 후 클립 복원(`restore_clip_after_soft_mask`)이 비어 있다.
- [렌더 인터프리터](render-interpreter.md) — q/Q 스택.

## Known holes / open
- 중첩 클립이 `Q`에서 복원되지 않는다(위, 추론; 테스트 없음).
- Tracked: #50 (클립 복원)
