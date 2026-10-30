# Release

## What it is
The GitHub Actions workflow `.github/workflows/release.yml`. Pushing a `v*` tag is its only trigger. It builds only `justpdf-cli` in release mode on four targets (`x86_64-unknown-linux-gnu`, `x86_64-apple-darwin`, `aarch64-apple-darwin`, `x86_64-pc-windows-msvc`), bundles the binary with `README.md`, `LICENSE-MIT` and `LICENSE-APACHE` into a `tar.gz` (zip on Windows), and attaches it to the GitHub Release of the same name (`softprops/action-gh-release@v2`).

## Governing decisions
- [ADR-0003](../../adr/0003-cli-is-end-product-not-a-dependency-layer.md) — the CLI is the single end product, so it is the only binary distributed.

## Design model
Status: from reading the workflow; it has never run.
- The workflow has never run: the repository has no tags on `origin` (`git ls-remote --tags origin` empty), no GitHub Releases, and no runs of `release.yml` (`gh run list --workflow release.yml` empty) (all measured 2026-10-30). Nothing in this note has been seen working.
- Publishing to crates.io, PyPI and npm is not in this workflow ([Crate publishing](crate-publishing.md)).
- It does not check that the tag matches any crate version, does not run tests, and does not wait for CI ([CI](ci.md)).
- The toolchain is `rustup update stable` at run time (unpinned), and the build has no `--locked`, so a release can build with a different toolchain than CI used and could resolve dependencies differently from the committed `Cargo.lock` if a manifest moved (inferred).
- The four jobs run with `fail-fast: false` and each uploads to the same release, so a failing target leaves a release with fewer than four archives (inferred).

## Code
- `.github/workflows/release.yml` — job `build` (matrix `target`/`os`/`archive`), steps "Build release binary", "Package (tar.gz)", "Package (zip)", "Upload to GitHub Release"

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [CLI](cli.md) — the packaging copies `target/<target>/release/justpdf` by name; renaming the binary or adding a second one breaks the archive step.
- [Crate publishing](crate-publishing.md) — the tag is the only version a binary carries, and nothing ties it to the crate versions published by hand.
- [Published docs](published-docs.md) — the archive includes the root README as it is at the tag, examples and all.
- [CI](ci.md) — CI never builds on macOS or Windows, so a platform-specific break first shows here.

## Known holes / open
- Never run (above): whether it works on all four targets is unknown.
- No tag-to-version check and no test step (above).
- The workflow's comments are in Korean, against the repository's English-only rule (`CLAUDE.md` → Language).
