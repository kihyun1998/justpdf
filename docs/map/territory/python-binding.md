# Python 바인딩

## What it is
PyO3 모듈(maturin 빌드). `Document`(열기·인증·텍스트·렌더·Info)와 크기·회전만 가진 `Page`.

## Governing decisions
- [ADR-0002](../../adr/0002-language-bindings-outside-workspace.md) — 빈 `[workspace]`와 자체 `Cargo.lock`으로 ADR대로 분리되어 있다.

## Design model
- `Page`에는 텍스트 메서드가 없다(텍스트는 `Document.page_text`).

## Code
- `justpdf-python/src/lib.rs` — `Document`, `open`, `from_bytes`, `authenticate`, `page`, `text`, `page_text`, `render_page`, `render_page_to_file`, `Page`
- `justpdf-python/pyproject.toml` — `maturin`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [문서 접근](document-access.md), [텍스트 추출](text-extraction.md), [렌더 API](render-api.md).
- [게시 문서](published-docs.md) — 루트 README·mdBook의 Python 예제.
- [CI](ci.md) — 빌드되지 않는다.

## Known holes / open
- 자체 `Cargo.lock`이 루트와 따로 움직인다 — core 의존성 업데이트가 여기엔 반영되지 않는다.
