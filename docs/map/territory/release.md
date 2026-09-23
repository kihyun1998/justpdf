# 릴리스 (태그 → CLI 바이너리)

## What it is
`v*` 태그 push가 유일한 트리거. 네 타깃(Linux x86_64, macOS x86_64·aarch64, Windows x86_64)에서 `justpdf-cli`만 릴리스 빌드해 README·라이선스와 함께 묶고, 같은 이름의 GitHub Release에 첨부한다.

## Governing decisions
- [ADR-0003](../../adr/0003-cli-is-end-product-not-a-dependency-layer.md) — CLI가 단일 완제품이므로 바이너리 배포 대상이 CLI 하나다.

## Design model
- crates.io·PyPI·npm 게시는 이 워크플로에 없다([크레이트 배포](crate-publishing.md)).
- 태그와 크레이트 버전의 일치를 검사하지 않는다.

## Code
- `.github/workflows/release.yml` — `build`, `softprops/action-gh-release`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [CLI](cli.md) — 배포 대상.
- [크레이트 배포](crate-publishing.md) — 버전 번호의 출처.
- [게시 문서](published-docs.md) — 아카이브에 루트 README가 들어간다(예제가 틀린 README).

## Known holes / open
**None.** 이 노트를 쓰며 발견된 구멍은 없다.
