# 디스플레이 리스트와 타일 렌더링

## What it is
그리기 명령을 기록해 두었다가 재생·변환 재생·최적화·타일 단위 렌더링을 하는 구조.

## Governing decisions
**None.**

## Design model
- 인터프리터가 여기에 기록하지 않는다. 파일 밖 사용처가 `lib.rs`의 `pub mod display_list`뿐이다.

## Code
- `justpdf-render/src/display_list.rs` — `DisplayList`, `DisplayCommand`, `replay`, `replay_with_transform`, `optimize`, `render_tile`, `render_tiled`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [렌더 인터프리터](render-interpreter.md) — 연결되어야 할 쪽.
- [렌더 API](render-api.md) — `parallel`은 페이지 단위 병렬이지 타일 단위가 아니다.

## Known holes / open
- 타일 렌더링(`DisplayList::render_tile`)이 제품 경로에 연결되지 않았다.
