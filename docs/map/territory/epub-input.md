# EPUB input

## What it is
Reads text from an EPUB's OPF and XHTML and turns it into a PDF. Rejects DRM files.

## Governing decisions
- [ADR-0001](../../adr/0001-crates-split-by-dependency-layer.md) — `epub` is a feature assumed to build without render.

## Design model
- The the preview (`render_page`, `render_page_png`) borrows `crate::plaintext` only under `cfg(feature = "plaintext")`. With `plaintext` off it checks the index (`OutOfRange`) and then returns `FormatError::Format` ("page preview needs the `plaintext` feature") — no blank page. The feature definition does not pull `plaintext`: it stays render-free, as #2 and ADR-0001 set (maintainer's call in #35, 2026-10-02, chosen over making the feature pull `plaintext`). It compiles on its own, without warnings (measured 2026-10-07; until #35 it failed with `E0433: cannot find plaintext in crate`). Tests: `test_preview_needs_plaintext_feature`, `test_preview_renders_with_plaintext`, `test_preview_out_of_range`.
- Carries text only.

## Code
- `justpdf-formats/src/epub/mod.rs` — `EpubDocument`, `to_pdf`, `render_page`, `render_page_png`

## Reference behaviour
**None.**

## Cross-cutting invariants
- [Content text encoding](../invariant/content-text-encoding.md)

## Blast radius
- [Plaintext input](plaintext-input.md) — whose preview it borrows when the `plaintext` feature is on.
- [Crate layering](crate-layering.md) — the isolation script checks that `epub` pulls no render and compiles on its own.
- [Format document](format-document.md).

## Known holes / open
**None.**
