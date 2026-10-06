# CI

## What it is
GitHub Actions jobs that run on every push and PR: workspace check, test, all-features test, the feature isolation script, the map checker, clippy, fmt.

## Governing decisions
**None.**

## Design model
- On master the feature isolation job had been failing since 2026-05 (the tree-character problem, fixed in #62). PRs merged in that window were merged on red CI — once failure is the standing state, it is not a gate.
- **clippy (`-D warnings`) and fmt are `continue-on-error: true`** — a failure does not break the build. In practice they are not gates.
- Every cargo job is scoped to `--workspace`. There are no builds for the crates outside the workspace (python, wasm, node) and none for wasm32, maturin or napi — "passing" does not mean these crates were checked.
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
- Tracked: #59 (CI gate gaps), #188 (fmt), #189 (clippy)
