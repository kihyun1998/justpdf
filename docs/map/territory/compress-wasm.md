# Compress WASM (browser compression product)

## What it is
A WASM module that compresses PDFs in the browser with no server. It exposes only core's compression API (`compress_pdf`, `analyze_pdf`) and does not pull in the renderer. It ships as the npm package `@kihyun1998/justpdf-compress-wasm`, which the external repo just-pdf-web calls from a Web Worker.

## Governing decisions
- [ADR-0002](../../adr/0002-language-bindings-outside-workspace.md) — unlike the general bindings it is a finished product, so it lives **inside** the workspace and carries CI, tests and versioning along with core (`Cargo.toml` `members` includes it, read at 88b2a08).
- [ADR-0001](../../adr/0001-crates-split-by-dependency-layer.md) — it does not depend on `justpdf-render`, to keep the browser bundle small. `justpdf-compress-wasm/Cargo.toml` depends on `justpdf-core`, `wasm-bindgen`, `js-sys` and `getrandom` only (read at 88b2a08).
- [ADR-0003](../../adr/0003-cli-is-end-product-not-a-dependency-layer.md) names compress-wasm as the model for a single-purpose release, kept apart from the CLI.
- Adjacent, not governing: [ADR-0006](../../adr/0006-breaking-changes-are-allowed-before-1-0.md) allows public-API breaks before 1.0 on the ground that nobody outside the project depends on the crates. This crate's JS surface has a dependent, just-pdf-web (below).

