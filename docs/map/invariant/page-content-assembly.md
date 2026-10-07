# Page content assembly

## The fact
A page's `/Contents` is one stream or an array of streams. If it is an array, each stream must be decoded and read as a single content **joined with whitespace in between** (a token can straddle a stream boundary). Every place that reads page content must assemble it the same way to get the same operator list.

## Why it is cross-cutting
Each consumer has its own assembly function: the render interpreter, the SVG renderer, the bbox device, text extraction (private), redaction. None calls another. Fixing the handling of indirect arrays, empty streams or decode failures in one place leaves the rest as they were.

## Territories it holds in
- [Render interpreter](../territory/render-interpreter.md) — `get_page_content`, `concat_content_streams`.
- [SVG renderer](../territory/svg-renderer.md) — `get_page_content`, `concat_content_streams`.
- [BBox device](../territory/bbox-device.md) — `get_page_content` ("simplified version"), `concat_streams`.
- [Text extraction](../territory/text-extraction.md) — `get_page_content_data` (private).
- [Redaction](../territory/redaction.md) — `get_page_content_data`.
- Compression (`writer/compress.rs`) — four sites (`collect_image_display_sizes`, `rewrite_color_operators_to_gray`, `subset_embedded_fonts`, `remove_unused_resources`) each match `/Contents` as a stream reference or a direct array, missing an indirect array. Decided in #138: one compress-local helper with a fail-safe for unreadable content; a core-wide assembler is left to #207.
- [Content stream parsing](../territory/content-stream-parsing.md) — the side that receives the assembled bytes.

Command to find them again: `rg -n 'fn get_page_content|fn concat_(content_)?streams' --glob '*.rs' --glob '!target' .`

## What a violation looks like
Content visible in rendering on the same page is missing from text extraction or redaction (or the reverse), or a single operator straddling a stream boundary breaks on one side only. It reproduces only with a `/Contents` made of several streams.

## Discovery history
No recorded incident. On 2026-09-23, while the map was being written, the render and interactive research agents reported the copies. The differences in boundary handling between the copies have not been compared yet.

## Where it will recur
**Any new feature that reads the content of a page (or a Form XObject) as operators is subject to this invariant.** Before writing a new assembly function, find the existing ones with the command above. If assembly is gathered into one public core function, this note shrinks to one line pointing at that function.
