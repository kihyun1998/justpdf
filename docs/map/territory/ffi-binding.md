# C FFI 바인딩

## What it is
C ABI 함수(`justpdf_open` … `justpdf_page_size`)와 손으로 쓴 헤더 `include/justpdf.h`. 결과는 out-parameter로 돌려주고 반환값은 `int` 상태 코드다.

## Governing decisions
- [ADR-0002](../../adr/0002-language-bindings-outside-workspace.md) — 워크스페이스 밖이어야 한다고 정하지만, 실제로는 루트 `members`에 들어 있다([aggregate](language-bindings.md#adr-0002와-저장소가-어긋나는-지점)).

## Design model
- 헤더는 생성되지 않는다(cbindgen 흔적 없음) — Rust 시그니처를 바꾸면 헤더를 손으로 맞춰야 한다.
- core `PdfDocument`·`page`·`text`와 render `render_page`(PNG)만 부른다.
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
- [문서 접근](document-access.md), [텍스트 추출](text-extraction.md), [렌더 API](render-api.md) — 감싸는 대상.
- [게시 문서](published-docs.md) — 루트 README·mdBook·크레이트 README의 C 예제(out-parameter + `JUSTPDF_OK` 형태).
- [CI](ci.md) — 호스트 컴파일만.

## Known holes / open
- 헤더와 Rust 시그니처의 일치를 검사하는 장치가 없다.
- Tracked: #36 (justpdf-wasm 매니페스트·ADR-0002), #148 (`justpdf_page_count` and `justpdf_extract_all_text` map every error to `JUSTPDF_ERR_PARSE`)
