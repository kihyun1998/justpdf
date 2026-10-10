# Sanitizing removes unpainted text by category

Applying a redaction leaves the text a document carries but no page paints — metadata, bookmarks, attachments and the rest — because none of it has a position for a redaction area to decide. A separate **sanitize** operation removes it: the caller chooses categories, and each chosen category is removed whole, whatever text it holds. A sanitized document promises that nothing of a chosen category can be recovered from the file, not that some particular words are gone.

## Categories

Each is a separate option, all on by default.

- **Metadata**: the trailer `/Info`, every XMP `/Metadata` stream (catalog, page and object), `/PieceInfo`. A document declaring PDF/A (`pdfaid`) or PDF/UA (`pdfuaid`) keeps a catalog XMP packet holding those declarations and nothing else.
- **Navigation**: `/Outlines`; named destinations, after every reference to one is rewritten to its explicit destination array; the `/P` prefixes of `/PageLabels` (the numbering style `/S` and start `/St` stay).
- **Attachments**: the `EmbeddedFiles` name tree, `/AF` arrays, FileAttachment annotations.
- **JavaScript**: the `JavaScript` name tree, and JavaScript actions in `/OpenAction`, `/AA` and annotation actions.
- **External actions**: URI, Launch, GoToR, GoToE, SubmitForm and ImportData actions. A Link annotation that loses its action stays; GoTo actions within the document stay.
- **Annotation text**: `/Contents`, `/T`, `/Subj` and `/RC` of every annotation, and Popup annotations; a form field's `/TU` and `/TM`.

A document whose modifier sanitized is refused incremental save, as after applying redactions: the original revision still holds what was removed.

## Considered options

- **Redaction clears the strings that contain erased text.** Rejected: a match can miss — a different encoding, a word split by hyphenation, XMP escaping, a compressed attachment — and a miss silently leaves the text, which breaks the never-under-erase promise of [ADR 0004](0004-redaction-never-under-erases.md).
- **Redaction clears every category whenever it applies.** Rejected: one Redact annotation would take the bookmarks and attachments with it, and a caller who wants only the page erased could not have it.
- **Removing the XMP of a PDF/A or PDF/UA document whole.** Rejected: the conformance declarations carry no text of the user's, and dropping them breaks the conformance.

## Out of scope

- Form field values, field names and `/Opt`: they are painted through appearance streams, so they belong to redaction.
- The structure tree's `/Alt`, `/ActualText`, `/E` and `/T`: removing them for the whole document breaks accessibility; redaction clears them where it erased content.
- Content that is in the page but not visible — invisible text (`Tr 3`), hidden optional content, content outside the crop box.
