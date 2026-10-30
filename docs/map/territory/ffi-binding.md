# C FFI binding

## What it is
C ABI functions (`justpdf_open` … `justpdf_page_size`, 15 in all) and a hand-written header `include/justpdf.h`. Results come back through out-parameters; the return value is an `int` status code (`JUSTPDF_OK` = 0, `JUSTPDF_ERR_*` = −1 … −7). Published on crates.io as `justpdf-ffi` 0.1.2 (measured 2026-10-30).

## Governing decisions
- [ADR-0002](../../adr/0002-language-bindings-outside-workspace.md) — decides it should be outside the workspace with its own empty `[workspace]`; in fact it is in the root `members` and has no `[workspace]` block (re-checked 2026-10-30, [aggregate](language-bindings.md#where-adr-0002-and-the-manifests-disagree)).

## Design model
Status: only from reading the source and the crate's tests.
- The header is not generated (no cbindgen in any manifest or build script) — changing a Rust signature means updating the header by hand. On 2026-10-30 the header declares the same 15 function names and the same 8 status codes as `src/lib.rs` (compared by reading; argument types were not checked by any tool).
- It calls only core `PdfDocument`, `page::get_page`, `page::page_count` and `text::extract_*_string`, and render `render_page` (PNG).
- `page_error_code` sorts a core error into a status code: `PageOutOfRange` → `JUSTPDF_ERR_OUT_OF_RANGE`, `EncryptedDocument` → `JUSTPDF_ERR_ENCRYPTED`, any other (`CircularReference`, `LimitExceeded`, a malformed page tree, a `/Count` larger than the tree) → `JUSTPDF_ERR_PARSE`. `justpdf_extract_page_text` and `justpdf_page_size` use it for `get_page`; `justpdf_render_page_png` uses it for `RenderError::Core` and returns `JUSTPDF_ERR_RENDER` for every other render error. Whether an index is out of range is core's call — the binding never compares against a page count (#134; the four status-code tests pass, measured 2026-10-30 with `cargo test -p justpdf-ffi`).
- Rendering sorts core errors the same way as page lookup — the maintainer's judgement (#134, 2026-09-30). Shown: an unauthenticated encrypted document gave `-6` from `justpdf_page_size` and `-4` from `justpdf_render_page_png`, while `justpdf_page_count` and `justpdf_extract_all_text` give `-3`; options were routing render's core errors through `page_error_code`, doing that for the whole FFI, or keeping the brief. Chosen: render only. `justpdf_page_count` and `justpdf_extract_all_text` still map every error to `JUSTPDF_ERR_PARSE`, and text extraction after a successful `get_page` does too (`justpdf_extract_page_text`). `justpdf_open`/`justpdf_open_memory` map every parse error to `JUSTPDF_ERR_PARSE`, and `justpdf_authenticate` maps every error to `JUSTPDF_ERR_ENCRYPTED` (inferred).
- Extracted text is returned through `CString::new(s).unwrap_or_default()`: text containing a NUL byte comes back as an empty string with `JUSTPDF_OK` (inferred).
- The crate is `cdylib` and `staticlib` only, so an integration test under `tests/` does not link. The status-code tests are a `#[cfg(test)]` module in `src/lib.rs` that opens documents with `justpdf_open_memory`.
- Outside input: a path or a `(pointer, length)` buffer from C. The binding copies the buffer and adds no size bound; core's limits apply ([Document access](document-access.md)). Null pointers are checked (`JUSTPDF_ERR_NULL_PTR`); everything else about pointer validity is the caller's contract (the `# Safety` sections).

## Code
- `justpdf-ffi/src/lib.rs` — `JustPdfDocument`, `JustPdfImage`, `JUSTPDF_OK`, `JUSTPDF_ERR_PARSE`, `JUSTPDF_ERR_ENCRYPTED`, `page_error_code`, `justpdf_open`, `justpdf_open_memory`, `justpdf_close`, `justpdf_authenticate`, `justpdf_page_count`, `justpdf_version`, `justpdf_is_encrypted`, `justpdf_extract_page_text`, `justpdf_extract_all_text`, `justpdf_free_string`, `justpdf_render_page_png`, `justpdf_image_data`, `justpdf_image_save`, `justpdf_free_image`, `justpdf_page_size`; tests `page_lookup_reports_out_of_range_only_for_an_index_past_the_last_page`, `page_lookup_reports_a_broken_page_tree_as_parse`, `page_lookup_reports_an_unauthenticated_document_as_encrypted`, `render_reports_core_errors_as_page_lookup_does`
- `justpdf-ffi/include/justpdf.h` — `JustPdfDocument`, `JustPdfImage`, `justpdf_open`, `justpdf_page_size`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [Document access](document-access.md), [Page tree](page-tree.md), [Text extraction](text-extraction.md), [Render API](render-api.md) — the status code is chosen from core's error kind (`page_error_code`); a new or renamed `JustPdfError` variant falls into `JUSTPDF_ERR_PARSE` silently.
- [Language bindings](language-bindings.md) — the other three wrap the same calls with their own error mapping; a mapping change here is not shared.
- [Published docs](published-docs.md) — the C examples in the root README, mdBook (`docs/src/bindings.md`) and crate README use the out-parameter + `JUSTPDF_OK` form; a signature change leaves them wrong.
- [CI](ci.md) — compiled and tested on the Linux host only; the header is never compiled by any job.

## Known holes / open
- Nothing checks that the header matches the Rust signatures, and no C compiler ever reads the header in CI.
- A NUL byte in extracted text yields an empty string with `JUSTPDF_OK` (above).
- No rotation accessor; decided in #263 to add `justpdf_page_rotation`. Tracked: #263
- `justpdf_page_count` and `justpdf_extract_all_text` map every error to `JUSTPDF_ERR_PARSE`; to go through `page_error_code`, with `LimitExceeded` and `CircularReference` staying `JUSTPDF_ERR_PARSE` as file damage. Tracked: #148
- Workspace membership disagrees with ADR-0002. Tracked: #36
