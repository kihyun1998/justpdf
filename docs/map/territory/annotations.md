# Annotations (model, reading, creation)

## What it is
Parses a page's `/Annots` into per-type models (`Annotation`, `AnnotationData`), and builds new annotation dictionaries with `AnnotationBuilder` to add to and delete from a page. An addition asks [Annotation appearance](annotation-appearance.md) for an appearance stream and, when one is made, attaches it as `/AP /N`; types without a generator (Link, FreeText, Polygon, PolyLine, FileAttachment and the rest) are added without `/AP` (inferred, `generate_appearance` returns `None` for them).

## Governing decisions
**None.** [ADR 0005](../../adr/0005-sanitizing-removes-unpainted-text-by-category.md) names annotation text (`/Contents`, `/T`, `/Subj`, `/RC`, Popup and FileAttachment annotations) as a category [Sanitize](sanitize.md) removes; it decides that operation, not this model. [ADR 0004](../../adr/0004-redaction-never-under-erases.md) decides that redaction removes other annotations overlapping the area, which [Redaction](redaction.md) does not do yet.

## Design model
From reading the source; no record governs it.
- **Reading** (`get_annotations`): `/Annots` direct or indirect; each entry a reference to a dictionary or a direct dictionary, others skipped. An entry without `/Subtype` or a usable `/Rect` is skipped (`parse_annotation_dict` returns `None`). `/Rect` is normalised by `Rect::from_pdf_array` (#345). `appearance_ref` is `/AP /N` only when it is a reference, so a state dictionary or a direct `/N` reads as `None` (inferred). `popup_ref` is `/Popup` as a reference; nothing follows it.
- **Link** annotations read `/A /URI` directly and keep `/Dest` as a raw `PdfObject` — they do not use the [Actions](actions.md) parser (`parse_annotation_data`, Link arm).
- **Text values**: reading uses `String::from_utf8_lossy`, writing puts the Rust string's UTF-8 bytes (`build_dict`: `/Contents`, `/OverlayText`, `/DA`, `/URI`) — [Text string encoding](../invariant/text-string-encoding.md).
- **`add_annotation` on a page whose `/Annots` is an indirect reference** replaces it with a new direct array holding only the new annotation, so the existing ones are dropped from the page (measured 2026-10-30 at 88b2a08, scratch probe: 2 annotations, add one, `get_annotations` returns 1). **`delete_annotation`** on such a page fails with `AnnotationError` "page has no annotations" (measured, same probe). Both take `/Annots` from the page dictionary in the modifier and handle only a direct array.
- **Bounds on input**: the number of annotations and the length of `/QuadPoints`, `/InkList`, `/Vertices` are bounded only by the file's size; there is no recursion and no visit budget (inferred).

## Code
- `justpdf-core/src/annot/types.rs` — `AnnotationType`, `Annotation`, `AnnotationData`, `AnnotationFlags`, `AnnotColor`, `BorderStyle`, `BorderStyleType`, `LineEndingStyle`
- `justpdf-core/src/annot/parse.rs` — `get_annotations`, `get_all_annotations`, `parse_annotation_dict`, `parse_border_style`, `parse_annotation_data`
- `justpdf-core/src/annot/builder.rs` — `AnnotationBuilder`, `AnnotationBuilder::build_dict`, `add_annotation`, `delete_annotation`

## Reference behaviour
**None.** Clause to compare against: ISO 32000-2 §12.5.

## Cross-cutting invariants
- [Text string encoding](../invariant/text-string-encoding.md)

## Blast radius
- [Annotation appearance](annotation-appearance.md) — the dictionary `build_dict` writes (`/Rect`, `/C`, `/IC`, `/BS /W`, `/QuadPoints`, `/L`, `/InkList`, `/Name`) is the generator's only input; a key renamed or moved here draws nothing or the default there.
- [Render annotations](render-annotations.md) — draws what `add_annotation` attaches, through `/AP /N` and `/Rect`; a change to which annotations get `/AP` changes what is drawn.
- [Redaction](redaction.md) — reads Redact annotations (`/Rect`, `/IC`, `/OverlayText`) from `/Annots` with its own loop; the keys `AnnotationBuilder::redact` writes are the ones it reads.
- [Sanitize](sanitize.md) — ADR 0005's annotation-text and attachment categories remove keys and annotation types this model reads and writes (not built: #310).
- [Document modifier](document-modifier.md) — additions and deletions are written through `set_object` on the page in the modifier; the indirect-`/Annots` loss above sits on that boundary.
- [Facade](facade.md) — `Document::annotations` returns this module's `Annotation` type, so a field change is a public API change.

## Known holes / open
- The builder has no Caret creation and no Popup linking (Popup only parses `popup_ref`).
- `add_annotation` drops the annotations behind an indirect `/Annots`, and `delete_annotation` cannot delete from it (above). Tracked: #41
- Text strings are written as BOM-less UTF-8 and read as lossy UTF-8. Tracked: #33
