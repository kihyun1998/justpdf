# CLI (the `justpdf` binary)

## What it is
The one official PDF CLI (crate `justpdf-cli`, binary `justpdf`). The `info`, `text`, `render`, `merge`, `split`, `encrypt`, `decrypt`, `clean`, `compress`, `convert` and `sign` subcommands (`Commands`) call core, render and formats directly, not the [Facade](facade.md). It is the only product the release workflow builds as per-OS binaries ([Release](release.md)); the crate is also on crates.io (0.1.2, measured 2026-10-30).

## Governing decisions
- [ADR-0003](../../adr/0003-cli-is-end-product-not-a-dependency-layer.md) — the CLI is a finished product. New features are added as subcommands, not separate tools; it depends on core+render+formats wholesale and does not aim to minimize binary size. `justpdf-cli/Cargo.toml` takes `justpdf-formats` with `features = ["all"]` (inferred, read 2026-10-30).

## Design model
Status: from reading the source and the tests; ADR-0003 decides only the CLI's scope.
- Passwords: `info`, `text`, `render` and `compress` open through `open_doc`, which authenticates with `--password` and refuses an encrypted, unauthenticated file without one. `decrypt` takes a required `--password` of its own. `merge`, `split`, `encrypt`, `clean` and `convert` open with `PdfDocument::open` and have no password flag, so an encrypted input with a non-empty user password fails there (inferred; `encrypt` refusing an encrypted input is tested, `encrypt_refuses_an_encrypted_input`).
- Page options are 1-based. `text --page` and `render --page` are `NonZeroUsize`, so clap rejects `0` while parsing (exit 2). `split --pages` (`parse_page_range`) rejects page `0` or a range starting at `0` (`Page 0 out of range (1-N)`), rejects a single page past the end, and clamps a range's end to the last page without a message (`1-99` on a one-page file extracts one page, exit 0; measured 2026-10-30). `text --page` past the end reports `Page N out of range (total: M)`, mapping core's `PageOutOfRange`; `render` reports the same text from its own check. Until #149 all three subtracted 1 from `0` and panicked (measured in #149; now `tests/page_numbers.rs`).
- `encrypt` is always AES-128 (`EncryptionMethod::AES128`) and exposes only `--no-print` and `--no-copy` besides the passwords. It calls `DocumentModifier::set_encryption` and then only `build` — `/Info`, `/ID` and `/Encrypt` are written by [Document modifier](document-modifier.md) ([Object encryption](object-encryption.md)).
- `clean` runs `DocumentModifier::garbage_collect` and `build`; it does not call the [Clean](clean.md) module.
- Every `DocumentModifier::from_document` caller (`split`, `encrypt`, `decrypt`, `clean`, the encrypted branch of `compress`) inherits its silent drop of unreadable source objects: the output is written and the exit code is 0. Decided in #97 (maintainer, 2026-10-07): print one stderr warning per unreadable object and exit 3; not implemented (`main` exits only 0 or 1). Tracked: #97
- Exit codes: 0 success, 1 any error (`main`), 2 clap usage.
- `sign` is not implemented: `run` returns an error for `Commands::Sign` without reading its arguments, so it writes nothing and exits 1 (`sign_fails_without_writing_output`). How it should take keys and certificates is open (#181).
- `compress` has per-knob overrides on top of `--preset` (`resolve_options`, `resolve_bool`: a flag wins, an unspecified knob keeps the preset, giving both `--x`/`--no-x` is a clap error), `--analyze` (`cmd_compress_analyze`, writes nothing), `--verbose` (`print_compress_breakdown`, stderr) and `--password` (decrypts through `DocumentModifier`, then compresses; the output is **not encrypted** and a warning says so). On an unencrypted file `--password` is ignored and compression succeeds (measured 2026-10-30, `compressible.pdf --password x`, exit 0). Only `remove_unused_resources` has no flag.
- `convert` picks the input type from the file extension only (`detect_format`, `justpdf-formats/src/detect.rs`); the output type is `--format` or the output extension, default `pdf`. Every `FormatDocument` input — plain text, SVG, EPUB, CBZ, XPS, Office, MOBI, FB2 — goes through `write_format_document`: `pdf` is `to_pdf()`, `png` the first page at 150 dpi, anything else `unsupported output format`, returned before anything is written (`unsupported_output_fails_without_writing`). PDF input has its own arm (`svg`, `png` of page 1, `txt`). MOBI and FB2 fell through to "unsupported input format" until #49 (measured on master before the fix, #49).
- `info` prints `/Info` fields through `print_info_field`, which decodes them as lossy UTF-8 — see [Text string encoding](../invariant/text-string-encoding.md).
- The integration tests write every input and output they make into a per-test `tempfile::tempdir()`, never under a fixed name in `std::env::temp_dir()` (no `temp_dir` in `justpdf-cli/tests/`, checked 2026-10-30): that directory is shared by every `cargo test` on the machine. Four `cargo test -p justpdf-cli --tests` at once, three rounds: 3 of 12 runs failed before the switch and none after; with each formerly fixed path occupied by a directory, 16 of the then 27 tests failed before and none after (measured, #183). At HEAD the crate has 6 unit and 30 integration tests, all passing (measured 2026-10-30).
- Outside input: files named on the command line. The CLI adds no size or count bound; `merge` opens every input before merging and holds all of them in memory, and `render --all` writes one file per page (inferred). Core's limits apply ([Document access](document-access.md)).

## Code
- `justpdf-cli/src/main.rs` — `Cli`, `Commands`, `CompressArgs`, `main`, `run`, `open_doc`, `print_info_field`, `cmd_info`, `cmd_text`, `cmd_render`, `cmd_merge`, `cmd_split`, `parse_page_range`, `cmd_encrypt`, `cmd_decrypt`, `cmd_clean`, `resolve_bool`, `resolve_options`, `cmd_compress`, `print_compress_breakdown`, `cmd_compress_analyze`, `write_format_document`, `cmd_convert`; unit tests `unset_knobs_preserve_the_preset`, `on_off_knobs_override_in_both_directions`, `unknown_preset_is_rejected`
- `justpdf-cli/tests/compress.rs` — `every_preset_produces_a_valid_smaller_pdf`, `conflicting_on_off_pair_is_rejected`, `jpeg_quality_out_of_range_is_rejected`, `overrides_layer_onto_preset_and_still_compress`, `analyze_needs_no_output_flag_and_writes_nothing`, `verbose_prints_breakdown_on_stderr`
- `justpdf-cli/tests/compress_encrypted.rs` — `compresses_encrypted_pdf_with_password_and_drops_encryption`, `encrypted_pdf_without_password_is_an_error`, `wrong_password_is_rejected`
- `justpdf-cli/tests/encrypt.rs` — `encrypt_keeps_the_source_permanent_id`, `encrypt_without_source_id_gets_a_fresh_one`, `encrypt_with_empty_source_id_gets_a_fresh_one`, `encrypt_refuses_an_encrypted_input`
- `justpdf-cli/tests/sign.rs` — `sign_fails_without_writing_output`
- `justpdf-cli/tests/page_numbers.rs` — `text_page_zero_is_rejected`, `render_page_zero_is_rejected`, `text_page_past_the_end_reports_one_based_page`, `text_first_page_still_works`, `split_range_starting_at_zero_is_rejected`
- `justpdf-cli/tests/convert.rs` — `fb2_converts_to_pdf`, `mobi_converts_to_pdf`, `fb2_and_mobi_convert_to_png`, `unsupported_output_fails_without_writing`

## Reference behaviour
**None.** The example reference for the feature scope is MuPDF `mutool` (`docs/mupdf-feature-analysis.md`; `docs/agents/thegraph.md` lists MuPDF as an example reference). There is no record comparing each command's behaviour with `mutool`.

## Cross-cutting invariants
- [Text string encoding](../invariant/text-string-encoding.md) — `print_info_field` reads `/Info` values as lossy UTF-8.

## Blast radius
- [Compress presets](compress-presets.md), [Compress pipeline](compress-pipeline.md) — `resolve_options` mirrors the knobs of `CompressOptions` by hand and `--preset` names come from `from_preset`; a new or renamed knob needs a flag here, or it stays unreachable from the CLI.
- [Text output formats](text-output-formats.md), [Render API](render-api.md), [SVG renderer](svg-renderer.md) — `text --format` and `render --format` map strings onto their output enums by hand; a new format is not reachable until mapped.
- [Format detection](format-detection.md), [Format document](format-document.md) — `convert` dispatches on `DocumentFormat` by extension and writes through the `FormatDocument` trait; a new format or a trait change needs a `cmd_convert` arm.
- [Document modifier](document-modifier.md), [File serialization](file-serialization.md), [Object encryption](object-encryption.md), [Permissions](permissions.md) — split, encrypt, decrypt, clean and encrypted compress write only through `DocumentModifier::build`; what it drops or writes (`/ID`, `/Encrypt`, unreadable objects, #97) is what the CLI outputs, and exit code 3 waits on the modifier reporting it.
- [Signing](signing.md) — `sign` has no connection yet; the CLI's key and certificate input (#181) depends on what `sign_pdf` takes.
- [Release](release.md) — only this crate is built as a release binary; a new binary name or a removed one changes the release packaging (`target/.../justpdf`).
- [Published docs](published-docs.md) — the CLI examples in the README, the mdBook (`docs/src/cli.md`) and the crate README are compared against `--help` by hand only; a renamed flag leaves all three wrong.

## Known holes / open
- The `--structural` help says "GC + dedup + object streams", but object stream packing is disabled: the `pack_into_object_streams` call is commented out in `justpdf-core/src/writer/compress.rs` (inferred, read 2026-10-30).
- The "`--password` on an unencrypted PDF" path has no test (it works when run, above).
- `split` clamps a range past the last page without a message (above); whether that should be an error like a single page past the end is not decided.
- `merge`, `split`, `encrypt`, `clean` and `convert` take no password, so they cannot read an encrypted input with a user password (above).
- Unreadable source objects are dropped with exit 0. Tracked: #97
- Tracked: #37 (connect signing), #181 (CLI sign: key and certificate input), #308 (CLI sign: PKCS#12 input), #309 (CLI sign: `--timestamp-url`), #174 (CLI `--font` for text converters)
- `split` builds each part with `DocumentModifier::reorder_pages`, so it inherits the root-`/Kids`-only page edits: on a nested page tree it changes the wrong node or nothing. Tracked: #80 (inferred, reported by the 2026-10-30 rewrite).
