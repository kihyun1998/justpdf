# CI

## What it is
push·PR마다 도는 GitHub Actions 작업: 워크스페이스 check, test, 전체 기능 test, 기능 격리 스크립트, clippy, fmt.

## Governing decisions
**None.**

## Design model
- master CI는 2026-05부터 기능 격리 작업이 실패 상태였다(트리 문자 문제, #62에서 수정). 그 사이 병합된 PR들은 빨간 CI로 병합됐다 — 실패가 상시 상태가 되면 게이트가 아니다.
- **clippy(`-D warnings`)와 fmt는 `continue-on-error: true`** — 실패해도 빌드가 깨지지 않는다. 사실상 게이트가 아니다.
- 모든 cargo 작업이 `--workspace` 범위다. 워크스페이스 밖 크레이트(python·wasm·node)와 wasm32·maturin·napi 빌드는 없다 — "통과"가 이 크레이트들을 검사했다는 뜻이 아니다.
- 전체 기능 test는 `justpdf/mmap`을 포함하지 않는다.
- mdBook(`docs/`)을 빌드하는 작업이 없다.

## Code
- `.github/workflows/ci.yml` — `check`, `test`, `test-features`, `feature-isolation`, `clippy`, `fmt`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [언어 바인딩](language-bindings.md) — 게이트 밖.
- [크레이트 레이어링](crate-layering.md) — 격리 스크립트.
- [게시 문서](published-docs.md) — 문서 예제가 컴파일·실행되는지 아무것도 보지 않는다.
- [릴리스](release.md) — 릴리스 워크플로는 CI 통과를 전제하지 않는다(태그 push만).

## Known holes / open
- 게이트의 "전체"가 저장소 전체가 아니다(위).
- Tracked: #59 (CI 게이트 공백)
