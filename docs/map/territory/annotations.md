# Annotations (model, reading, creation)

## What it is
Parses a page's `/Annots` into per-type models, and builds new annotation dictionaries with a builder to add to and delete from pages. Every addition generates an appearance stream and attaches it to `/AP /N`.

## Governing decisions
**None.**

## Design model
- Link annotations read `/A /URI` directly and keep `/Dest` as a raw `PdfObject` — they do not use the [Actions](actions.md) parser.
- Text values: reading uses `from_utf8_lossy`, writing uses UTF-8 bytes — [Text string encoding](../invariant/text-string-encoding.md).
- `add_annotation`: if the page's `/Annots` is an indirect reference, it drops the existing annotations and replaces them with a new array (measured: 1 existing + 1 added → 1). `delete_annotation` on such a page reports that the page has no annotations (inferred).

## Code
- `justpdf-core/src/annot/types.rs` — `AnnotationType`, `AnnotationData`, `AnnotationFlags`, `AnnotColor`, `BorderStyle`
- `justpdf-core/src/annot/parse.rs` — `get_annotations`, `get_all_annotations`, `parse_annotation_dict`
- `justpdf-core/src/annot/builder.rs` — `AnnotationBuilder`, `build_dict`, `add_annotation`, `delete_annotation`

## Reference behaviour
**None.** Clause to compare against: ISO 32000-2 §12.5.

## Cross-cutting invariants
- [Text string encoding](../invariant/text-string-encoding.md)

## Blast radius
- [Annotation appearance](annotation-appearance.md) — always called on addition.
- [Render annotations](render-annotations.md) — the side that draws the appearance.
- [Redaction](redaction.md) — applies Redact annotations.
- [Document modifier](document-modifier.md) — the save path for additions and deletions.
- [Facade](facade.md) — `annotations` (read-only).

## Known holes / open
- The builder has no Caret creation and no Popup linking (Popup only parses `popup_ref`).
- Tracked: #33 (text string encoding), #41 (annotation appearance coordinates, indirect `/Annots`)
