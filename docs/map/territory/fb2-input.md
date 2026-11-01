# FB2 input

## What it is
Reads a FictionBook 2 XML file — book title, author and the text of each top-level `section` — and writes each section to one PDF page as wrapped Courier lines.

## Governing decisions
- [ADR-0001](../../adr/0001-crates-split-by-dependency-layer.md) — `fb2` (`roxmltree`) pulls render for its preview (#2, closed 2026-05-08); the isolation script checks that it does.

## Design model
Status: read from the source only.
- Encoding: the bytes must be UTF-8 (`invalid UTF-8 in FB2`); the XML declaration's `encoding` is not read, so a windows-1251 book is refused (inferred).
- Metadata: `description/title-info/book-title` and the last `author`'s first and last name.
- Sections: each `section` directly under each `body` is one `FormatDocument` page. Inside it, `title`, `p`, `epigraph`, `poem` and `cite` become paragraphs; a nested `section` is flattened into its parent (its title becomes a paragraph). Images (`binary`, `image`) are ignored.
- `to_pdf`: one PDF page per section. The title is cut to one line; paragraphs wrap with `wrap_fixed_pitch` at `usable_width / (10 × 0.6)` characters. Lines below the bottom margin are dropped, so a section longer than a page loses the rest. Each line goes through `move_to` ([Format document](format-document.md)).
- Preview: `build_section_pdf` repeats the `to_pdf` loop for one section and renders it.
- Input bound: none beyond roxmltree's own limits; the file is read whole (inferred). Nesting: `collect_text` reads at most 256 levels below the node it starts at and `parse_section` flattens nested `<section>` at most 256 deep (`MAX_XML_DEPTH`), and roxmltree 0.21.2 parses without recursing (#449; [XML nesting depth](../invariant/xml-nesting-depth.md)).

## Code
- `justpdf-formats/src/fb2/mod.rs` — `Fb2Document`, `Fb2Document::from_bytes`, `Fb2Document::from_str`, `Fb2Document::open`, `Fb2Document::extract_metadata`, `Fb2Document::extract_body`, `Fb2Document::parse_section`, `Fb2Document::build_section_pdf`, `collect_text`, `wrap_text`, `impl FormatDocument for Fb2Document` (`render_page`, `render_page_png`, `to_pdf`)

## Reference behaviour
**None.**

## Cross-cutting invariants
- [XML nesting depth](../invariant/xml-nesting-depth.md)
- [Content text encoding](../invariant/content-text-encoding.md)

## Blast radius
- [CLI](cli.md) — `convert` reads `.fb2` (`justpdf-cli/tests/convert.rs`: `fb2_converts_to_pdf`, `fb2_and_mobi_convert_to_png`).
- [Render API](render-api.md) — the preview assumes `render_page_to_pixmap` returns RGBA and `render_page` PNG.
- [Document builder](document-builder.md) — assumes `show_text` writes WinAnsi for Courier and that `move_to` is absolute.

## Known holes / open
- Text past one page per section is dropped (Design model).
- Non-UTF-8 books are refused (inferred).
- Caller-supplied fonts and width-based layout. Tracked: #174
