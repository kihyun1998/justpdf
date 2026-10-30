# Redaction (applying redactions)

## What it is
Applies a page's Redact annotations (`apply_redactions`): filters the page content inside each **redaction area** (GLOSSARY), appends filled cover rectangles, and removes the Redact annotations. The promise it is held to is an **applied redaction** (GLOSSARY, [ADR 0004](../../adr/0004-redaction-never-under-erases.md)): nothing that would paint inside an area can be recovered from the file. The code at HEAD does not keep that promise; the gap is below.

## Governing decisions
- [ADR 0004 — Redaction never under-erases](../../adr/0004-redaction-never-under-erases.md): the promise and the rules for text, paths, images, Form XObjects, optional content, other annotations and incremental save, and the shared interpreter. The code predates it; each gap is an open issue under #203.
- [ADR 0005 — Sanitizing removes unpainted text by category](../../adr/0005-sanitizing-removes-unpainted-text-by-category.md): what redaction leaves alone. **Unpainted text** (metadata, bookmarks, attachments, scripts, annotation text) has no position for an area to decide, so [Sanitize](sanitize.md) removes it by category and redaction does not.

## Design model
### ADR 0004's rules against the code at HEAD
| ADR 0004 rule | At 88b2a08 | Evidence | Tracked |
|---|---|---|---|
| Text erased per glyph, glyph box through the text rendering matrix and the CTM | **Not built.** A whole `BT…ET` block is removed when the start point of any of its text-showing operators is inside an area | `filter_content_ops`; measured below | #209 |
| `Tr 3` text erased the same way | Only as a side effect: the block rule ignores `Tr` (inferred) | `filter_content_ops` | #209 |
| `/ActualText`, `/Alt` on marked content enclosing an erased glyph removed | **Not built**; `BMC`/`BDC` are passed through | `filter_content_ops` | #215 |
| Vector paths and shadings painting inside the area removed | **Not built**: paths and `sh` are kept | measured below | #210 |
| Image pixels in the area overwritten, image re-stored | **Not built**: a `Do` or `BI` whose box overlaps an area is removed whole | `filter_content_ops` | #211 |
| Form XObjects entered and filtered per call | **Not built**: a form's `Do` is treated as an image (removed whole on overlap, never entered) | `filter_content_ops` | #214 |
| Optional content judged by geometry, not visibility | Holds: the filter never reads `/OC` (inferred) | `filter_content_ops` | — |
| Other annotations overlapping the area removed; widgets detached; `/Thumb` removed | **Not built**: only Redact annotations are removed | `apply_redactions` | #212 |
| Incremental save refused after applying redactions | **Not built**: `incremental_save` has no such check | `writer/modify.rs — incremental_save` | #213 |
| Glyph positions from the interpreter text extraction uses | The shared interpreter exists (`content/interpret.rs — interpret`, `ContentVisitor`, `Glyph` with its `quad`), and text extraction uses it (`text/mod.rs`); `redact.rs` does not | grep: `interpret(` has no caller in `annot/` | #207 (built), #209 |
| Nothing recoverable from the file | **Not held**: the old content stream stays in the output | measured below | #39 |
| Structure tree `/Alt`, `/ActualText` | Out of ADR 0004's scope; not touched | — | #215 |
| Drawing `OverlayText` | Out of ADR 0004's scope; `RedactInfo::overlay_text` is read and never used (`dead_code`) | `RedactInfo` | — |

Measured 2026-10-30 at 88b2a08 with a scratch probe (`add_annotation` of a Redact, then `apply_redactions`, then `build`):
- `q 1 0 0 1 300 300 cm BT … 0 0 Td (Secret) Tj ET Q` with the area at the painted spot `[290 290 400 320]`: the new page content still holds `Secret`. With the area at the untransformed origin `[−5 −5 20 20]`, it is removed. The CTM is not applied to text.
- `14 TL … 50 100 Td (a) Tj T* T* T* T* T* (Secret) Tj` (the second string at y = 30): an area over y 25…35 leaves it; an area over y 38…42 removes it, because `T*` moves by a fixed 12 units.
- `1 0 0 rg 100 100 50 50 re f` under an area `[90 90 160 160]`: the path is kept, and the cover is appended after it.
- In every case the output bytes still hold `Secret`: `apply_redactions` points `/Contents` at a new stream and `DocumentModifier::build` does not garbage-collect, so the old stream object is written too.

