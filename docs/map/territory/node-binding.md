# Node binding

## What it is
Exposes `Document` (open from a path, from a Buffer, authenticate, page count, version, text, PNG render, page size, Title/Author) through napi-rs (crate `justpdf-node`, npm package name `justpdf` in `package.json`).

## Governing decisions
- [ADR-0002](../../adr/0002-language-bindings-outside-workspace.md) — kept separate as the ADR says: an empty `[workspace]` in `justpdf-node/Cargo.toml` and its own `Cargo.lock` (re-checked 2026-10-30).

## Design model
Status: only from reading the source; no test in the repo runs it.
- `build.rs` calls `napi_build::setup`, and `package.json` builds with `napi build --release`. napi-rs exposes the snake_case Rust names in camelCase (`pageCount`, `renderPage`), which is what the mdBook uses (inferred from the napi-rs convention, not run).
- Every error becomes `Error::from_reason(format!("{e}"))`: a JS `Error` whose message is the only way to tell an out-of-range index from a damaged file (inferred).
- `page_width`/`page_height` are the CropBox (else MediaBox) before `/Rotate`; there is no rotation accessor ([aggregate](language-bindings.md#page-size-and-rotation)).
- `title`/`author` are read by the binding's own `get_info_string` as lossy UTF-8 — see [Text string encoding](../invariant/text-string-encoding.md).
- `package.json` says version `0.1.0` while `Cargo.toml` says `0.1.2`; nothing ties the two (inferred).
- Outside input: a path or a Buffer from JS (copied with `to_vec`); the binding adds no size bound and core's limits apply ([Document access](document-access.md)).

## Code
- `justpdf-node/src/lib.rs` — `Document`, `Document::open`, `from_buffer`, `authenticate`, `page_count`, `version`, `is_encrypted`, `text`, `page_text`, `render_page`, `page_width`, `page_height`, `title`, `author`, `get_info_string`
- `justpdf-node/build.rs` — `main` (`napi_build::setup`)
- `justpdf-node/package.json` — `napi`, `scripts.build`

## Reference behaviour
**None.**

## Cross-cutting invariants
- [Text string encoding](../invariant/text-string-encoding.md) — `get_info_string` reads `/Info` values as lossy UTF-8.

## Blast radius
- [Document access](document-access.md), [Text extraction](text-extraction.md), [Render API](render-api.md) — wrapped directly; a signature change there breaks this crate with CI green, since nothing builds it.
- [Language bindings](language-bindings.md) — the same calls wrapped separately in three other bindings.
- [Published docs](published-docs.md) — the mdBook (`docs/src/bindings.md`) and crate README use the camelCase names and `npm install justpdf`.
- [Crate publishing](crate-publishing.md) — the npm version in `package.json` is kept by hand apart from the crate version.
- [CI](ci.md) — not built.

## Known holes / open
- Its own `Cargo.lock` moves separately from the root one: on 2026-10-30 `cargo check` in `justpdf-node` passed, but only after cargo rewrote the committed lock for core 0.1.4 (measured; the change was reverted).
- The README's `npm install justpdf` names a package that does not exist on npm (measured 2026-10-30).
- No rotation accessor; decided in #263 to add one. Tracked: #263
- Not built in CI. Tracked: #201
