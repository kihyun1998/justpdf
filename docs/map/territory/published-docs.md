# Published docs (README, crate READMEs, mdBook, brand assets)

## What it is
The documentation surfaces people outside the repository read: the root `README.md` (GitHub, and every release archive), each crate's `README.md` (its crates.io page), the `docs/` mdBook guide (`docs/book.toml`, chapters under `docs/src/`), and the brand assets under `logo/` that the READMEs and the docs.rs attributes show. Nothing checks these documents when the code changes. Other prose in the repository (`dev/pdf-compress-wasm-design.md`, `docs/mupdf-feature-analysis.md`, `CHANGELOG.md`) is read by contributors, not shipped; the ADRs, `GLOSSARY.md` and `docs/map/` are not part of this territory.

## Governing decisions
**None.**

## Design model
Status: only from reading the documents and the history.
- On 2026-09-23 the examples and feature lists in the README, crate READMEs and mdBook were brought in line with the code in one commit (`0154536`, which also deleted the old `roadmap.md`). Rust examples were checked by compiling them with `cargo check` in a temporary crate outside the repository, and CLI examples against the built binary's `--help`; Python, Node, WASM and C examples were compared by reading the binding sources and headers, not run (map note of 2026-09-23). The same day `dev/pdf-compress-wasm-design.md`, `docs/mupdf-feature-analysis.md` and `CHANGELOG.md` had their false sentences fixed (same note).
- Since then the docs have been edited by hand inside feature PRs (e.g. `06eaddc` #173, `ecb7e72` #336, `a7f6296` #75 touch `README.md`/`docs/src`), and there is still no mechanism that compiles or runs the doc examples (no doctest of the README, no `mdbook build`, no CI job). The crates' rustdoc has one example (`justpdf/src/lib.rs`, `no_run`), which `cargo test` compiles; `justpdf/examples/*.rs` are compiled by CI too, but they are not the README's code (#202).
- Size of the unchecked surface: 6 fenced blocks with a language tag in `README.md` and 42 across the 11 mdBook chapters (`grep -c '^```[a-z]' README.md docs/src/*.md`, 2026-10-30).
- Known unimplemented parts are stated as they are: CLI `sign` is "not implemented yet: it writes nothing and exits with code 1" (`docs/src/cli.md`, `justpdf-cli/README.md`), and standard fonts encode WinAnsi, other characters becoming `?` (`docs/src/creating.md`, since #34) — re-checked 2026-10-30.
- Install lines name registry packages this project has not published or does not own: `justpdf-python/README.md` says `pip install justpdf`, a name that belongs to an unrelated PyPI project; `justpdf-node/README.md` says `npm install justpdf`, which does not exist on npm (both measured 2026-10-30). `cargo install justpdf-cli` (README, mdBook) and `justpdf = "0.1"` match crates.io.
- The mdBook is not built or published anywhere, neither in CI nor linked from the README.
- Brand assets: every crate README shows `logo/readme/justpdf-readme-light.png` through an absolute `raw.githubusercontent.com/kihyun1998/justpdf/master/...` URL (the root README uses the relative path, light and dark), and every library crate's `lib.rs` sets `html_logo_url` (`logo/icons/justpdf-icon-light-128.png`) and `html_favicon_url` (`logo/favicon/favicon-32.png`) to the same kind of URL (`git grep -n 'logo/' -- '*.rs' '*README.md'`; all referenced files exist, checked 2026-10-30). Because the URLs point at `master`, a crates.io page or docs.rs build of an already published version shows whatever sits at that path on `master` now; moving or renaming those files breaks every published version's images (inferred). `logo/README.md` holds the usage rules for the assets.

## Code
- `README.md` — "Rust example" (`Document::open`, `render_png`), "CLI", "Language bindings" (Python, JavaScript / WASM, C)
- `justpdf/README.md`, `justpdf-core/README.md`, `justpdf-render/README.md`, `justpdf-formats/README.md`, `justpdf-special/README.md`, `justpdf-cli/README.md`, `justpdf-ffi/README.md`, `justpdf-python/README.md`, `justpdf-node/README.md`, `justpdf-wasm/README.md`, `justpdf-compress-wasm/README.md` — crates.io pages
- `docs/book.toml` — `[book]`, `[output.html]`
- `docs/src/SUMMARY.md` — table of contents; chapters `cli.md`, `bindings.md`, `creating.md`, `reading.md`, `rendering.md`, `modifying.md`, `text-extraction.md`, `formats.md`, `getting-started.md`, `introduction.md`, `api-reference.md`
- `logo/README.md` — asset usage; `logo/readme/`, `logo/icons/`, `logo/favicon/`, `logo/social/`, `logo/macos/`
- `*/src/lib.rs` — `#![doc(html_logo_url, html_favicon_url)]` in every library crate

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [Facade](facade.md), [CLI](cli.md), [Language bindings](language-bindings.md), [Format document](format-document.md), [Document modifier](document-modifier.md) — every public-API or flag change: the docs name these APIs and nothing compiles them, so a rename leaves them wrong with CI green. Edges from this map to the documentation surface converge here.
- [Crate publishing](crate-publishing.md) — install lines and crate READMEs depend on the registry names and versions actually published.
- [CI](ci.md) — there is no doctest of the README and no mdBook build.
- [Release](release.md) — the root README goes into every archive as it is at the tag.

## Known holes / open
- There is no mechanism that compiles the examples or builds the book; scope (book build only, or Rust fences as doctests, which fences are exempt) needs the maintainer's decision. Tracked: #202
- The Python and Node install lines point at packages this project does not own or has not published (above); depends on the publishing names (open question in [Crate publishing](crate-publishing.md)).
- Absolute `master` URLs for logos tie every published version's images to the current `master` layout (above).
- The README lists "signature detection and verification (RSA)", but `verify_signature` cannot parse the zero-padded `/Contents` of a real signature, so no actual signature verifies ([Signature verification](signature-verification.md), measured 2026-10-30 by the map rewrite).
