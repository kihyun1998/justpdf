# 게시 문서 (README·크레이트 README·mdBook)

## What it is
저장소 밖 사람이 읽는 문서 표면: 루트 README(GitHub·릴리스 아카이브), 각 크레이트 README(crates.io 페이지), `docs/` mdBook 가이드. 코드가 바뀌어도 아무것도 이 문서를 검사하지 않는다.

## Governing decisions
**None.**

## Design model
- 2026-09-23에 README·크레이트 README·mdBook의 예제와 기능 목록을 코드에 맞췄다. Rust 예제는 저장소 밖 임시 크레이트에서 `cargo check`로 컴파일해 확인했고, CLI 예제는 빌드한 바이너리의 `--help`와 대조했다. Python·Node·WASM·C 예제는 바인딩 소스와 헤더를 읽어 대조했다(실행하지 않음).
- **그 확인은 한 번뿐이다.** 문서 예제를 컴파일하거나 실행하는 장치(doctest, mdBook 빌드, CI 작업)가 없으므로, 공개 API가 바뀌면 문서는 다시 조용히 어긋난다.
- 알려진 미구현은 문서에 그대로 적었다(예: CLI `sign`은 아무것도 하지 않음, 표준 폰트는 ASCII만).
- mdBook은 CI·README 어디서도 빌드·게시되지 않는다.
- 저장소 안의 다른 산문 문서(`dev/pdf-compress-wasm-design.md`, `docs/mupdf-feature-analysis.md`, `CHANGELOG.md`)도 같은 날 사실과 다른 문장을 고쳤다. 옛 `roadmap.md`는 코드와 맞지 않아 삭제했다.

## Code
- `README.md` — `render_png`
- `docs/book.toml` — `book`
- `docs/src/SUMMARY.md` — 목차

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- 공개 API를 바꾸는 모든 노트 — [파사드](facade.md), [CLI](cli.md), [언어 바인딩](language-bindings.md), [포맷 변환 계약](format-document.md), [문서 수정기](document-modifier.md). 이 맵에서 문서 표면으로 가는 간선은 여기로 모인다.
- [CI](ci.md) — doctest·mdBook 빌드가 없다.
- [릴리스](release.md) — 루트 README가 아카이브에 들어간다.

## Known holes / open
- 예제를 컴파일하는 장치가 없다. 가장 싼 게이트는 README의 Rust 예제를 `justpdf/examples/`로 옮기거나 doctest로 만드는 것이다.
- Tracked: #59 (CI 게이트 공백)
