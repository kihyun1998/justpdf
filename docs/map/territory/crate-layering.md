# Crate layering and feature isolation

## What it is
The structure that splits the workspace into external-dependency layers (core → +render → +formats/+special) and gates optional features behind feature flags so they pull in a heavy layer (render) only when needed. `scripts/check-feature-isolation.sh` checks part of this promise in CI.

## Governing decisions
- [ADR-0001](../../adr/0001-crates-split-by-dependency-layer.md) — the primary principle is layering by external dependency. Rejects the single crate + feature flags alternative.
- [ADR-0002](../../adr/0002-language-bindings-outside-workspace.md) — bindings outside the workspace, compress-wasm inside.
- [ADR-0003](../../adr/0003-cli-is-end-product-not-a-dependency-layer.md) — the CLI is not subject to this principle.

## Design model
- The isolation script checks render's presence/absence with `cargo tree --depth 1 --prefix none` (it avoids tree characters because they are ASCII in CI and UTF-8 in a terminal — until #62 the "present" check always failed in CI and the "absent" check always passed), and runs `cargo build` only for `special/barcode`, `special/all` and `formats/all`.
- **What the script cannot see**: per-feature standalone builds. `epub` and `office` pass under `cargo tree` because render is absent, but do not compile on their own ([EPUB](epub-input.md)). In a workspace build, `formats/all`, which the CLI turns on, hides this through feature unification.
- The script's scope is a hand-written list. A new feature is not checked unless it is added to the script.

## Code
- `scripts/check-feature-isolation.sh` — `assert_dep_absent`, `assert_dep_present`, `assert_build_ok`
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
- No per-feature standalone build check (above). It is the check #2's acceptance criteria asked for.
- Tracked: #35 (epub/office standalone builds), #59 (CI gate gaps)
