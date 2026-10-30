# Text output formats

## What it is
Formats extracted `PageText` as plain text, HTML, JSON or Markdown (`format_page` for one page, `format_pages` for several). CLI `text --format` is the only caller.

## Governing decisions
**None.**

## Design model
From reading the source (`text/format.rs`) and the CLI's `cmd_text`.
- **Plain** is `PageText::plain_text` (blocks joined by a blank line); several pages are joined by a blank line.
- **HTML**: one page is a `<div class="page">` fragment with a `<p>` per block and `<br/>` between lines; several pages are a full document with an `<h2>Page N</h2>` per page. Text is escaped for `&`, `<`, `>` and `"` (`html_escape`).
- **JSON** is written by hand: blocks, their lines with `x`/`y`, and each line's words with `x`, `y`, `width` and `font_size`, numbers printed with `{:.2}`, and `char_count` (characters themselves are not written). Strings are escaped by `json_string`. A non-finite coordinate is printed as `NaN` or `inf`, which is not JSON (measured 2026-10-30, probe: a `Td` of 10^330 gives `"x": NaN`).
- **Markdown** is each block's text with a blank line between blocks; no heading detection and no escaping, so a line starting with `#`, `-` or `1.` becomes Markdown structure (inferred). `format_pages` prefixes `## Page N` and separates pages with `---`. The CLI calls `format_pages` whenever `--page` is not given, so a one-page document gets `## Page 1` too (inferred from `cmd_text`).
- The CLI maps `plain`, `html`, `json`, `markdown` and `md` to `OutputFormat` and rejects any other name.
- **Bounds on input**: output size is proportional to the `PageText` given; nothing is bounded here.

## Code
- `justpdf-core/src/text/format.rs` — `OutputFormat`, `format_page`, `format_pages`, `format_plain`, `format_html`, `format_html_multi`, `html_escape`, `format_json`, `format_json_multi`, `json_string`, `format_markdown`
- `justpdf-cli/src/main.rs` — `cmd_text`

## Reference behaviour
**None.** No comparison with `mutool draw -F text|html|stext` output is recorded.

## Cross-cutting invariants
**None.**

## Blast radius
- [Text extraction](text-extraction.md) — supplies `PageText`; a field whose value can be non-finite, or a renamed field, changes JSON output directly.
- [Reading order](reading-order.md) — every format prints its blocks as they come, so a block change is an output change in all four.
- [CLI](cli.md) — the format name mapping in `cmd_text` and the `--format` help text; a new format needs both.
- [Published docs](published-docs.md) — `docs/src/cli.md` lists the formats (`plain, html, json, markdown`).

## Known holes / open
- JSON output can contain `NaN`/`inf` (measured, above). Tracked: #424.
- Markdown output does not escape text (inferred). Not tracked by an issue.
- The JSON shape (field names, the two-decimal numbers, no characters) is not documented anywhere a consumer could rely on (inferred: `docs/src` names only the format list).
