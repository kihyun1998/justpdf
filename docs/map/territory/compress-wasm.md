# Compress WASM (browser compression product)

## What it is
A WASM module that compresses PDFs in the browser with no server. It exposes only core's compression API and does not pull in the renderer. It ships as an npm package, and the external repo Just-pdf-web calls `analyze` and `compress(bytes, preset)` from a worker.

## Governing decisions
- [ADR-0002](../../adr/0002-language-bindings-outside-workspace.md) — unlike the general bindings it is a "finished product", so it lives **inside** the workspace and carries CI, tests and versioning along with core.
- [ADR-0001](../../adr/0001-crates-split-by-dependency-layer.md) — leaves out render to keep the bundle small.

## Design model
- Surface: `compress` (preset), `compress_custom` (quality and DPI, the rest fixed), `compress_advanced` (some knobs), `analyze`. The result getters expose the `CompressStats` and `AnalyzeResult` fields 1:1.
- `jpeg_quality: i32` in `compress_advanced` is truncated with `as u8`.
- The `js` feature of `getrandom` is not used by compress-wasm code. Core depends on `getrandom` (directly: crypto randomness — [Object encryption](object-encryption.md); transitively: `rsa` → `rand_core`), so it is there to make the build work on wasm32 (design doc I-3).
- `pkg/` (the build output) is not tracked in git.

## Code
- `justpdf-compress-wasm/src/lib.rs` — `compress`, `compress_custom`, `compress_advanced`, `analyze`, `CompressResult`, `AnalyzeResult`
- `justpdf-compress-wasm/Cargo.toml` — `getrandom`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [Compress presets](compress-presets.md) — preset names and meanings.
- [Compress pipeline](compress-pipeline.md) — adding or removing a stats field reaches the getters.
- [Crate publishing](crate-publishing.md) — the dependency on core has only `path` and no `version`, which blocks publishing to crates.io (inferred, `cargo publish` not run). There is no npm publishing path in the repo's workflows.
- The external repo Just-pdf-web — renaming a getter or a preset breaks its worker and types.

## Known holes / open
- In `compress_advanced`, even with `jpeg_quality` set to 0, images are re-encoded at q75 when `max_dpi > 0` (documented in the README). Only the DPI side can be turned off with a knob.
- Tracked: #57 (jpeg_quality truncation), #58 (extreme removes embedded files)
