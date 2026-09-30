# Facade (the `justpdf` crate)

## What it is
The user-facing API wrapping `justpdf-core` and `justpdf-render`: `Document` (open, authenticate, pages, metadata, text, search, outlines, annotations, forms, attachments, signature detection, render), `Page`, `Modifier`, `merge`. The entry point consumed from crates.io under the name `justpdf`.

## Governing decisions
- [ADR-0001](../../adr/0001-crates-split-by-dependency-layer.md) — this crate integrates only core + render and **deliberately excludes** formats/special.

## Design model
- No crate in the repository depends on `justpdf`. The four bindings and the CLI all use core/render directly.
- There is no `Metadata` type — metadata is string getters and `metadata() -> Vec<(String,String)>`.
- `Page::render_svg` and `render_raw` collect the page again by index, while the other render methods pass the cached `PageInfo` (inconsistent).
- `open`, `from_bytes` and `open_mmap` collect pages during construction, so on an encrypted file with a user password they fail with `Core(EncryptedDocument)`. Such files are opened with the `*_with_password` constructors — they collect pages after core `authenticate`. A document that opens with an empty user password returns "already authenticated" from core `authenticate`, so it opens with any password. The facade `authenticate` is reached only on files with an empty user password (#99; the maintainer chose adding password constructors — lazy page collection was not chosen because it changes meaning: `page_count()` would be 0 before authentication).
- `Document::modify` passes `&self.inner` to `DocumentModifier::from_document` — the authentication state carries over. Saving an encrypted document through `modify()` gives a file without `/Encrypt` by default (measured 2026-09-28, `aes256_r5_empty_user.pdf`). `Modifier::preserve_encryption` writes with the same encryption as the source (keep-source in [Document modifier](document-modifier.md)), and a new password is set with `inner_mut().set_encryption` (#102).
- Feature flags: `mmap` (core), `parallel` (render), `async` (tokio — only reading is async; parsing is sync). `arena` is passed through to core, but the facade code gates nothing on it.

## Code
- `justpdf/src/lib.rs` — `Document`, `open`, `from_bytes`, `open_mmap`, `open_with_password`, `from_bytes_with_password`, `open_mmap_with_password`, `authenticated`, `authenticate`, `pages`, `PageIter`, `metadata`, `text`, `search`, `outlines`, `annotations`, `form_fields`, `embedded_files`, `signatures`, `modify`, `Page`, `render_png`, `render_svg`, `render_raw`, `Modifier`, `preserve_encryption`, `merge`, `merge_bytes`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [Document access](document-access.md), [Page tree](page-tree.md), [Text extraction](text-extraction.md), [Text search](text-search.md), [Render API](render-api.md), [Document modifier](document-modifier.md) — wrapped. Check whether a lower-level signature change stops here or leaks through.
- [Outlines](outlines.md), [Annotations](annotations.md), [AcroForm](acroform.md), [Embedded files](embedded-files.md), [Signature detection](signature-detection.md), [Page labels](page-labels.md), [Linearization](linearization.md) — read-only exposure.
- [Crate layering](crate-layering.md) — the dependency layout principle.
- [Published docs](published-docs.md) — Rust examples in the README and mdBook call this API. When changing a signature, recompile the examples (no gate).

## Known holes / open
- There are no tests for the mmap, parallel and async features. The CI all-features run leaves out `mmap`.
