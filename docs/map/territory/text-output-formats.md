# Text output formats

## What it is
Formats an extracted `PageText` as plain / HTML / JSON / Markdown. CLI `text --format` is the only consumer.

## Governing decisions
**None.**

## Design model
- JSON uses hand-written escaping (`json_string`).
- Markdown is block text with no heading detection; with several pages it adds `## Page N`.

## Code
- `justpdf-core/src/text/format.rs` — `OutputFormat`, `format_page`, `format_pages`, `format_html`, `format_json`, `format_markdown`, `json_string`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [Text extraction](text-extraction.md) — the input structure.
- [CLI](cli.md) — the format name mapping in `cmd_text`. Adding a format means the CLI help too.
- [Published docs](published-docs.md) — the format list in the CLI docs.

## Known holes / open
**None.** No holes were found while writing this note.
