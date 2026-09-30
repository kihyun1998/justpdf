# General-purpose WASM binding

## What it is
Exposes `WasmDocument` (constructor, authenticate, text, PNG render, size, Info) through wasm-bindgen. A different crate from the compression-only [Compress WASM](compress-wasm.md).

## Governing decisions
- [ADR-0002](../../adr/0002-language-bindings-outside-workspace.md) — it should be outside the workspace, but it has no `[workspace]` block, so cargo currently cannot resolve its manifest ([aggregate](language-bindings.md#where-adr-0002-and-the-repository-disagree)).

## Design model
- There is no `js_name` rename, so the class name on the JS side is `WasmDocument`.

## Code
- `justpdf-wasm/src/lib.rs` — `WasmDocument`, `new`, `authenticate`, `text`, `page_text`, `render_page_png`, `page_width`, `page_height`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [Document access](document-access.md), [Text extraction](text-extraction.md), [Render API](render-api.md).
- [Published docs](published-docs.md) — the README and mdBook examples use `WasmDocument` and snake_case method names (fix them together if `js_name` changes).
- [CI](ci.md) — not built.

## Known holes / open
- Cannot be built on its own (above).
- Tracked: #36 (justpdf-wasm manifest, ADR-0002)
