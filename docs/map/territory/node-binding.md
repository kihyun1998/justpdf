# Node.js 바인딩

## What it is
napi-rs로 `Document`(열기·버퍼·인증·텍스트·렌더·크기·Info)를 노출한다.

## Governing decisions
- [ADR-0002](../../adr/0002-language-bindings-outside-workspace.md) — ADR대로 분리되어 있다.

## Design model
- `build.rs`가 `napi_build::setup`을 부르고 `package.json`이 `napi build`를 쓴다.

## Code
- `justpdf-node/src/lib.rs` — `Document`, `open`, `from_buffer`, `authenticate`, `text`, `page_text`, `render_page`, `page_width`, `page_height`
- `justpdf-node/build.rs` — `setup`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [문서 접근](document-access.md), [텍스트 추출](text-extraction.md), [렌더 API](render-api.md).
- [CI](ci.md) — 빌드되지 않는다.

## Known holes / open
- 자체 `Cargo.lock`이 루트와 따로 움직인다.
