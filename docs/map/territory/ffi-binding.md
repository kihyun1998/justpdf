# C FFI binding

## What it is
C ABI functions (`justpdf_open` … `justpdf_page_size`) and a hand-written header `include/justpdf.h`. Results come back through out-parameters and the return value is an `int` status code.

## Governing decisions
- [ADR-0002](../../adr/0002-language-bindings-outside-workspace.md) — decides it should be outside the workspace, but it is in fact in the root `members` ([aggregate](language-bindings.md#where-adr-0002-and-the-repository-disagree)).

## Design model
- The header is not generated (no trace of cbindgen) — changing a Rust signature means updating the header by hand.
- It calls only core `PdfDocument`, `page` and `text`, and render `render_page` (PNG).
- `page_error_code` sorts a core error into a status code: `PageOutOfRange` → `JUSTPDF_ERR_OUT_OF_RANGE`, `EncryptedDocument` → `JUSTPDF_ERR_ENCRYPTED`, any other (`CircularReference`, `LimitExceeded`, a malformed page tree, a `/Count` larger than the tree) → `JUSTPDF_ERR_PARSE`. `justpdf_extract_page_text` and `justpdf_page_size` use it for `get_page`; `justpdf_render_page_png` uses it for `RenderError::Core` and returns `JUSTPDF_ERR_RENDER` for every other render error. Whether an index is out of range is core's call — the binding never compares against a page count (#134; measured by the crate's tests).
- Rendering sorts core errors the same way as page lookup — the **maintainer's judgement** (2026-09-30, #134). Shown: an unauthenticated encrypted document gave `-6` from `justpdf_page_size` and `-4` from `justpdf_render_page_png`, while `justpdf_page_count` and `justpdf_extract_all_text` give `-3`; options were routing render's core errors through `page_error_code`, doing that for the whole FFI, or keeping the brief (`RENDER` for everything but out-of-range). Chosen: render only. `justpdf_page_count` and `justpdf_extract_all_text` still map every error to `JUSTPDF_ERR_PARSE` — that call did not cover them.
- The crate is `cdylib` and `staticlib` only, so an integration test under `tests/` does not link. The status-code tests are a `#[cfg(test)]` module in `src/lib.rs` that opens documents with `justpdf_open_memory`.

## Code
- `justpdf-ffi/src/lib.rs` — `page_error_code`, `justpdf_open`, `justpdf_open_memory`, `justpdf_close`, `justpdf_authenticate`, `justpdf_page_count`, `justpdf_extract_page_text`, `justpdf_extract_all_text`, `justpdf_free_string`, `justpdf_render_page_png`, `justpdf_free_image`, `justpdf_page_size`, `JustPdfDocument`, `JustPdfImage`
- `justpdf-ffi/include/justpdf.h` — `justpdf_open`, `JustPdfDocument`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [Document access](document-access.md), [Text extraction](text-extraction.md), [Render API](render-api.md) — what it wraps.
- [Published docs](published-docs.md) — the C examples in the root README, mdBook and crate README (out-parameter + `JUSTPDF_OK` form).
- [CI](ci.md) — host compilation only.

## Known holes / open
- Nothing checks that the header matches the Rust signatures.
- Tracked: #36 (justpdf-wasm manifest, ADR-0002), #148 (`justpdf_page_count` and `justpdf_extract_all_text` map every error to `JUSTPDF_ERR_PARSE`; to go through `page_error_code`, with `LimitExceeded` and `CircularReference` staying `JUSTPDF_ERR_PARSE` as file damage)
