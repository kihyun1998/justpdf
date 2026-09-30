# Actions

## What it is
Parses action dictionaries (GoTo, URI, Launch, Named, JavaScript and so on) into a model and follows the chain, and builds action dictionaries from the model.

## Governing decisions
**None.**

## Design model
- It takes only a `&PdfDict`, with no document, so it cannot resolve indirect references. It cannot follow a `/Next` reference. It ignores `/F` file specification dictionaries.
- It reads a JavaScript `/JS` stream as its raw `data`, without decoding (inferred).
- The destination type re-exports `Destination` from [Outlines](outlines.md).

## Code
- `justpdf-core/src/action/types.rs` — `PdfAction`, `NamedAction`
- `justpdf-core/src/action/parse.rs` — `parse_action`, `parse_action_chain`
- `justpdf-core/src/action/builder.rs` — `build_action`

## Reference behaviour
**None.** Clause to compare against: ISO 32000-2 §12.6.

## Cross-cutting invariants
- [Text string encoding](../invariant/text-string-encoding.md) — the builder writes Rust strings such as URI and JS as strings without a BOM.

## Blast radius
- [Annotations](annotations.md), [Outlines](outlines.md) — parse `/A` by hand, without this module. Fixing action parsing means looking at three places.
- [Compress stripping](compress-stripping.md) — JavaScript removal does not use this model.

## Known holes / open
- Nothing outside the module consumes it.
- Tracked: #33 (text string encoding)
