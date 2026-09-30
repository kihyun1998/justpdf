# Language binding crates live outside the workspace

`justpdf-ffi`, `justpdf-python`, `justpdf-wasm` and `justpdf-node` are **deliberately excluded** from the workspace members in the root `Cargo.toml`, and each declares an empty `[workspace]` block in its own `Cargo.toml` to split itself off as a separate workspace. `justpdf-compress-wasm`, by contrast, is inside the workspace.

## Criterion: first-class product vs general language binding

- **Inside the workspace (first-class product)**: `justpdf-compress-wasm`. A finished product: "a PDF compression tool that runs in the browser". It carries CI/tests/versioning along with `core`.
- **Outside the workspace (general binding)**: `ffi/python/wasm/node`. General bindings for users of other languages, each with its own registry (crates.io/PyPI/npm) and build pipeline. Tying them into the workspace makes the lockfile and the feature resolver conflict, so they are split off.

## When adding a new crate

Even for the same wasm/cdylib, decide by its identity.

- "Is this a finished tool that users consume in its own right?" → inside the workspace.
- "Is this a thin adapter for calling the Rust API from another language?" → outside the workspace.

## Trade-offs

The alternative of tying everything into the workspace is simpler, with a single lockfile/shared deps, but the per-binding build tools (maturin, napi-build, wasm-pack) interfere with each other during a workspace build. The gain from build isolation was judged to outweigh the cost of more lockfiles.
