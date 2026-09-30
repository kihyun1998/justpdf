# Redaction (applying redactions)

## What it is
Strips the page content inside Redact annotation areas, draws filled rectangles, then deletes the Redact annotations. Its promise is not "erase what is visible" but "remove the information from the file".

## Governing decisions
**None.**

## Design model
The actual rules of the content filter (`filter_content_ops`):
- Text: if the **start position** of a text-showing operator is inside the area, the whole BT…ET block is deleted. The position is computed from `Tm`/`Td`/`TD` alone, and `T*` hard-codes a 12pt leading ("approximate leading"). The CTM is not applied to text.
- Images: deletes a `Do` (image or Form, not told apart) whose box, computed from the single preceding `cm`, overlaps the area, and inline images (`BI`). There is no q/Q stack and no matrix accumulation.
  - Inline images were added in #29. Before #29 the rewrite destroyed every inline image on the page, so images inside the area disappeared by accident too. Once the rewrite preserved images, images inside the area stayed under the cover (even after `garbage_collect`), so they were added as well. **Maintainer decision (2026-09-25, #29 check-it)**: filter them by the same rule as `Do`. Alternative offered: only leave a comment on #39. Precise geometry (CTM accumulation) stays with #39.
- **Vector paths are not deleted**. The inside of Form XObjects is not looked at, and image pixels are not changed.
- The filtered operators are rewritten with `write_content` — operators outside the area (including strings with unbalanced parentheses and inline images) read back the same (`test_redaction_keeps_content_outside_the_area_unchanged`, #29). The cover rectangle and colour are written as `Number` (#90) — the generator reals of [Object syntax roundtrip](../invariant/object-syntax-roundtrip.md).
- **The deleted bytes stay in the file**: `build` in [Document modifier](document-modifier.md) does not GC the old content stream object, so it remains in the output. [Incremental save](incremental-save.md) keeps the original by design (inferred: the deleted text is recoverable).
- It assembles page content with its own function — [Page content assembly](../invariant/page-content-assembly.md).

## Code
- `justpdf-core/src/annot/redact.rs` — `apply_redactions`, `filter_content_ops`, `get_page_content_data`, `RedactInfo`

## Reference behaviour
**None.**

## Cross-cutting invariants
- [Object syntax roundtrip](../invariant/object-syntax-roundtrip.md)
- [Page content assembly](../invariant/page-content-assembly.md)

## Blast radius
- [Content stream parsing](content-stream-parsing.md) — parsing and rewriting.
- [Document modifier](document-modifier.md) — whether it GCs on save decides whether the promise holds.
- [Annotations](annotations.md) — the Redact annotation input.
- [Text extraction](text-extraction.md) — the judge that would confirm the deletion (no test uses it today).

## Known holes / open
- `test_redaction_apply` only checks that the Redact annotation is gone, not that the text was deleted.
- On a page with no content, Redact annotations are not removed. A Redact without `/Rect` is silently dropped. `overlay_text` is `dead_code`.
- Tracked: #39 (redaction leftovers), #150 (a page index out of range is `AnnotationError`)
