# Crate layering and feature isolation

## What it is
The structure that splits the workspace into external-dependency layers (core → +render → +formats/+special) and gates optional features behind feature flags so they pull in a heavy layer (render) only when needed. `scripts/check-feature-isolation.sh` checks part of this promise in CI.

## Governing decisions
- [ADR-0001](../../adr/0001-crates-split-by-dependency-layer.md) — the primary principle is layering by external dependency. Rejects the single crate + feature flags alternative.
- [ADR-0002](../../adr/0002-language-bindings-outside-workspace.md) — bindings outside the workspace, compress-wasm inside.
- [ADR-0003](../../adr/0003-cli-is-end-product-not-a-dependency-layer.md) — the CLI is not subject to this principle.

## Design model
- The isolation script checks render's presence/absence with `cargo tree --depth 1 --prefix none` (it avoids tree characters because they are ASCII in CI and UTF-8 in a terminal — until #62 the "present" check always failed in CI and the "absent" check always passed), and runs `cargo build` for `special/barcode`, `special/all` and `formats/all`.
- It also runs `cargo check -p justpdf-formats --no-default-features` with no feature and with each feature alone. The feature list is read from the `[features]` table of `justpdf-formats/Cargo.toml` (`manifest_features`, awk — no Python, because `python3` on a Windows dev machine can be the Store stub), minus `default` and `all`, so a new formats feature is checked without editing the script; an empty list fails. Against the tree before #35 it failed naming `formats/epub-alone` and `formats/office-alone` (measured, #200).
- **What the script cannot see**: standalone builds of other crates' features. In a workspace build, a feature some consumer turns on (the CLI turns on `formats/all`) hides a feature that only compiles next to another one; that is how `epub` and `office` went uncaught until #35.
- Apart from the formats standalone checks, the script's scope is a hand-written list. A new `justpdf-special` feature, or a render-presence expectation, is not checked unless it is added to the script.

## Code
- `scripts/check-feature-isolation.sh` — `assert_dep_absent`, `assert_dep_present`, `assert_build_ok`, `assert_check_ok`, `manifest_features`
- `Cargo.toml` — `members`
- `justpdf-formats/Cargo.toml` — `plaintext`, `mobi`, `fb2`
- `justpdf-special/Cargo.toml` — `ocr`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [Format document](format-document.md), [plaintext](plaintext-input.md), [EPUB](epub-input.md), [Office](office-input.md), [OCR](ocr.md), [Barcode](barcode.md) — subject to feature gates.
- [Facade](facade.md) — the core+render combination.
- [Language bindings](language-bindings.md) — the workspace boundary (disagrees with the ADR).
- [CI](ci.md) — where the script runs.

## Known holes / open
- Standalone builds are checked for `justpdf-formats` only (above).
- Tracked: #59 (CI gate gaps)
