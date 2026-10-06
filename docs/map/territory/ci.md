# CI

## What it is
GitHub Actions jobs that run on every push and PR: workspace check, test, all-features test, the feature isolation script, the map checker, clippy, fmt.

## Governing decisions
**None.**

## Design model
- On master the feature isolation job had been failing since 2026-05 (the tree-character problem, fixed in #62). PRs merged in that window were merged on red CI — once failure is the standing state, it is not a gate.
- **clippy blocks** since #189: `cargo clippy --workspace --all-targets --features <the all-features list> -- -D warnings`, on **clippy 1.96.0, pinned in that job only** (`dtolnay/rust-toolchain@1.96.0`); every other job stays on `@stable`. Maintainer decisions (2026-10-06), shown with measured counts on `09ef7bd` — 249 findings at the old scope (lib/bin, default features), 462 with `--all-targets`, 484 with the features as well: (1) scope = all targets + the all-features list, so tests, examples and feature-gated code (`justpdf-special` adds 25) are linted; (2) pin the clippy job, because clippy has no stability guarantee and new stable releases add lints (`manual_is_multiple_of`, let-chain `collapsible_if` came that way) — the pin is raised together with the fixes for the new lints; (3) fix everything whose fix keeps behaviour and public API, and allow at the site only the rest (below). Not covered: whether to pin the other jobs' toolchain.
- Site-level clippy allows (no crate- or workspace-wide allow exists): `too_many_arguments` on `compress_advanced` (compress-wasm, wasm-bindgen export), `generate_values_r6` (`crypto/key.rs`, also `type_complexity`), the SVG raster helpers `fill_rect_pixels`, `fill_ellipse_pixels` and `draw_line_pixels`, `render_glyph`, and `parse_patch_mesh` — splitting the parameters is a structure call nobody made; `type_complexity` on `generate_values_r6`'s return type, two `let`s in `writer/compress.rs` and `parse_opf` (`epub`); `should_implement_trait` on `fb2::Fb2Document::from_str` (public API, kept); `approx_constant` on two tests whose `3.14` is test data; `manual_clamp` on the tiling-pattern cell size (`clamp` passes NaN through where `max(1.0).min(2048.0)` gives 1.0 — [Render tiling patterns](render-tiling-patterns.md)); `if_same_then_else` on the patch corner selection ([Render shading](render-shading.md)), left unmerged because it may be a defect.
- 318 of the 462 `--all-targets` findings were machine-applicable (`cargo clippy --fix`); the tests and feature isolation passed unchanged after them (measured).
- **fmt blocks** since #188: `cargo fmt --all -- --check` on `@stable`, unpinned. Pinning is not needed for fmt: rustfmt's stability RFC 2437 makes any change that would fail `rustfmt --check` on previously formatted code opt-in under the default options (maintainer agreed, 2026-10-06; clippy has no such guarantee, so pinning stays open in #189). The one reformat commit is in `.git-blame-ignore-revs`, which only works while that commit keeps its SHA — the PR was merged with a merge commit, not squashed or rebased.
- Every cargo job is scoped to `--workspace`. There are no builds for the crates outside the workspace (python, wasm, node) and none for wasm32, maturin or napi — "passing" does not mean these crates were checked.
- Both test jobs run `cargo test --workspace --no-fail-fast`. Without it cargo stops at the first test binary that fails, so the later crates' tests never run and a second, independent failure only shows on the next CI round (measured on #190's proof commit `322b6df`: a `justpdf-core` failure hid a `justpdf-special` one).
- The all-features test also turns on `justpdf-core/mmap` and `justpdf/mmap`, and installs `tesseract-ocr` first: OCR tests return early when `tesseract --version` fails, so the job checks `tesseract --list-langs` lists `eng` before testing, and a green run means the tesseract-backed tests ran (#187). On Ubuntu `tesseract-ocr` depends on `tesseract-ocr-eng`; noble's candidate is 5.3.4 (measured in WSL Ubuntu 24.04).
- The `map` job runs `scripts/check-map.py .` and blocks: the script exits 1 on any problem. It passes on Linux as on Windows — Linux `os.path.exists` is case-sensitive, Windows is not (measured on a clean `git archive` in WSL Ubuntu 24.04, 0 problems).
- No job builds the mdBook (`docs/`).

## Code
- `.github/workflows/ci.yml` — `check`, `test`, `test-features`, `feature-isolation`, `map`, `clippy`, `fmt`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [Language bindings](language-bindings.md) — outside the gate.
- [Crate layering](crate-layering.md) — the isolation script.
- [Published docs](published-docs.md) — nothing checks that the doc examples compile or run.
- [Release](release.md) — the release workflow does not require CI to pass (only a tag push).

## Known holes / open
- The gate's "all" is not the whole repository (above).
- Tracked: #59 (CI gate gaps)
