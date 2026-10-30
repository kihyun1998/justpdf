# Aggregate — Language bindings

**This note owns no detail.** How the four thin adapters for calling the Rust API from other languages relate.

| Concept | Note |
|---|---|
| C ABI + hand-written header | [ffi-binding](ffi-binding.md) |
| PyO3 / maturin | [python-binding](python-binding.md) |
| wasm-bindgen general-purpose module | [wasm-binding](wasm-binding.md) |
| napi-rs | [node-binding](node-binding.md) |

## Why they sit together
All four wrap **the same narrow core+render subset** — open, authenticate, page count, text extraction, PNG render, page size, a few `/Info` strings — each on its own. They do not call each other and do not use the [Facade](facade.md): each `lib.rs` calls `PdfDocument`, `page::get_page`/`page_count`, `text::extract_*_string` and `justpdf_render::render_page` directly, and Python, Node and WASM each carry their own copy of `get_info_string` (inferred, read 2026-10-30). So a change to the core or render public API has to be fixed four times, and a fix to one binding's decoding or error mapping does not reach the others. [ADR-0002](../../adr/0002-language-bindings-outside-workspace.md) groups these four as "general bindings outside the workspace".

## Where ADR-0002 and the manifests disagree
ADR-0002 states that all four bindings are excluded from the root workspace members and each declares an empty `[workspace]` block. Re-checked against the manifests on 2026-10-30 (HEAD 88b2a08), unchanged since 2026-09-23:
- `justpdf-ffi` **is** in the root `Cargo.toml` `members` (added before the ADR, d985dfa per #36).
- `justpdf-ffi` and `justpdf-wasm` have no `[workspace]` block.
- As a result `cargo metadata --manifest-path justpdf-wasm/Cargo.toml` fails with "current package believes it's in a workspace when it's not" (measured 2026-10-30) — the wasm binding cannot be built at all until one side changes.
- Only python and node are split off as the ADR says (empty `[workspace]`, their own `Cargo.lock`).

Command to check: `grep -n members Cargo.toml; grep -c '^\[workspace\]' justpdf-{ffi,python,wasm,node}/Cargo.toml`.

Which side to fix (the ADR or the manifests) is the maintainer's decision. Tracked: #36

## What CI does not see
Every CI job is `--workspace`, so ffi is compiled and tested on the Linux host only, and python, wasm and node are **built by no CI job** (no maturin, napi or wasm32 job) — [CI](ci.md). On 2026-10-30 `cargo check` in `justpdf-node` passed only after cargo rewrote its committed `Cargo.lock` (core moved to 0.1.4), and `justpdf-python` did not build against the local Python 3.14 because PyO3 0.23.5 refuses it (both measured; the lock changes were reverted). Tracked: #201

## Page size and rotation
Every binding's page size is the visible box (CropBox, else MediaBox) before `/Rotate`, while rendered output applies the rotation since #54 (a 600×200 `/Rotate 90` page renders 200×600, CHANGELOG). Only Python exposes the rotation (`Page.rotation`, raw `/Rotate`); Node, WASM and FFI have none, so their callers cannot derive the displayed size (read 2026-10-30). Decided in #263 (maintainer, 2026-10-07): keep the stored size, add a rotation accessor to every binding returning the renderer's `normalize_rotation` value (0/90/180/270, Python's `rotation` normalised too), and document the relation. Tracked: #263
