# EPUB input

## What it is
Reads text from an EPUB's OPF and XHTML and turns it into a PDF. Rejects DRM files.

## Governing decisions
- [ADR-0001](../../adr/0001-crates-split-by-dependency-layer.md) — `epub` is a feature assumed to build without render.

## Design model
- **The `epub` feature does not compile on its own**: `render_page` and `render_page_png` call `crate::plaintext`, but the `plaintext` module sits behind the `plaintext` feature. `cargo check -p justpdf-formats --no-default-features --features epub` fails with `E0433: cannot find plaintext in crate` (reproduced 2026-09-23). In a workspace build, the `all` feature the CLI turns on hides this through feature unification.
- Carries text only.

## Code
- `justpdf-formats/src/epub/mod.rs` — `EpubDocument`, `to_pdf`, `render_page`, `render_page_png`

## Reference behaviour
**None.**

## Cross-cutting invariants
- [Content text encoding](../invariant/content-text-encoding.md)

## Blast radius
- [Plaintext input](plaintext-input.md) — whose preview it borrows. A hidden dependency.
- [Crate layering](crate-layering.md) — the isolation script checks epub only with `cargo tree` and does not build it on its own.
- [Format document](format-document.md).

## Known holes / open
- Fails to build on its own (above). The acceptance criteria of #2 required each feature to build on its own, but the script does not check that.
- Tracked: #35 (epub and office standalone builds)
