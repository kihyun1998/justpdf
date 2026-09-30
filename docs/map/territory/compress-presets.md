# Compress presets and knobs

## What it is
The 10 knobs of `CompressOptions`, and the four presets (low/medium/high/extreme) that name combinations of them. The preset names are a **product promise** shared by the core API, the browser WASM, the CLI and the external repo Just-pdf-web. The code owns each preset's actual values — read `CompressOptions::preset_*` (they are not copied here).

## Governing decisions
**None.** [ADR-0002](../../adr/0002-language-bindings-outside-workspace.md) only settles that compress-wasm is a first-class product inside the workspace, and [ADR-0003](../../adr/0003-cli-is-end-product-not-a-dependency-layer.md) only that compression is a CLI subcommand; neither settles preset meanings, the knob set, or agreement across surfaces.

## Design model
- Tests pin each preset's identity: `test_preset_low_identity` … `test_preset_extreme_identity` (added in #6). Changing a preset's values should break these tests first.
- **All four presets have grayscale off.** Grayscale is turned on only through `compress_advanced` (WASM) or custom options.
- Each surface exposes different knobs:
  - WASM `compress` takes a preset name, `compress_custom` only quality and DPI (the rest is a fixed combination that differs from every preset), `compress_advanced` only some knobs.
  - The CLI takes `--preset` + per-knob overrides (`resolve_options`). Only `remove_unused_resources` has no flag.
- The default preset differs by surface: the CLI uses `"medium"`, `examples/compress_pdf.rs` uses `"high"`, and Just-pdf-web uses Strong, which maps to high.
- Even with `jpeg_quality` as `None`, images are re-encoded at q75 when `max_image_dpi` is set ([Compress images](compress-images.md)). The WASM README documents this behaviour.

## Code
- `justpdf-core/src/writer/compress.rs` — `CompressOptions`, `preset_low`, `preset_medium`, `preset_high`, `preset_extreme`, `from_preset`, `CompressStats`, `test_preset_low_identity`, `test_preset_extreme_identity`
- `justpdf-compress-wasm/src/lib.rs` — `compress`, `compress_custom`, `compress_advanced`
- `justpdf-cli/src/main.rs` — `cmd_compress`
- `justpdf-core/examples/compress_pdf.rs` — `main`

## Reference behaviour
**None.** `dev/pdf-compress-wasm-design.md` §3 carries a table of Ghostscript and qpdf techniques, but there is no record of comparing preset results with Ghostscript output.

## Cross-cutting invariants
**None.** Agreement across surfaces is managed with the blast radius checklist below.

## Blast radius
Checklist for changing a preset's name, meaning or default:
- [Compress WASM](compress-wasm.md) — the `compress` doc comment, the crate README, npm republishing.
- [CLI](cli.md) — the `--preset` help, the error string in `cmd_compress`, `resolve_options` and its unit tests (which compare preset values directly), the preset list in `justpdf-cli/tests/compress.rs`.
- The external repo Just-pdf-web (not a node of this map) — the `STRENGTH_TO_WASM` mapping, the preset type, the locale copy (the medium/extreme descriptions currently differ from the actual values), that repo's `CONTEXT.md`.
- [Compress pipeline](compress-pipeline.md) — which stage each knob turns on.
- The table in `dev/pdf-compress-wasm-design.md` §4.

## Known holes / open
- `remove_unused_resources` has no CLI flag (#11 was closed by merge, and there is no record of deciding to leave it out).
- There is no record of deciding the default preset mismatch (CLI medium vs example and web high).
- Tracked: #57 (jpeg_quality truncation), #58 (extreme removes embedded files)
