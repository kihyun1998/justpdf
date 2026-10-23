# Sanitizing removes unpainted text by category

Applying a redaction leaves the text a document carries but no page paints — metadata, bookmarks, attachments and the rest — because none of it has a position for a redaction area to decide. A separate **sanitize** operation removes it: the caller chooses categories, and each chosen category is removed whole, whatever text it holds. A sanitized document promises that nothing of a chosen category can be recovered from the file, not that some particular words are gone.

## Categories

Each is a separate option, all on by default except invisible text.

- **Metadata**: the trailer `/Info`, every XMP `/Metadata` stream (catalog, page and object), `/PieceInfo`. A document declaring PDF/A (`pdfaid`) or PDF/UA (`pdfuaid`) keeps a catalog XMP packet holding those declarations and nothing else.
- **Navigation**: `/Outlines`; named destinations, after every reference to one is rewritten to its explicit destination array; the `/P` prefixes of `/PageLabels` (the numbering style `/S` and start `/St` stay).
- **Attachments**: the `EmbeddedFiles` name tree, `/AF` arrays, FileAttachment annotations.
- **JavaScript**: the `JavaScript` name tree, and JavaScript actions in `/OpenAction`, `/AA` and annotation actions.
- **External actions**: URI, Launch, GoToR, GoToE, SubmitForm and ImportData actions. A Link annotation that loses its action stays; GoTo actions within the document stay.
- **Annotation text**: `/Contents`, `/T`, `/Subj` and `/RC` of every annotation, and Popup annotations; a form field's `/TU` and `/TM`.

### Unseen content

Unseen content is drawn by a page but not shown, so removing it filters the page's content rather than dropping dictionary entries, and each glyph, path and image is decided through the interpreter redaction uses (ADR 0004).

- **Invisible text**: glyphs shown with text rendering mode 3. Off by default: an OCR layer over a scanned page is this text, and removing it leaves the document unsearchable. Mode 7 (clip only) stays, because removing it changes the clipping path and with it what is shown.
- **Hidden optional content**: content in a marked-content section, and Form XObjects and images, whose optional content is off in the default configuration (`/D`), the configuration text extraction also judges visibility by. The optional content groups themselves stay.
- **Content outside the crop box**: glyphs, paths, images and Form XObjects whose box lies wholly outside the crop box (intersected with the media box). Anything partly inside stays.

A document whose modifier sanitized is refused incremental save, as after applying redactions: the original revision still holds what was removed.

## Considered options

- **Redaction clears the strings that contain erased text.** Rejected: a match can miss — a different encoding, a word split by hyphenation, XMP escaping, a compressed attachment — and a miss silently leaves the text, which breaks the never-under-erase promise of [ADR 0004](0004-redaction-never-under-erases.md).
- **Redaction clears every category whenever it applies.** Rejected: one Redact annotation would take the bookmarks and attachments with it, and a caller who wants only the page erased could not have it.
- **Removing the XMP of a PDF/A or PDF/UA document whole.** Rejected: the conformance declarations carry no text of the user's, and dropping them breaks the conformance.
- **Invisible text on by default.** Rejected: most invisible text is an OCR layer, and a caller sanitizing a scanned document rarely means to make it unsearchable.
- **Optional content counts as hidden only when it is off in every configuration.** Rejected: a viewer shows the default configuration, and judging visibility differently from text extraction would let the two disagree about the same content.
- **Clipping to the media box.** Rejected: a viewer shows the crop box, so content between the two is not seen.

## Out of scope

- Form field values, field names and `/Opt`: they are painted through appearance streams, so they belong to redaction.
- The structure tree's `/Alt`, `/ActualText`, `/E` and `/T`: removing them for the whole document breaks accessibility; redaction clears them where it erased content.
- Content painted in the background colour, or covered by later content: deciding it needs rendering.
