# justpdf

A pure-Rust PDF engine: parsing, writing, rendering, signing and conversion of PDF documents.

## Language

### Signature verification

**Integrity check**:
Whether the bytes covered by a signature's `/ByteRange` hash to the digest the signer signed.
_Avoid_: digest valid, hash check

**Signature check**:
Whether the signature value is a correct cryptographic signature by the signer certificate's key over the signed data.
_Avoid_: crypto valid, signature valid

**Trust check**:
Whether the signer certificate chains, by verified signatures, to a trust anchor.
_Avoid_: chain valid, certificate valid

**Trust anchor**:
A certificate the caller declares trusted; the end point a trust check must reach.
_Avoid_: root store, trusted root

**Unsupported**:
The outcome of a check that could not be carried out because the algorithm or structure is not implemented — distinct from a check that ran and failed.
_Avoid_: invalid, unknown

**Coverage**:
Whether a signature's `/ByteRange` reaches the end of the file, or later revisions were appended after it; a fact reported beside the verdict, not part of it.
_Avoid_: modified after signing, tampered

**Verdict**:
The single overall judgement of one signature, derived from its checks: valid, valid but untrusted, indeterminate, or invalid.
_Avoid_: validity, status

### Redaction

**Redaction area**:
The region of a page a Redact annotation marks for removal, in default user space.
_Avoid_: redact rect, redaction box

**Applied redaction**:
A page from which nothing that would paint inside a redaction area can be recovered from the file; removing content outside the area as well is acceptable, leaving any inside it is not.
_Avoid_: redacted, blacked out

**Unpainted text**:
Text a document carries that no page paints: metadata, bookmarks, destination names, page label prefixes, attachments, scripts, link and action targets, annotation text and form field tooltips.
_Avoid_: hidden text, document-level text

**Sanitized document**:
A document from which the chosen categories of unpainted text have been removed whole, independent of any redaction area.
_Avoid_: scrubbed, cleaned, stripped
