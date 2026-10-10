# Redaction never under-erases

Applying a redaction promises that **nothing that would paint inside a redaction area can be recovered from the file**. When a rule cannot tell whether content paints inside the area, the content is removed — erasing some content outside the area is accepted, leaving any inside it is not. This is a security promise, not a visual one: the overlay rectangle hides, the filter removes.

## Rules that follow

- **Text** is erased per glyph. A glyph is removed when its box — advance width by the font's `/FontBBox` height (−0.25…1.0 × size without one), through the text rendering matrix and the CTM — overlaps the area; the glyphs after it keep their positions through `TJ` adjustments. Invisible text (`Tr 3`) is erased the same way. `/ActualText` and `/Alt` on marked content enclosing an erased glyph are removed.
- **Vector paths** whose painted box (stroke width included) overlaps the area are removed — except a single filled rectangle that covers the whole area, which carries nothing about the area but its colour. A shading (`sh`) follows the path rule using its clip box. Clip-only paths (`W n`) paint nothing and stay.
- **Images** have the pixels overlapping the area overwritten (boundary pixels included) with the Redact's `/IC` colour (black without one) in the image's colour space; soft masks, `/Mask` and stencil masks are painted opaque over the same pixels. The result is a new image object stored with `FlateDecode`; an image used elsewhere keeps its original object. An image whose filter cannot be decoded is removed whole.
- **Form XObjects** are entered: each `Do` whose form overlaps the area is redirected to a copy filtered by these same rules under the form's `/Matrix` and resources. Past the depth limit or on a cycle, the `Do` is removed.
- **Optional content** is judged by geometry, never by visibility: a hidden layer is erased like a visible one.
- **Other annotations** whose `/Rect` overlaps the area are removed; a widget is also detached from the AcroForm field tree, and a field left without widgets is removed with its value. A redacted page's `/Thumb` is removed.
- **Incremental save** of a document with applied redactions is refused: it appends to the original revision, which still holds what was erased.

## One interpreter for extraction and redaction

Glyph positions come from the same graphics- and text-state interpreter that text extraction uses, moved out of text extraction into a shared module in `justpdf-core`. A glyph that extraction reports inside an area is then, by construction, one redaction erases — the property the tests assert. `justpdf-render`'s interpreter is out of reach (render depends on core), and a redaction-only interpreter would be a fourth operator walker whose positions could drift from extraction's.

## Considered options

- **Exactly the area** (MuPDF's model: split glyphs, clip paths, keep partially covered line art). Rejected as the default: clipping Béziers, stroke joins and dashes is costly, and every approximation in it errs toward leaving content.
- **A documented approximation** (the pre-ADR filter: whole BT…ET blocks by start point, one `cm` for images, paths untouched). Rejected: it under-erases, which breaks the promise.
- **Removing a whole image or form on overlap.** Rejected: a scanned page is one image, so the redaction would erase the page.
- **Documenting incremental save instead of refusing it.** Rejected: the promise would hold only for callers who read the documentation.

## Out of scope

The structure tree's `/Alt` and `/ActualText` (document-level), and drawing `OverlayText`.

Text the document carries but no page paints — metadata, bookmarks, destination names, attachments, scripts, action targets, annotation text — has no position for a redaction area to decide. Sanitizing removes it by category: [ADR 0005](0005-sanitizing-removes-unpainted-text-by-category.md).
