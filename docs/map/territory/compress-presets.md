# Compress presets and knobs

## What it is
The ten knobs of `CompressOptions` and the four presets (`low`, `medium`, `high`, `extreme`) that name combinations of them. The preset names are a **product promise** shared by the core API, the browser WASM, the CLI and the external repo just-pdf-web. The code owns each preset's values: read `CompressOptions::preset_*`. They are not copied here.

## Governing decisions
**None.** Adjacent records that do not govern preset meaning:
- [ADR-0002](../../adr/0002-language-bindings-outside-workspace.md) settles that compress-wasm is a first-class product inside the workspace.
- [ADR-0003](../../adr/0003-cli-is-end-product-not-a-dependency-layer.md) settles that compression is a CLI subcommand.
- [ADR-0006](../../adr/0006-breaking-changes-are-allowed-before-1-0.md) allows a public-API break before 1.0. Its ground is that nobody outside the project depends on the crates, but just-pdf-web depends on the npm package and sends the preset names (see Blast radius).

The table in `dev/pdf-compress-wasm-design.md` §4 records what each preset turns on. At 88b2a08 it matches `preset_low` … `preset_extreme` (re-verified by reading both, 2026-10-30). It is a design doc, not a decision record.

## Design model
- Each preset's values are pinned by `test_preset_low_identity`, `test_preset_medium_identity`, `test_preset_high_identity` and `test_preset_extreme_identity` (added in #6). Changing a preset's values breaks these tests first.
- **No preset turns `grayscale` on** (`preset_*`, read at 88b2a08). Grayscale is reached only through WASM `compress_advanced`, CLI `--grayscale`, or options a caller builds.
- `structural` and `compress_streams` are on in every preset.
- Each surface exposes a different set of knobs (read at 88b2a08):
  - WASM `compress` takes a preset name. `compress_custom` takes quality and DPI, and fixes the rest to a combination no preset has: `skip_below_bytes` 5,000, subsetting and unused-resource removal on, stripping off. `compress_advanced` takes quality, DPI and the five on/off knobs; `structural`, `compress_streams` and `skip_below_bytes` 5,000 are fixed.
  - The CLI takes `--preset` plus one override per knob (`resolve_options`, `resolve_bool`). Only `remove_unused_resources` has no flag.
- The default preset differs by surface: the CLI uses `medium` (`CompressArgs`, `default_value = "medium"`), `examples/compress_pdf.rs` uses `high`, and just-pdf-web defaults to Strong, which it maps to `high` (`DEFAULT_PRESET_STRENGTH`, `src/features/compress/lib/preset-mapping.ts`, just-pdf-web d4410b8, 2026-10-06).
- With `jpeg_quality` `None` and `max_image_dpi` set, images are still re-encoded, at quality 75 (`recompress_images`, `unwrap_or(75)`; [Compress images](compress-images.md)). The WASM README says so.
- `CompressOptions` is a plain struct of public fields. Nothing range-checks `jpeg_quality` (0 and 101–255 reach the encoder) or `max_image_dpi` (≤ 0 means off in `compute_target_dimensions_with_ctm`) (inferred, read at 88b2a08).

## Code
- `justpdf-core/src/writer/compress.rs` — `CompressOptions`, `CompressOptions::preset_low`, `CompressOptions::preset_medium`, `CompressOptions::preset_high`, `CompressOptions::preset_extreme`, `CompressOptions::from_preset`, `CompressStats`, `test_preset_from_name`, `test_preset_low_identity`, `test_preset_medium_identity`, `test_preset_high_identity`, `test_preset_extreme_identity`
- `justpdf-compress-wasm/src/lib.rs` — `compress`, `compress_custom`, `compress_advanced`
- `justpdf-cli/src/main.rs` — `CompressArgs`, `resolve_bool`, `resolve_options`, `cmd_compress`
- `justpdf-core/examples/compress_pdf.rs` — `main`

## Reference behaviour
**None.** `dev/pdf-compress-wasm-design.md` §3 tabulates which techniques Ghostscript and qpdf apply, but no record compares a preset's output with Ghostscript's.

## Cross-cutting invariants
**None.** Agreement across the surfaces is kept by the blast-radius checklist below.

## Blast radius
Checklist for changing a preset's name, its values or a default:
- [Compress WASM](compress-wasm.md) — the preset list in the `compress` doc comment and the crate README promise the names and what they do. A changed name or meaning also needs an npm release before the browser product sees it.
- [CLI](cli.md) — the `--preset` help, the "Unknown preset" message in `resolve_options`, its unit tests (they compare resolved values with `preset_high` directly) and the preset list in `justpdf-cli/tests/compress.rs` all spell the four names.
- The external repo just-pdf-web (not a node of this map; read at d4410b8, 2026-10-06) — `STRENGTH_TO_WASM` maps Light/Balanced/Strong/Maximum to the four names, and the worker's `Preset` type spells them. The locale copy in `src/i18n/locales/en-US.compress.json` describes the values: Balanced claims "metadata strip" (`preset_medium` has `strip_metadata` off) and Maximum claims "grayscale" (`preset_extreme` has it off). Its `GLOSSARY.md` "Preset Strength" names the mapping too.
- [Compress pipeline](compress-pipeline.md) — each knob gates a stage in `compress_pdf`. A new knob does nothing until a stage reads it.
- `dev/pdf-compress-wasm-design.md` §4 — the table restates every preset's values.

## Known holes / open
- `remove_unused_resources` has no CLI flag. #11 asked for "all 10" knobs but listed 9; this gap was noted on the issue on 2026-09-23, and the issue was closed by merge without the flag (#11, 2026-09-23). Open question for the maintainer: was leaving it out intended?
- No record decides why the defaults differ (CLI `medium`, example and web `high`). Open question for the maintainer.
- WASM `compress_advanced` truncates `jpeg_quality: i32` with `as u8`, and nothing range-checks it. `image`'s JPEG encoder clamps to 1–100, so `compress_custom(…, 0, …)` encodes at quality 1. Decided on #57 (triage, 2026-10-07): 0 = off, 1–100 pass, anything else is a JS error, taken as a type wide enough to check; `None` plus downscaling stays q75 and the docs say so. Tracked: #57.
- `extreme` removes the embedded-files name tree, which breaks ZUGFeRD/Factur-X invoices ([Compress stripping](compress-stripping.md)). Tracked: #58.
