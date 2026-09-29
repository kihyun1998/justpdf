# Incremental trailer

## The fact
The trailer of a section appended by an incremental update **must restate the document-level keys of the original trailer (`/Root`, `/Info`, `/Encrypt`, `/ID` and so on)**. justpdf's xref reader keeps only the newest trailer and does not merge keys from earlier trailers, so a key missing from the new trailer disappears on the read side. pdf.js and MuPDF also read only the newest trailer — if the write side does not carry a key over, it disappears in other readers too.

Both write sites build the new trailer with one function, `incremental_trailer` (`writer/modify.rs`): it carries over every key of the previous trailer, drops the xref-stream-only keys (`/Type /W /Index /Filter /DecodeParms /Length /F /FFilter /FDecodeParms /DL`) and `/XRefStm`, then writes `/Size` (at least the previous value), `/Root`, `/Info` when needed, and `/Prev`.

**Maintainer decision (2026-09-23, #26)**: fix only the write side. The reader does not merge. Alternatives offered: reader merging (reads our own files, but they still break in pdf.js and MuPDF, and write omissions get hidden), or both. Facts offered as grounds: pdf.js `xref.js` (`topDict ||= dict`) and MuPDF `pdf_trailer` (newest section) both do not merge. The ISO 32000-2 §7.5.6 wording has not been checked against the spec text.

## Why it is cross-cutting
Two pieces of code write an incremental section (plain incremental save, signing), and neither calls the other. Both write only the minimum keys. And [Xref](../territory/xref.md), which reads the result, is yet another module. Two write sites and one read site share the same assumption without calling each other.

## Territories it holds in
- [Xref](../territory/xref.md) — `load_xref_at`: keeps only the newest trailer ("First trailer wins for main keys", no merging). This is intended (decision above).
- [Incremental save](../territory/incremental-save.md) — writes with `incremental_trailer`.
- [Signing](../territory/signing.md) — writes with `incremental_trailer`.

## What a violation looks like
- (Before #26) Authenticating an encrypted PDF and running `incremental_save` produced a file that opened with `is_encrypted=false` and had no `/Encrypt` in its trailer. The appended objects were written as decrypted plaintext (reproduced 2026-09-23). The signing side had the same shape (from the code).
- `/Info` disappears after signing (signing side).
- Another reader (one that follows earlier trailers) and justpdf read the same file differently.

## Discovery history
No recorded incident. On 2026-09-23, while the map was being written, the read-side research agent reported it from the code, and a throwaway probe reproduced the `incremental_save` side the same day (above). `test_incremental_update` only checks that the newest trailer's `/Info` wins.


## Where it will recur
**Any function that writes a trailer with `/Prev` is subject to this invariant.** Build the new trailer with `incremental_trailer`. Do not write `<< /Size … /Prev … >>` by hand. If the document is encrypted, appended objects must also be encrypted with the original file key (`incremental_save` does this; `sign_pdf` has no key and rejects encrypted input).