### What the filter does (`filter_content_ops`)
- **Text position**: computed from `Tm` (its `e f` only), `Td`/`TD` (added) and `T*` (−12, "approximate leading"); `'` and `"` do not move it. Neither the CTM nor the font size enters it.
- **Removal**: when a showing operator's start point is in an area (edges included, `point_in_rect`), the operators of the block already emitted are drained back to its `BT` and the rest up to `ET` are skipped.
- **Images and forms**: the box is `[e, f, e + a, f + d]` from the most recent `cm`'s operands, not composed with earlier ones and ignoring `b` and `c`; there is no `q`/`Q` stack. The box is normalised (`Rect::normalized`) for a flipped image. `rects_overlap` needs an overlap with some area; touching edges do not count. The areas come normalised from `Rect::from_pdf_array` (#345; until then `point_in_rect` and `rects_overlap` each normalised their arguments by hand).
- **Inline images** are filtered by the same rule as `Do` (maintainer decision, 2026-09-25, #29 check-it; the alternative offered was a comment on #39). Before #29 the rewrite destroyed every inline image on the page; once it preserved them, images inside the area stayed under the cover, so they were added (#29).
- **Rewrite**: the kept operators go through `write_content`, so content outside the areas reads back unchanged, inline images and strings with unbalanced parentheses included (`test_redaction_keeps_content_outside_the_area_unchanged`, #29) — [Object syntax roundtrip](../invariant/object-syntax-roundtrip.md). The cover is `q`, one `rg`/`g`/`k` and `re f` per area from `/IC` (black without one), `Q`, written as `Number` (#90).
- **Page content** is read by its own `get_page_content_data`, joining an array's streams with `\n` — [Page content assembly](../invariant/page-content-assembly.md).

### Inputs and bounds
- The whole page content is decoded and parsed into memory (`stream::decode_stream`, `parse_content_stream`); nothing bounds its decoded size (Tracked: #161). The filter is one pass over the operators, testing each against every area: operators × areas (inferred).
- The page is found with `collect_pages`; an index past the last page is `AnnotationError`, not `PageOutOfRange` (decided in #150: look the page up with `get_page`, inheriting #134's `PageOutOfRange`/`InvalidObject` rule).

## Code
- `justpdf-core/src/annot/redact.rs` — `apply_redactions`, `RedactInfo`, `filter_content_ops`, `operand_f64`, `point_in_rect`, `rects_overlap`, `get_page_content_data`
- `justpdf-core/tests/integration.rs` — `test_redaction_apply`, `test_redaction_keeps_content_outside_the_area_unchanged`
- `justpdf-core/src/annot/redact.rs` (tests) — `a_redact_rect_written_upper_right_first_still_erases_text`, `a_flipped_image_inside_the_redact_rect_is_removed`

## Reference behaviour
- MuPDF `pdf_redact_page` (per-glyph filtering with full graphics state; options for images and line art), as ADR 0004 records it. ADR 0004 departs from it on partially covered line art: removed, not kept.

## Cross-cutting invariants
- [Object syntax roundtrip](../invariant/object-syntax-roundtrip.md)
- [Page content assembly](../invariant/page-content-assembly.md)

## Blast radius
- [Content stream parsing](content-stream-parsing.md) — the filter relies on `parse_content_stream` and `write_content` round-tripping every operator it keeps; a lossy change there alters content outside the areas.
- [Content interpreter](content-interpreter.md) — ADR 0004 requires redaction to take glyph boxes from it (#209); a change to its glyph box (`Glyph::quad`) or form entry changes what redaction will erase once it does.
- [Text extraction](text-extraction.md) — ADR 0004's test property is that a glyph extraction reports inside an area is one redaction erases; the two must use the same positions. No test checks it today.
- [Document modifier](document-modifier.md) — the promise depends on `build` not writing the replaced content stream; it does write it today (#39).
- [Incremental save](incremental-save.md) — appends to the original revision, which holds the erased content; ADR 0004 requires it to refuse a redacted modifier (#213).
- [Annotations](annotations.md) — the Redact keys (`/Rect`, `/IC`, `/OverlayText`) that `AnnotationBuilder::redact` writes are the ones read here; ADR 0004 also makes every other annotation type an input (#212).
- [Sanitize](sanitize.md) — ADR 0005 divides the work: what has a position is redaction's, the rest sanitize's; sanitize's unseen-content categories filter page content through the same interpreter, and both share the incremental-save refusal (#213). Not built: #310, #321.

## Known holes / open
- Every "Not built" row in the table above. Tracked: #209 text, #210 paths, #211 images, #214 Form XObjects, #212 other annotations and `/Thumb`, #213 incremental save, #215 structure tree.
- The erased content stays in the output file. Tracked: #39
- On a page with no content, the Redact annotations are not removed: the whole update is inside `if !content_data.is_empty()`. A Redact without a usable `/Rect` is dropped from `/Annots` without erasing anything. Tracked: #39
- `test_redaction_apply` only checks that the Redact annotation is gone, not that the text was erased.
- An index past the last page is `AnnotationError`. Tracked: #150
- Unpainted text (metadata, bookmarks, attachments, scripts, annotation text) stays after redaction by design; removing it is sanitize's. Tracked: #310 (unseen content in the page: #321)
