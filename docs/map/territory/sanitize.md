# Sanitize (removing unpainted text and unseen content)

## What it is
A designed operation, not built, that turns a document into a **sanitized document** (GLOSSARY): it removes the categories of **unpainted text** and **unseen content** (GLOSSARY) the caller chooses, each category whole, independent of any redaction area. It is the counterpart [ADR 0005](../../adr/0005-sanitizing-removes-unpainted-text-by-category.md) gives [Redaction](redaction.md): text that no page paints has no position for a redaction area to decide, so it is removed by category instead. Its promise is that nothing of a chosen category can be recovered from the file, not that particular words are gone.

## Governing decisions
- [ADR 0005 — Sanitizing removes unpainted text by category](../../adr/0005-sanitizing-removes-unpainted-text-by-category.md): the categories, their defaults, the PDF/A and PDF/UA exception, the incremental-save refusal, and what is out of scope.
- [ADR 0004 — Redaction never under-erases](../../adr/0004-redaction-never-under-erases.md): its *Out of scope* hands unpainted text to sanitizing, and its shared interpreter is the one ADR 0005's unseen-content categories decide each glyph, path and image through.

## Design model
All from ADR 0005 and the agent briefs on #310 (2026-10-10) and #321; none of it is in code.
- **Categories**, each a separate option, all on by default except invisible text:
  - *Metadata*: the trailer `/Info`, every XMP `/Metadata` stream (catalog, page, object), `/PieceInfo`. A document declaring PDF/A (`pdfaid`) or PDF/UA (`pdfuaid`) keeps a catalog XMP packet holding only those declarations.
  - *Navigation*: `/Outlines`; named destinations, after every reference to one is rewritten to its explicit destination array; the `/P` prefixes of `/PageLabels` (`/S` and `/St` stay).
  - *Attachments*: the `EmbeddedFiles` name tree, `/AF` arrays, FileAttachment annotations.
  - *JavaScript*: the `JavaScript` name tree, and JavaScript actions in `/OpenAction`, `/AA` and annotation actions (the #310 brief adds `/Next` chains).
  - *External actions*: URI, Launch, GoToR, GoToE, SubmitForm and ImportData actions. A Link annotation that loses its action stays; GoTo actions within the document stay.
  - *Annotation text*: `/Contents`, `/T`, `/Subj`, `/RC` of every annotation, and Popup annotations; a form field's `/TU` and `/TM`.
- **Unseen content** (ADR 0005, *Unseen content*; #321) filters page content rather than dropping entries, each glyph, path and image decided through the interpreter redaction uses:
  - *Invisible text*: glyphs in text rendering mode 3, off by default (an OCR layer is this text). Mode 7 stays.
  - *Hidden optional content*: marked-content sections, Form XObjects and images whose optional content is off in the default configuration `/D`, the one text extraction judges by. The groups themselves stay.
  - *Content outside the crop box* (intersected with the media box): glyphs, paths, images and Form XObjects wholly outside; anything partly inside stays.
- **No incremental save** after sanitizing that removed something, as after applying redactions: the original revision still holds what was removed (ADR 0005; the #310 brief reuses #213's mechanism).
- **Removed objects must not be in a full save**: `DocumentModifier::build` does not garbage-collect (#39), so the #310 brief requires sanitize to garbage-collect or overwrite what it unlinks.
- **Out of scope** (ADR 0005): form field values, field names and `/Opt` (redaction's, through appearances); the structure tree's `/Alt`, `/ActualText`, `/E`, `/T`; content in the background colour or covered by later content.
- **Considered and rejected** (ADR 0005): redaction clearing the strings that contain erased text (a missed match leaves the text silently); redaction clearing every category whenever it applies; dropping a PDF/A or PDF/UA document's XMP whole; invisible text on by default; optional content hidden only when off in every configuration; clipping to the media box.
- **Bounds on input** (designed, not built): every category walks document structures whose size comes from the file — name trees, annotation arrays, `/Next` chains, page content. The walkers it would reuse carry the visit budget for name trees ([Tree traversal cycles](../invariant/tree-traversal-cycles.md)) and none for `/Next` chains ([Actions](actions.md)).

## Code
**None.** `git grep -i sanitiz` over the workspace's Rust sources finds nothing at 88b2a08; no `sanitize`, `SanitizeOptions` or `SanitizeReport` exists, and `incremental_save` has no refusal for a redacted or sanitized modifier.

What does not exist:
- the operation, its options and its report (the #310 brief: `sanitize(modifier, doc, &SanitizeOptions) -> Result<SanitizeReport>` in `justpdf-core`, shaped like `apply_redactions`);
- a way to drop the trailer `/Info`: `DocumentModifier` holds `info_num` and only `set_info` changes it;
- PDF/A and PDF/UA detection from XMP (`git grep pdfaid` finds nothing; #58 needs the same);
- the modifier flag that makes `incremental_save` refuse (#213);
- a way for the interpreter to report path and image boxes and the crop box test (#321).

Where a first slice would start, on existing symbols:
- `justpdf-core/src/annot/redact.rs — apply_redactions`: the `(modifier, doc)` shape the operation copies.
- `justpdf-core/src/outline/builder.rs — remove_outlines`; `justpdf-core/src/outline/parse.rs — read_named_destinations` (names to rewrite into explicit arrays).
- `justpdf-core/src/page_label.rs — read_page_labels`, `set_page_labels` (rewrite without `/P`).
- `justpdf-core/src/writer/compress.rs — strip_non_essential` (private): already removes the catalog XMP, `/PieceInfo`, the `EmbeddedFiles` and `JavaScript` name trees and page `/AA` for compression's `strip_metadata`/`strip_extras`, without the PDF/A exception.
- `justpdf-core/src/writer/modify.rs — DocumentModifier::garbage_collect`, `DocumentModifier::set_info`, `incremental_save`.
- For unseen content: `justpdf-core/src/content/interpret.rs — interpret`, `ContentVisitor`, `Glyph` (its `quad` and `hidden`), `TextState::render_mode`; `justpdf-core/src/ocg/parse.rs — is_ocg_visible`, `is_ocmd_visible`. The interpreter already reads `BDC /OC` and reports `Glyph::hidden` under the default configuration (a step past what #321's brief describes); it does not track clipping.

## Reference behaviour
- Acrobat separates "Redact" from "Sanitize document" (removing metadata, bookmarks, attachments, hidden text and hidden layers) — from memory, not checked (#310, #321).
- MuPDF's redaction filter (`source/pdf/pdf-op-filter.c`, read raw 2026-10-08 for #215) handles marked content and structure elements only; no catalog-level cleanup was seen there (not searched beyond that file) (#310).

## Cross-cutting invariants
- [Tree traversal cycles](../invariant/tree-traversal-cycles.md) — the navigation and attachments categories walk name trees.
- [Hidden content state](../invariant/hidden-content-state.md) — removing hidden optional content must keep the state changes inside it (designed, #321).

## Blast radius
- [Redaction](redaction.md) — ADR 0005 draws the line between the two by position; both refuse incremental save through one mechanism (#213), and the unseen-content categories filter page content through the interpreter redaction will use (#209).
- [Content interpreter](content-interpreter.md) — glyph boxes, render mode and optional-content visibility for unseen content come from it; a change there changes what sanitize removes.
- [Compress stripping](compress-stripping.md) — removes part of the same categories for file size, without the PDF/A exception (#58); the two must not disagree on what a category holds.
- [Outlines](outlines.md), [Page labels](page-labels.md), [Embedded files](embedded-files.md), [Actions](actions.md), [Annotations](annotations.md) — each holds a category's data; the forms those modules read are the forms sanitize must find.
- [Document modifier](document-modifier.md) — `build` writes unlinked objects (#39), so removal depends on sanitize garbage-collecting; dropping `/Info` needs a new modifier operation.
- [Incremental save](incremental-save.md) — must refuse a sanitized modifier (#213).
- [Optional content](optional-content.md) — the default-configuration visibility sanitize judges by is the one shared with text extraction and the renderer.

## Known holes / open
- The operation is not built. Tracked: #310 (unpainted text), #321 (unseen content in the page)
- #310 is blocked by #213 (the no-incremental-save flag); #321 by #310 and #209 (per-glyph filtering through the shared interpreter).
- PDF/A and PDF/UA detection is wanted by both sanitize and compression. Tracked: #58
