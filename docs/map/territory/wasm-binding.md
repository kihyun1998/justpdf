# General-purpose WASM binding

## What it is
Exposes `WasmDocument` (constructor from bytes, authenticate, page count, version, text, PNG render, page size, Title/Author) through wasm-bindgen (crate `justpdf-wasm`). A different crate from the compression-only [Compress WASM](compress-wasm.md), which is inside the workspace by ADR-0002's criterion.

## Governing decisions
- [ADR-0002](../../adr/0002-language-bindings-outside-workspace.md) — it should be outside the workspace with an empty `[workspace]`. It has no `[workspace]` block and is not in the root `members`, so cargo cannot resolve its manifest (measured 2026-10-30, [aggregate](language-bindings.md#where-adr-0002-and-the-manifests-disagree)).

## Design model
Status: only from reading the source; it cannot be built at HEAD.
- There is no `js_name` rename, so the class name on the JS side is `WasmDocument` and method names stay snake_case (`page_text`, `render_page_png`, `page_width`) — the README and mdBook use those names.
- Every error becomes `JsValue::from_str(format!("{e}"))`: callers get a string, with no way to tell an out-of-range index from a damaged file except by its text (inferred).
- `page_width`/`page_height` are the CropBox (else MediaBox) before `/Rotate`; there is no rotation accessor ([aggregate](language-bindings.md#page-size-and-rotation)).
- `title`/`author` are read by the binding's own `get_info_string` as lossy UTF-8 — see [Text string encoding](../invariant/text-string-encoding.md).
- Outside input: a byte slice from JS, copied (`data.to_vec()`); the binding adds no size bound and core's limits apply ([Document access](document-access.md)).

## Code
- `justpdf-wasm/src/lib.rs` — `WasmDocument`, `WasmDocument::new`, `page_count`, `version`, `is_encrypted`, `authenticate`, `text`, `page_text`, `render_page_png`, `page_width`, `page_height`, `title`, `author`, `get_info_string`

## Reference behaviour
**None.**

## Cross-cutting invariants
- [Text string encoding](../invariant/text-string-encoding.md) — `get_info_string` reads `/Info` values as lossy UTF-8.

## Blast radius
- [Document access](document-access.md), [Text extraction](text-extraction.md), [Render API](render-api.md) — wrapped directly; their signatures reach this crate with no compiler run to catch a break, since nothing builds it.
- [Language bindings](language-bindings.md) — the same calls wrapped separately in three other bindings.
- [Published docs](published-docs.md) — the README, mdBook and crate README examples use `WasmDocument` and snake_case method names; adding a `js_name` changes all of them.
- [Compress WASM](compress-wasm.md) — the two wasm crates share a target and a toolchain (wasm-bindgen) but sit on opposite sides of the workspace boundary; a workspace-level change for one does not reach the other.
- [CI](ci.md) — not built.

## Known holes / open
- Cannot be built at all until its manifest or ADR-0002 changes (above). Tracked: #36
- Not on crates.io or npm (measured 2026-10-30); how it is meant to be published (`wasm-pack`, per its README) is not written down.
- No rotation accessor; decided in #263 to add `page_rotation`. Tracked: #263
- Not built in CI. Tracked: #201
