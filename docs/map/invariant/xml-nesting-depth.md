# XML nesting depth

## The fact
Every reader that parses XML from outside does so with roxmltree, at version 0.21.2 or later, and walks the resulting tree to at most `MAX_XML_DEPTH` (256, `justpdf-formats/src/common.rs`) levels below the node it starts at, skipping what lies deeper. Where depth 0 is differs by walker: the root element for SVG and XPS, the document node for EPUB (one level fewer), each paragraph or section for FB2, and for Office only nested hyperlinks count. Both halves are needed: roxmltree 0.20 recursed once per element while parsing, so the parse itself overflowed the stack, and a recursive walk over the parsed tree overflows the same way. Check: a document of 100,000 nested elements reads without aborting, through the parse and through every walk.

## Why it is cross-cutting
Seven walkers in five format modules recurse over roxmltree nodes, and none calls another (inferred, read at `9a9623c`):
- SVG `parse_node` and `collect_text_content`
- EPUB `extract_text_recursive`
- FB2 `collect_text` and `Fb2Document::parse_section` (nested `<section>`)
- Office `extract_docx_paragraph_text` (nested `w:hyperlink`)
- XPS `extract_glyphs_text`

ZUGFeRD in `justpdf-special` parses with roxmltree too, but walks only with roxmltree's own iterators (`descendants`, `children`), which do not recurse (inferred). A new walker written as plain recursion, or a downgrade of roxmltree, reopens the abort for that format alone.

## Territories it holds in
- [SVG input](../territory/svg-input.md) — `parse_node`, `collect_text_content`.
- [EPUB input](../territory/epub-input.md) — `extract_text_recursive`.
- [FB2 input](../territory/fb2-input.md) — `collect_text`, `Fb2Document::parse_section`.
- [Office input](../territory/office-input.md) — `extract_docx_paragraph_text`.
- [XPS input](../territory/xps-input.md) — `extract_glyphs_text`.
- [ZUGFeRD](../territory/zugferd.md) — the parse only.

## What a violation looks like
The process aborts with "has overflowed its stack" — no `Result` or `catch_unwind` stops it. A 700 KB SVG of 100,000 nested `<g>` aborted `justpdf convert` with exit 134; on an 8 MB thread roxmltree 0.20 overflowed at 20,000 levels before any justpdf code ran; after the bump the walkers overflowed next (*measured* 2026-11-01, release and the tests below).

## Discovery history
Found by probing the SVG reader while working #425 (2026-10-31). On #449 the maintainer chose to bump roxmltree to 0.21.2, whose CHANGELOG and `tokenizer.rs` (`parse_content`, read raw at tag `v0.21.2`) parse element content iteratively, and to bound every walker rather than SVG's alone, each skipping what lies deeper or failing as its existing error handling does — all of them return partial text or elements, so all skip (2026-11-01). `MAX_XML_DEPTH` is the formats crate's own constant; #122 plans one shared 256 limit with a `LimitExceeded` error for core's recursive parsers, which these readers would join only if it reaches this crate. `parse_section` was missed in the first version: it is a method, and the first enumeration looked at free functions only; the review measured it aborting at 100,000 nested sections. Pinned by a `deeply_nested_*` or `a_deeply_nested_*` test in each module (FB2 has two) and `elements_are_read_to_depth_255_below_the_root` in SVG; removing any walker's check, or moving SVG's by one, failed a named test (*measured* 2026-11-01). roxmltree's `nodes_limit` stays at its default `u32::MAX` in every reader, so a document's node count, and its memory, is bounded only by its size (inferred).

## Where it will recur
**A function or method that recurses over `roxmltree::Node`, or a change to the roxmltree version.** Command: `rg -n 'roxmltree::Node' justpdf-formats/src justpdf-special/src` and check each function and method that calls itself (`Self::` included). Carry a depth and stop at `MAX_XML_DEPTH`, or walk with `descendants()`; test it with 100,000 nested elements.
