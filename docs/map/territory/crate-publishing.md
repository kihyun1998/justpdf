# 크레이트 배포와 버전

## What it is
crates.io(각 크레이트), npm(compress-wasm), PyPI·npm(바인딩)으로의 게시와, 크레이트 간 버전 요구·CHANGELOG. 저장소 워크플로에 게시 자동화는 없고 손으로 한다.

## Governing decisions
**None.** [ADR-0002](../../adr/0002-language-bindings-outside-workspace.md)가 바인딩은 각자 다른 레지스트리·파이프라인을 가진다고 적을 뿐, 버전 정책은 정하지 않는다.

## Design model
- 크레이트들이 제각기 버전을 가진다. 하위 크레이트는 core에 캐럿 요구(`version = "…", path = …`)를 적는다 — core가 패치 버전을 올려도 요구 문자열은 옛 값에 머문다(캐럿이라 만족은 된다). 현재 값은 각 `Cargo.toml`이 소유한다: `grep -n 'justpdf-core' */Cargo.toml`.
- `justpdf-compress-wasm`은 core를 `path`로만 의존하고 `version`이 없으며 `publish = false`도 없다 — `cargo publish`가 거부할 형태다(추론, 미실행).
- CHANGELOG의 `[Unreleased]`가 0.1.4 이후 병합분을 모은다. 크레이트별 버전과 CHANGELOG 헤딩의 대응(어느 항목이 어느 크레이트 릴리스인지)은 헤딩의 괄호 주석에만 있다.

## Code
- `CHANGELOG.md` — `Unreleased`
- `justpdf-compress-wasm/Cargo.toml` — `justpdf-core`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [compress-wasm](compress-wasm.md) — 게시 불가 의존성, npm 패키지.
- [언어 바인딩](language-bindings.md) — 각자 레지스트리.
- [릴리스](release.md) — 바이너리 버전.
- [게시 문서](published-docs.md) — 크레이트 README가 crates.io 페이지가 된다.

## Known holes / open
- 게시 절차가 어디에도 기록되어 있지 않다.
