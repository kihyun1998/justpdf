# Actions

## What it is
Parses action dictionaries (GoTo, GoToR, URI, Named, Launch, JavaScript, SubmitForm, ResetForm, and the rest as `Unknown`) into `PdfAction` and follows a `/Next` chain (`parse_action`, `parse_action_chain`), and builds action dictionaries from the model (`build_action`).

## Governing decisions
**None.** [ADR 0005](../../adr/0005-sanitizing-removes-unpainted-text-by-category.md)'s JavaScript and external-actions categories name action types (JavaScript; URI, Launch, GoToR, GoToE, SubmitForm, ImportData) that [Sanitize](sanitize.md) removes wherever they occur; it decides that operation, not this model.

## Design model
From reading the source; no record governs it.
- **No document**: the functions take only a `&PdfDict`, so they cannot resolve indirect references. A `/Next` that is a reference is not followed, and neither is a `/JS` that is a reference.
- **JavaScript**: `/JS` is read when it is a string, or a stream held directly in the dictionary, as its raw `data` without decoding. The parser builds a stream only at the top level of an indirect object (see [Form flatten](form-flatten.md)), so a `/JS` stream in a parsed file is always a reference and the action parses as `None` (inferred, not run).
- **File specifications**: `/F` is read only as a string; a file specification dictionary reads as the empty string (GoToR, Launch, SubmitForm).
- **GoToR without `/D`** gets `Destination::Named("")`.
- The destination type is [Outlines](outlines.md)' `Destination`, re-exported (`action/types.rs`). A destination whose page is an integer (the form GoToR uses for a page in another file) is held as an `IndirectRef` with that number, so `build_action` writes it back as an indirect reference `n 0 R` (measured 2026-10-30 at 88b2a08, scratch probe on `Destination::from_object` and `to_pdf_array`: `[3 /Fit]` comes back as `[3 0 R /Fit]`).
- Text values are read with `from_utf8_lossy` and written as the Rust string's UTF-8 bytes (`/URI`, `/F`, `/JS`) — [Text string encoding](../invariant/text-string-encoding.md).
- **Bounds on input**: `parse_action_chain` recurses through direct `/Next` dictionaries and arrays with no depth or count limit; only the nesting the object parser builds bounds it (inferred; the parser's nesting limit was not checked here).

## Code
- `justpdf-core/src/action/types.rs` — `PdfAction`, `NamedAction`
- `justpdf-core/src/action/parse.rs` — `parse_action`, `parse_action_chain`
- `justpdf-core/src/action/builder.rs` — `build_action`

## Reference behaviour
**None.** Clause to compare against: ISO 32000-2 §12.6.

## Cross-cutting invariants
- [Text string encoding](../invariant/text-string-encoding.md) — the builder writes Rust strings such as URI and JS as strings without a BOM.

## Blast radius
- [Annotations](annotations.md), [Outlines](outlines.md) — read `/A` by hand (`/A /URI` for links, `/A /D` for outline items), not through this parser; a rule about what an action means, fixed here, stays wrong in those two.
- [Outlines](outlines.md) — owns `Destination`; its integer-page handling is what this module writes back for GoTo and GoToR.
- [Compress stripping](compress-stripping.md) — removes JavaScript and actions for `strip_extras` with its own key list, not this model; the two disagree on what counts as an action.
- [Sanitize](sanitize.md) — ADR 0005 removes JavaScript and external actions by type, including through `/Next` chains (#310's brief); the type set and the chain rule here are the ones it must match (not built: #310).

## Known holes / open
- No code in the workspace outside `action/` calls `parse_action`, `parse_action_chain` or `build_action` (grep at 88b2a08); the facade does not expose them.
- A JavaScript action whose `/JS` is a stream (always a reference in a parsed file) parses as `None`; references in `/Next` are not followed (inferred).
- An integer destination page is written back as an indirect reference (measured above).
- Text strings are written as BOM-less UTF-8 and read as lossy UTF-8. Tracked: #33
- A GoToR destination's integer page is rewritten as an object reference through the shared `Destination` ([Outlines](outlines.md)). Tracked: #423
