# The CLI is a finished product — not subject to the dependency layer principle

`justpdf-cli` (binary `justpdf`) is **the one official PDF CLI**. New features are added to this crate as subcommands, not split out into separate lightweight tools. It therefore depends on `core + render + formats` wholesale and does not aim to minimize binary size.

## Relation to ADR 0001

The primary principle of [ADR 0001](./0001-crates-split-by-dependency-layer.md) is "users pull in only as far as the layer they need and so control binary size and compile time". This principle governs **library consumers** — the person who writes a crate into `[dependencies]` picks the dependency graph directly.

CLI users do not pick a dependency graph. They just type `justpdf compress`. Whether the binary contains `render` is an invisible implementation detail to them. So ADR 0001's minimization principle **does not apply to the CLI's feature surface.** One binary bundling every subcommand is the normal case.

## Trade-offs

The alternative considered was a `just-tic`-style **single-purpose lightweight CLI** — a `justpdf-core`-only binary that only compresses. Its advantages are a small cold install and a minimal surface.

Why it was rejected: that path adds a crate, a binary name and a release artifact for every feature, and users have to remember "tool A to merge PDFs, tool B to compress". The value of a single entry point — "`justpdf` alone does everything for PDF" — was judged to outweigh the cost of fragmentation.

## Consequences

- The question "should this feature go into the CLI or be split out into a separate tool?" is settled once and for all by this ADR — **it goes in.**
- If a lightweight single-purpose release is ever genuinely needed (e.g. browser, embedded), it is handled not as the CLI but as a separate first-class product like `compress-wasm` (the identity criterion of [ADR 0002](./0002-language-bindings-outside-workspace.md)).
- CLI binary size is a non-goal. If size is a problem, the answer is the library path that uses the layers directly, not trimming the CLI.
