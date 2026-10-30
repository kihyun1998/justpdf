# CI

## What it is
The GitHub Actions workflow `.github/workflows/ci.yml`, run on every push to `master` and every PR against it: six jobs on `ubuntu-latest` — `check` (workspace check), `test`, `test-features` (all-features test), `feature-isolation` (the isolation script), `clippy`, `fmt` (read in the working tree, 2026-10-30).

## Governing decisions
**None.** No record decides what CI gates. The clippy and fmt policies below are maintainer decisions on issues (#189, #188), not records.

## Design model
Status: from the workflow file and the decisions on #188/#189.
- A failing job does not block a merge by itself: `master` has no branch protection and the repository has no rulesets (`gh api repos/kihyun1998/justpdf/branches/master/protection` → "Branch not protected", `.../rulesets` → `[]`; measured 2026-10-30). "Blocks" below means the job fails, nothing more. The feature-isolation job had been failing on master since 2026-05 until #62 fixed the tree-character problem; PRs merged in that window were merged on red CI — once failure is the standing state, it is not a gate (#62).
- **clippy fails on any warning** since #189: `cargo clippy --workspace --all-targets --features <the all-features list> -- -D warnings`, on **clippy 1.96.0, pinned in that job only** (`dtolnay/rust-toolchain@1.96.0`); every other job uses `@stable`. Maintainer decisions (#189, 2026-10-06), shown with measured counts on `09ef7bd` — 249 findings at the old scope (lib/bin, default features), 462 with `--all-targets`, 484 with the features as well: (1) scope = all targets + the all-features list, so tests, examples and feature-gated code (`justpdf-special` adds 25) are linted; (2) pin the clippy job, because clippy has no stability guarantee and new stable releases add lints (`manual_is_multiple_of`, let-chain `collapsible_if` came that way) — the pin is raised together with the fixes for the new lints; (3) fix everything whose fix keeps behaviour and public API, and allow at the site only the rest. 318 of the 462 `--all-targets` findings were machine-applicable (`cargo clippy --fix`); the tests and feature isolation passed unchanged after them (measured, #189).
- Clippy allows sit at the site; there is no crate- or workspace-wide allow and no `[lints]` table in any manifest. One module-wide allow exists: `#![allow(clippy::write_with_newline)]` in `justpdf-core/src/annot/appearance.rs`. The site list is produced by `git grep -n 'allow(clippy' -- '*.rs'` (28 attribute lines at 88b2a08: `too_many_arguments` in core, render, formats, the CLI's `cmd_render` and compress-wasm's `compress_advanced`; `type_complexity`; `approx_constant`; `should_implement_trait`; `manual_clamp`; `if_same_then_else`). Hand-written reasons the command cannot show: `should_implement_trait` on `fb2::Fb2Document::from_str` keeps public API; `approx_constant` marks test data (`3.14`) in `font/cff.rs` and `journal.rs`; `manual_clamp` on the tiling-pattern cell size because `clamp` passes NaN through where `max(1.0).min(2048.0)` gives 1.0 ([Render tiling patterns](render-tiling-patterns.md)); `if_same_then_else` on the patch corner selection in `parse_patch_mesh` ([Render shading](render-shading.md)), left unmerged because it may be a defect (Tracked: #196); `too_many_arguments` because splitting the parameters is a structure call nobody made (#189).
- **fmt fails on any diff** since #188: `cargo fmt --all -- --check` on `@stable`, unpinned. Pinning is not needed for fmt: rustfmt's stability RFC 2437 makes any change that would fail `rustfmt --check` on previously formatted code opt-in under the default options (maintainer agreed, 2026-10-06). The one reformat commit (`9bf93c7`) is in `.git-blame-ignore-revs`, which works only while that commit keeps its SHA — PR #192 was merged with a merge commit (`09ef7bd`), not squashed or rebased (checked 2026-10-30: `9bf93c7` is an ancestor of master).
- Every cargo job is scoped to `--workspace`. There are no builds for the crates outside the workspace (python, wasm, node) and none for wasm32, maturin or napi — "passing" does not mean these crates were checked ([Language bindings](language-bindings.md)).
- Both test jobs run `cargo test --workspace --no-fail-fast`. Without it cargo stops at the first test binary that fails, so the later crates' tests never run and a second, independent failure only shows on the next CI round (measured on #190's proof commit `322b6df`, a branch commit not on master: a `justpdf-core` failure hid a `justpdf-special` one). `cargo test` also compiles the workspace's examples (e.g. `justpdf/examples/`) and the one rustdoc example (`no_run`, #202).
- The all-features test turns on `justpdf-formats/all`, `justpdf-special/all`, `justpdf/parallel`, `justpdf/arena`, `justpdf/async`, `justpdf-core/mmap` and `justpdf/mmap`, and installs `tesseract-ocr` first: OCR tests return early when `tesseract --version` fails, so the job checks `tesseract --list-langs` lists `eng` before testing, and a green run means the tesseract-backed tests ran (#187). On Ubuntu `tesseract-ocr` depends on `tesseract-ocr-eng`; noble's candidate is 5.3.4 (measured in WSL Ubuntu 24.04, #187). The same feature list is repeated by hand in the clippy job.
- `feature-isolation` runs `./scripts/check-feature-isolation.sh` ([Crate layering](crate-layering.md)); it passed locally at HEAD (measured 2026-10-30).
- No job builds the mdBook (`docs/`) or compiles the README/mdBook code blocks ([Published docs](published-docs.md)). No job runs on macOS or Windows, while the release builds there ([Release](release.md)).

## Code
- `.github/workflows/ci.yml` — jobs `check`, `test`, `test-features`, `feature-isolation`, `clippy`, `fmt`
- `scripts/check-feature-isolation.sh` — run by `feature-isolation`
- `.git-blame-ignore-revs` — the #188 reformat commit

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [Language bindings](language-bindings.md) — outside every `--workspace` job; a core API change that breaks them passes CI.
- [Crate layering](crate-layering.md) — the isolation script is the only check of ADR-0001's feature promises; what it does not list, CI does not check.
- [Published docs](published-docs.md) — nothing checks that the doc examples compile or run.
- [Release](release.md) — the release workflow does not require CI to pass (only a tag push), and builds on macOS and Windows where CI never runs.
- [Facade](facade.md) — the all-features feature list in `test-features` and `clippy` names facade and core features by hand; a new feature is not tested or linted until added to both lines.

## Known holes / open
- The gate's "all" is not the whole repository (above), and the gate does not block merging (no branch protection).
- Whether to pin the toolchain of the jobs other than clippy was not covered by the #189 decision; open question for the maintainer.
- The all-features list is written twice (`test-features`, `clippy`) and kept in step by hand.
- Bindings are not built. Tracked: #201
- README and mdBook examples are not checked. Tracked: #202
