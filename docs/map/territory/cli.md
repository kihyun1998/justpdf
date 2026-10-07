# CLI (the `justpdf` binary)

## What it is
The one official PDF CLI. The `info`, `text`, `render`, `merge`, `split`, `encrypt`, `decrypt`, `clean`, `compress`, `convert` and `sign` subcommands call core, render and formats directly. It is the only product released as per-OS binaries from a tagged release.

## Governing decisions
- [ADR-0003](../../adr/0003-cli-is-end-product-not-a-dependency-layer.md) — the CLI is a finished product. New features are added as subcommands, not separate tools; it depends on core+render+formats wholesale and does not aim to minimize binary size.

## Design model
- Encrypted input is authenticated by `open_doc` with `--password`.
- `encrypt` is always AES-128 and exposes only `--no-print` and `--no-copy`. It calls only `build` after `DocumentModifier::set_encryption` — `/Info`, `/ID` and `/Encrypt` are written by [Document modifier](document-modifier.md) ([Object encryption](object-encryption.md)).
- `clean` only rebuilds (it does not call the [Clean](clean.md) module).
- The integration tests write every input and output they make into a per-test `tempfile::tempdir()`, never under a fixed name in `std::env::temp_dir()`: that directory is shared by every `cargo test` on the machine, and worktrees with separate target dirs do not separate it. Four `cargo test -p justpdf-cli --tests` at once, three rounds: 3 of 12 runs failed before the switch (`overrides_layer_onto_preset_and_still_compress`, `encrypt_without_source_id_gets_a_fresh_one`) and none after; with each formerly fixed path occupied by a directory, 16 of the 27 tests failed before and none after (measured, #183).
- `convert` detects the input by extension (`detect_format`). Every `FormatDocument` input — plain text, SVG, EPUB, CBZ, XPS, Office, MOBI, FB2 — goes through `write_format_document`: `pdf` is `to_pdf()`, `png` the first page at 150 dpi, anything else `unsupported output format`, returned before anything is written (measured, `tests/convert.rs`). PDF input has its own arm. MOBI and FB2 fell through to "unsupported input format" until #49 (measured on master before the fix).
- `sign` is not implemented: it writes nothing and fails with exit code 1 (measured, `sign_fails_without_writing_output`). How it should take keys and certificates is open (#181).
- `compress` has per-knob overrides on top of `--preset` (`resolve_options`: a flag wins, an unspecified knob keeps the preset, giving both `--x`/`--no-x` is an error), `--analyze` (writes nothing), `--verbose` (details on stderr) and `--password` (decrypts, reserializes and compresses; the output is **not encrypted**). Only `remove_unused_resources` has no flag.

## Code
- `justpdf-cli/src/main.rs` — `Commands`, `CompressArgs`, `resolve_options`, `open_doc`, `cmd_info`, `cmd_text`, `cmd_render`, `cmd_merge`, `cmd_split`, `cmd_encrypt`, `cmd_decrypt`, `cmd_clean`, `cmd_compress`, `cmd_convert`, `write_format_document`
- `justpdf-cli/tests/compress.rs` — `every_preset_produces_a_valid_smaller_pdf`, `conflicting_on_off_pair_is_rejected`, `analyze_needs_no_output_flag_and_writes_nothing`, `verbose_prints_breakdown_on_stderr`
- `justpdf-cli/tests/compress_encrypted.rs` — `compresses_encrypted_pdf_with_password_and_drops_encryption`, `wrong_password_is_rejected`
- `justpdf-cli/tests/encrypt.rs` — `encrypt_keeps_the_source_permanent_id`, `encrypt_without_source_id_gets_a_fresh_one`, `encrypt_with_empty_source_id_gets_a_fresh_one`
- `justpdf-cli/tests/sign.rs` — `sign_fails_without_writing_output`
- `justpdf-cli/tests/convert.rs` — `fb2_converts_to_pdf`, `mobi_converts_to_pdf`, `fb2_and_mobi_convert_to_png`, `unsupported_output_fails_without_writing`

## Reference behaviour
**None.** The example reference for the feature scope is MuPDF `mutool` (`docs/mupdf-feature-analysis.md`). There is no record comparing each command's behaviour with `mutool`.

## Cross-cutting invariants
**None.**

## Blast radius
- [Compress presets](compress-presets.md), [Compress pipeline](compress-pipeline.md) — `compress`'s names and output.
- [Text output formats](text-output-formats.md), [Render API](render-api.md), [SVG renderer](svg-renderer.md) — `text` and `render`.
- [Format detection](format-detection.md), [Format document](format-document.md) — `convert`.
- [Document modifier](document-modifier.md), [File serialization](file-serialization.md), [Object encryption](object-encryption.md), [Permissions](permissions.md) — split/encrypt/decrypt/merge.
- [Signing](signing.md) — where `sign` should be connected.
- [Release](release.md) — only this crate is released as a binary.
- [Published docs](published-docs.md) — the CLI examples in the README, the mdBook (`docs/src/cli.md`) and the crate README. When changing a flag, check all three.

## Known holes / open
- The `--structural` help says "GC + dedup + object streams", but object stream packing is off.
- The "`--password` on an unencrypted PDF" path has no test (it works when run by hand).
- Tracked: #37 (connect signing), #181 (CLI sign: key and certificate input), #149 (`text --page`: 0-based message, `--page 0` panics)
