# Python binding

## What it is
A PyO3 module (built with maturin). `Document` (open, authenticate, text, render, Info) and a `Page` that has only size and rotation.

## Governing decisions
- [ADR-0002](../../adr/0002-language-bindings-outside-workspace.md) — split off as the ADR says, with an empty `[workspace]` and its own `Cargo.lock`.

## Design model
- `Page` has no text method (text is `Document.page_text`).
- Only core `PageOutOfRange` (from rendering, `RenderError::Core(PageOutOfRange)`) becomes `IndexError`; every other error becomes `RuntimeError(str(e))` (`page_error`, `render_error`, #134; measured by building the module with cargo and calling it from Python — no test in the repo runs it). `__getitem__` resolves a negative index against `page_count` first, so a document whose `page_count` fails reads as length 0 there and raises `IndexError` (measured on an unauthenticated encrypted document, 2026-09-30; Tracked: #147). Decided in #147: `len()` and indexing raise the `page_count` error through `page_error`; `__repr__` never raises and shows the error in place of the count, since a raising `repr()` breaks printing and debuggers.

## Code
- `justpdf-python/src/lib.rs` — `page_error`, `render_error`, `Document`, `open`, `from_bytes`, `authenticate`, `page`, `text`, `page_text`, `render_page`, `render_page_to_file`, `Page`
- `justpdf-python/pyproject.toml` — `maturin`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [Document access](document-access.md), [Text extraction](text-extraction.md), [Render API](render-api.md).
- [Published docs](published-docs.md) — the Python examples in the root README and mdBook.
- [CI](ci.md) — not built.

## Known holes / open
- Its own `Cargo.lock` moves separately from the root's — core dependency updates are not reflected here.