## Design model
- **Surface** (`justpdf-compress-wasm/src/lib.rs`, read at 88b2a08): `compress(data, preset)`, `compress_custom(data, jpeg_quality: u8, max_dpi)`, `compress_advanced(data, jpeg_quality: i32, max_dpi, five on/off knobs)` and `analyze(data)`. Which knobs each fixes is in [Compress presets](compress-presets.md). Every error crosses as a string (`JsValue::from_str(&format!("{e}"))`), so a caller can tell errors apart only by message text.
- **Results**: `CompressResult` copies the 13 `CompressStats` fields by hand into `u32` getters (`stats_to_result`) and adds `ratio`; `data()` returns a copy of the bytes. `AnalyzeResult` copies `analyze_pdf`'s four fields. On wasm32 `usize` is 32 bits, so the `as u32` casts lose nothing there.
- `compress_advanced` truncates `jpeg_quality: i32` with `as u8` and passes 101–255 through; `compress_custom` passes 0 as quality 0, which the `image` encoder clamps to 1 (#57).
- **The seam with just-pdf-web** (read at kihyun1998/just-pdf-web d4410b8, 2026-10-06): `package.json` depends on `@kihyun1998/justpdf-compress-wasm` `^0.1.3`. `src/workers/compression.worker.ts` calls `mod.default()` (the wasm-bindgen init), `wasm.analyze(bytes)` (reading `pages`, `images`, `total_image_bytes`, `is_encrypted`), `wasm.compress(bytes, preset)` (reading `data()`, `original_size`, `compressed_size`, `ratio`), and analyzes the output again as a sanity check comparing page and image counts (`compareSanity`). It never calls `compress_custom` or `compress_advanced`. Its `docs/adr/0001-wasm-runs-only-in-the-compression-worker.md` keeps every wasm call in that worker because the API is synchronous with no progress callback. Its `preset-mapping.ts` maps Light/Balanced/Strong/Maximum to `low`/`medium`/`high`/`extreme`, default Strong. `size-policy.ts` asks for confirmation above 100 MB and rejects above 500 MB.
- **The browser runs an old core.** npm's latest version is 0.1.3, published 2026-05-08T01:59Z (measured, `npm view @kihyun1998/justpdf-compress-wasm time`, 2026-10-30), right after c33dbff bumped the crate to 0.1.3 (2026-05-08). Since c33dbff, 23 commits have touched `writer/compress.rs` or `font/subset.rs` (measured, `git log --oneline c33dbff..HEAD`), among them #27 (dedup equality), #51 (glyph IDs kept), #69, #117, #223 and #46, so none of those reach the browser product until a new npm release (inferred).
- The crate builds for `wasm32-unknown-unknown` at 88b2a08 (measured, `cargo check -p justpdf-compress-wasm --target wasm32-unknown-unknown`, 2026-10-30).
- The `js` feature of `getrandom` is not used by this crate's code. Core depends on `getrandom` (directly for crypto randomness — [Object encryption](object-encryption.md); transitively through `rsa` → `rand_core`), and the feature makes that build on wasm32 (design doc §5, I-3: "added the getrandom js feature"; not re-checked by building without it).
- `pkg/` (the `wasm-pack` output the README's example imports) is not in the repository, and `.gitignore` does not ignore it, so a local build shows it as untracked (read at 88b2a08).
- **Input bounds**: nothing in the crate bounds the input; it inherits [Compress pipeline](compress-pipeline.md)'s (none) inside wasm32's 4 GiB address space. The only size limit at the seam is just-pdf-web's 500 MB hard cap.

## Code
- `justpdf-compress-wasm/src/lib.rs` — `compress`, `compress_custom`, `compress_advanced`, `analyze`, `stats_to_result`, `CompressResult`, `CompressResult::data`, `CompressResult::ratio`, `AnalyzeResult`
- `justpdf-compress-wasm/Cargo.toml` — `justpdf-core` (path only), `getrandom` (feature `js`)
- `justpdf-compress-wasm/README.md` — the npm package's documentation of the API

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [Compress presets](compress-presets.md) — `compress` accepts exactly the names `from_preset` knows, and the README and doc comment describe what each does.
- [Compress pipeline](compress-pipeline.md) — `CompressStats` and `AnalyzeResult` are copied field by field; a new field is invisible here until copied, and a renamed one breaks the build. `analyze_pdf`'s image count is what the web's sanity check compares.
- [Crate publishing](crate-publishing.md) — the dependency on core has a `path` and no `version`, so `cargo publish` would refuse the crate (inferred, not run); it is not on crates.io (measured, `cargo search justpdf-compress-wasm` finds nothing, 2026-10-30). No workflow in `.github/workflows/` builds or publishes the npm package.
- [CI](ci.md) — CI checks and tests this crate only for the host target (`cargo check --workspace`, `cargo test --workspace`); no job builds it for wasm32, so a change that breaks the wasm32 build passes CI.
- The external repo just-pdf-web (not a node of this map) — renaming `compress`, `analyze`, a getter it reads, or a preset name breaks its worker and its `Preset` type when it upgrades; `^0.1.3` takes any 0.1.x release without a change on its side.

## Known holes / open
- `compress_advanced` truncates `jpeg_quality` and nothing range-checks it; `compress_custom(…, 0, …)` encodes at quality 1; with `jpeg_quality` 0 and `max_dpi > 0` images are still re-encoded at q75 (README says so). Decided on #57 (triage, 2026-10-07): 0 = off, 1–100 pass, anything else a JS error, taken as a type wide enough to check; `None` plus downscaling stays q75 with the docs fixed. Tracked: #57.
- `extreme` removes embedded files, which breaks ZUGFeRD/Factur-X invoices; the README warns. Tracked: #58.
- The npm package lags the repository by every compress fix since 2026-05-08 (measured above). No issue tracks a release. Open question for the maintainer: how is the npm package built and published (no workflow does it)?
- No job builds the crate for wasm32 or runs `wasm-pack` (above). #201 asks for wasm32 builds of the bindings outside the workspace (`justpdf-wasm` and others), not this crate.
- just-pdf-web's sanity check fails on a correct output when dedup or GC removes an image stream ([Compress pipeline](compress-pipeline.md), measured on `testpdf.pdf`).
- The crate has no tests of its own (no `#[test]` and no wasm-bindgen tests in `src/lib.rs`, read at 88b2a08); the JS surface is exercised only by just-pdf-web.
