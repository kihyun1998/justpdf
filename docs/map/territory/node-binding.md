# Node binding

## What it is
Exposes `Document` (open, buffer, authenticate, text, render, size, Info) through napi-rs.

## Governing decisions
- [ADR-0002](../../adr/0002-language-bindings-outside-workspace.md) — kept separate, as the ADR says.

## Design model
- `build.rs` calls `napi_build::setup`, and `package.json` uses `napi build`.

## Code
- `justpdf-node/src/lib.rs` — `Document`, `open`, `from_buffer`, `authenticate`, `text`, `page_text`, `render_page`, `page_width`, `page_height`
- `justpdf-node/build.rs` — `setup`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [Document access](document-access.md), [Text extraction](text-extraction.md), [Render API](render-api.md).
- [CI](ci.md) — not built.

## Known holes / open
- Its own `Cargo.lock` moves separately from the root one.
