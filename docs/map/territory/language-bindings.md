# Aggregate — Language bindings

**This note owns no detail.** How the four thin adapters for calling the Rust API from other languages relate.

| Concept | Note |
|---|---|
| C ABI + hand-written header | [ffi-binding](ffi-binding.md) |
| PyO3 / maturin | [python-binding](python-binding.md) |
| wasm-bindgen general-purpose module | [wasm-binding](wasm-binding.md) |
| napi-rs | [node-binding](node-binding.md) |

## Why they sit together
All four wrap **the same narrow core+render subset** (open, authenticate, page count, text extraction, PNG render, a few Info strings) each on their own. They do not call each other and do not use the [Facade](facade.md), so when the core or render public API changes, the four have to be fixed separately. [ADR-0002](../../adr/0002-language-bindings-outside-workspace.md) groups these four as "general bindings outside the workspace".

## Where ADR-0002 and the repository disagree
ADR-0002 states that all four bindings are excluded from the workspace members and each has an empty `[workspace]` block. State of the repository on 2026-09-23:
- `justpdf-ffi` **is** in the root `Cargo.toml` `members` (added before the ADR).
- `justpdf-ffi` and `justpdf-wasm` have no `[workspace]` block.
- As a result `cargo metadata --manifest-path justpdf-wasm/Cargo.toml` fails with "current package believes it's in a workspace when it's not" — the wasm binding currently cannot be built on its own.
- Only python and node are split off as the ADR says (empty `[workspace]`, their own `Cargo.lock`).

Command to check: `grep -n members Cargo.toml; grep -c '^\[workspace\]' justpdf-{ffi,python,wasm,node}/Cargo.toml`.

Which side to fix (the ADR or the manifests) needs a decision. This map does not change the ADR.

## CI does not look
CI runs only `--workspace` commands, so ffi is compiled only on the host platform, and python, wasm and node are **built by no CI job** (no maturin, napi or wasm32 jobs). They belong to the class that breaks quietly — [CI](ci.md).

## Page size and rotation
Every binding's page size is the visible box (CropBox, else MediaBox) before `/Rotate`, while rendered output applies the rotation since #54 (a 600×200 `/Rotate 90` page renders 200×600). Only Python exposes the rotation (`Page.rotation`, raw `/Rotate`); Node, WASM and FFI have none, so their callers cannot derive the displayed size (read 2026-10-07). Decided in #263: keep the stored size, add a rotation accessor to every binding returning the renderer's normalised 0/90/180/270 (Python's `rotation` normalised too), and document the relation.

Tracked: #36 (justpdf-wasm manifest, ADR-0002), #263 (rotation accessors)
