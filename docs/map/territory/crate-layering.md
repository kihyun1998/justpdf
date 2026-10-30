# Crate layering and feature isolation

## What it is
The structure that splits the workspace into external-dependency layers (core → +render → +formats/+special, with the facade as core+render) and gates optional features behind feature flags so they pull in a heavy layer (render) only when needed. `scripts/check-feature-isolation.sh` checks part of this promise, run by CI's `feature-isolation` job ([CI](ci.md)).

## Governing decisions
- [ADR-0001](../../adr/0001-crates-split-by-dependency-layer.md) — the primary principle is layering by external dependency. Rejects the single crate + feature flags alternative.
- [ADR-0002](../../adr/0002-language-bindings-outside-workspace.md) — general bindings outside the workspace, compress-wasm inside. The manifests disagree for ffi and wasm ([aggregate](language-bindings.md#where-adr-0002-and-the-manifests-disagree)).
- [ADR-0003](../../adr/0003-cli-is-end-product-not-a-dependency-layer.md) — the CLI is not subject to this principle.

## Design model
Status: the layers come from ADR-0001; what is checked comes only from the script.
- The workspace (`Cargo.toml` `members`) holds core, render, the facade, the CLI, ffi, formats, special and compress-wasm (read 2026-10-30). Render-gating features: `justpdf-formats` `plaintext`, `mobi`, `fb2` take `dep:justpdf-render`, the rest (`xps`, `epub`, `svg`, `office`, `cbz`) do not; `justpdf-special` `ocr` takes `dep:justpdf-render`, the rest (`barcode`, `zugferd`, `bidi`, `deskew`) do not.
- The isolation script checks render's presence/absence with `cargo tree --depth 1 --edges no-dev --prefix none` (`assert_dep_absent`, `assert_dep_present`). It avoids tree characters because they are ASCII in CI and UTF-8 in a terminal — until #62 the "present" check always failed in CI and the "absent" check always passed (#62). It runs `cargo build` for `special/barcode`, `special/all` and `formats/all` (`assert_build_ok`).
- It also runs `cargo check -p justpdf-formats --no-default-features` with no feature and with each feature alone (`assert_check_ok`). The feature list is read from the `[features]` table of `justpdf-formats/Cargo.toml` (`manifest_features`, awk — no Python, because `python3` on a Windows dev machine can be the Store stub), minus `default` and `all`, so a new formats feature is checked without editing the script; an empty list fails. Against the tree before #35 it failed naming `formats/epub-alone` and `formats/office-alone` (measured, #200). At HEAD all 23 assertions pass (measured 2026-10-30, macOS).
- **What the script cannot see**: standalone builds of other crates' features. In a workspace build, a feature some consumer turns on (the CLI turns on `formats/all`) hides a feature that only compiles next to another one; that is how `epub` and `office` went uncaught until #35.
- Apart from the formats standalone checks, the script's scope is a hand-written list. Not checked at HEAD: `justpdf-special` `zugferd`, `bidi`, `deskew` alone; `justpdf-render` without its default `base14-fonts`; `justpdf-core` `mmap` or `arena` alone; the facade's features alone. A new `justpdf-special` feature, or a render-presence expectation, is not checked unless added to the script (inferred from the script).

## Code
- `scripts/check-feature-isolation.sh` — `assert_dep_absent`, `assert_dep_present`, `assert_build_ok`, `assert_check_ok`, `manifest_features`
- `Cargo.toml` — `members`
- `justpdf-formats/Cargo.toml` — `[features]` (`plaintext`, `mobi`, `fb2`, `all`)
- `justpdf-special/Cargo.toml` — `[features]` (`ocr`, `barcode`, `all`)
- `justpdf/Cargo.toml` — `[dependencies]` (core, render, optional `tokio`)

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [Format document](format-document.md), [plaintext](plaintext-input.md), [EPUB](epub-input.md), [Office](office-input.md), [OCR](ocr.md), [Barcode](barcode.md) — code behind a feature gate must compile with only that feature on; a shared helper used by two features but gated on one breaks the other's standalone build (only formats is checked).
- [Facade](facade.md) — ADR-0001 says it is core+render only; a new dependency there breaks that promise and no script checks it.
- [Language bindings](language-bindings.md) — the workspace boundary ADR-0002 draws; moving a crate across it changes which lockfile and which CI jobs see it.
- [Compress WASM](compress-wasm.md) — ADR-0001 relies on it not depending on render; nothing checks that.
- [CI](ci.md) — the script runs only there and on a developer's machine; a check added to the script but not to CI is not a gate.

## Known holes / open
- Standalone builds are checked for `justpdf-formats` only; special, render, core and facade features are not (above). No open issue tracks this (#59, closed 2026-10-06, split its remaining CI gaps into #201 and #202, neither of which covers it).
- Nothing checks that the facade and compress-wasm stay off formats/special and render respectively.
- The script's `assert_check_ok` discards `cargo check`'s stderr and builds without `-D warnings`, so a feature combination that compiles with warnings passes: `justpdf-formats` built without `plaintext`, `mobi` and `fb2` warns `wrap_fixed_pitch is never used` (measured 2026-10-30 by the map rewrite). Clippy runs only with all features, so CI does not see it either.
