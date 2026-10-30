# Python binding

## What it is
A PyO3 module named `justpdf` (crate `justpdf-python`, `[lib] name = "justpdf"`, built with maturin). `Document` (open, from_bytes, authenticate, page count, text, render, Title/Author/Subject, `len`/indexing) and a `Page` that has only width, height and rotation.

## Governing decisions
- [ADR-0002](../../adr/0002-language-bindings-outside-workspace.md) — split off as the ADR says: an empty `[workspace]` in `justpdf-python/Cargo.toml` and its own `Cargo.lock` (re-checked 2026-10-30).

## Design model
Status: only from reading the source; no test in the repo runs the module.
- `Page` holds only a `PageInfo` copy and has no text method (text is `Document.page_text`). `Page.width`/`height` are the CropBox (else MediaBox) before `/Rotate`; `Page.rotation` is the raw `/Rotate` ([aggregate](language-bindings.md#page-size-and-rotation)).
- Error mapping: `page_error` turns core `PageOutOfRange` into `IndexError` and everything else into `RuntimeError(str(e))`; it serves `page` and the `get_page` step of `page_text`. `render_error` does the same for `render_page` (`RenderError::Core(PageOutOfRange)` → `IndexError`). `authenticate`, `page_count`, `text` and the extraction step of `page_text` map every error to `RuntimeError` directly; `open` maps every error to `IOError` (a parse error too) and `from_bytes` to `ValueError` (inferred). The mapping was measured by building the module with cargo and calling it from Python (#134, 2026-09-30).
- `__len__` and `__repr__` use `page_count(...).unwrap_or(0)`, and `__getitem__` resolves a negative index against that length and refuses any index `>= len` with `IndexError`. So a document whose `page_count` fails reads as length 0 and every index raises `IndexError` (measured on an unauthenticated encrypted document, 2026-09-30; still the code at 88b2a08). Decided in #147 (maintainer, 2026-10-07): `len()` and indexing raise the `page_count` error through `page_error`; `__repr__` never raises and shows the error in place of the count, since a raising `repr()` breaks printing and debuggers. Not implemented.
- `/Info` strings (`title`, `author`, `subject`) are read by the binding's own `get_info_string` as lossy UTF-8 — see [Text string encoding](../invariant/text-string-encoding.md).
- Packaging: `pyproject.toml` names the distribution `justpdf` and has neither `project.version` nor `dynamic = ["version"]`, which makes maturin stop before building (measured with maturin 1.15.0, #201, 2026-09-30). The name `justpdf` on PyPI belongs to an unrelated project (Pujan-Dev/justpdf, 0.0.2, measured 2026-10-30), so the README's `pip install justpdf` installs that project.
- Outside input: a path or `bytes` from Python; the binding adds no size bound and core's limits apply ([Document access](document-access.md)).

## Code
- `justpdf-python/src/lib.rs` — `page_error`, `render_error`, `Document`, `Document::open`, `from_bytes`, `authenticate`, `page_count`, `version`, `is_encrypted`, `page`, `text`, `page_text`, `render_page`, `render_page_to_file`, `title`, `author`, `subject`, `__repr__`, `__len__`, `__getitem__`, `get_info_string`; `Page`, `Page::width`, `height`, `rotation`; `open`; `justpdf` (the `#[pymodule]`)
- `justpdf-python/pyproject.toml` — `maturin`, `name = "justpdf"`

## Reference behaviour
**None.**

## Cross-cutting invariants
- [Text string encoding](../invariant/text-string-encoding.md) — `get_info_string` reads `/Info` values as lossy UTF-8.

## Blast radius
- [Document access](document-access.md), [Page tree](page-tree.md), [Text extraction](text-extraction.md), [Render API](render-api.md) — the Python exception type is chosen from core's error kind; a change in which kind core returns for an index or a damaged tree changes `IndexError` vs `RuntimeError`.
- [Language bindings](language-bindings.md) — the same calls wrapped separately in three other bindings.
- [Published docs](published-docs.md) — the Python examples in the root README, mdBook (`docs/src/bindings.md`) and crate README name `justpdf.open`, `page_count`, `page_text`, `render_page`; and the install line points at a PyPI name this project does not own.
- [Crate publishing](crate-publishing.md) — the distribution name and the missing version are publishing facts.
- [CI](ci.md) — not built by any job.

## Known holes / open
- Its own `Cargo.lock` moves separately from the root's — core dependency updates are not reflected here until someone builds it.
- It did not build locally on 2026-10-30: PyO3 0.23.5 refuses Python 3.14 (measured, `cargo check` with `PYO3_PYTHON=python3`). Which Python versions the binding supports is not written down.
- The PyPI name `justpdf` is taken by another project (above). Which distribution name to publish under is an open question for the maintainer.
- `len()`/indexing/`repr()` hide page-count errors. Tracked: #147
- `rotation` is raw; to be normalised. Tracked: #263
- Not built in CI, and maturin cannot build it without a version. Tracked: #201
