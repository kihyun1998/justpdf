# Narrowing justpdf-render's public API

`justpdf-render` keeps its internal modules public (`device`, `graphics_state`, `interpreter`, `glyph`, `glyph_cache`, `shading`, `svg_device`, `display_list`, `bbox_device`). They are not being narrowed to `pub(crate)` before 1.0.

## Why this is out of scope

The point of narrowing them would be that an internal refactor stops being a breaking change. Before 1.0 this project accepts such breaks: see [ADR 0006](../docs/adr/0006-breaking-changes-are-allowed-before-1-0.md). With nobody outside the project depending on the crate, narrowing would buy little: a smaller docs.rs page, and the compiler flagging unused internals such as `DisplayList` as dead code.

Measured when this was decided (2026-10-24): workspace consumers use only the `render` module's functions and options and `error`. On crates.io the crate's reverse dependencies are this project's own crates.

Revisit before 1.0, when the public surface becomes a promise.

## Prior requests

- #371: "justpdf-render exposes its internals as pub mod: an internal refactor breaks the public API"
