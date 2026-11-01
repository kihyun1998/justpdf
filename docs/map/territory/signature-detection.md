# Signature detection

## What it is
Walks the `/AcroForm /Fields` tree, collects every field that is `/FT /Sig` and has a `/V`, and returns a `SignatureInfo` per signature: field name, signer name, signing time, reason, location, contact info, `/Filter`, `/SubFilter`, `/ByteRange`, the raw `/Contents` bytes, and the certificates parsed from them (`detect_signatures`). It is what the facade's `Document::signatures` returns, and the input of [Signature verification](signature-verification.md).

## Governing decisions
**None.** The glossary's verification terms (`GLOSSARY.md`, "Signature verification") describe what is done with the result, not how it is found.

## Design model
From reading the source (`sign/detect.rs`, `sign/cert.rs`) and the tests named.
- **Walk** (`collect_sig_fields`): only reference entries of a direct `/Fields` array and of `/Kids` are followed. A field with `/Kids` is recursed into and never checked itself, so a `/FT /Sig` field whose only kids are its widgets is not found, and `/FT` is not inherited by kids (inferred). The field name is the parent's name, `.`, and `/T`.
- **Cycles and budget**: a field listing its own ancestor as a kid is `CircularReference`; going over the visit budget through shared fields is `LimitExceeded` — [Tree traversal cycles](../invariant/tree-traversal-cycles.md). Pinned by `tests/tree_cycles.rs` (`detect_signatures_rejects_field_tree_cycle`, `detect_signatures_stops_at_visit_limit`).
- **Text**: `/T`, `/Name`, `/M`, `/Reason`, `/Location` and `/ContactInfo` are decoded with `from_utf8_lossy`, so PDFDocEncoding and UTF-16BE (with BOM) text is garbled — [Text string encoding](../invariant/text-string-encoding.md). `/M` is kept as the raw date string.
- **Certificates**: `cert::extract_certificates` reads `/Contents` through `cert::decode_contents`, which decodes only the first DER element, so the zero padding after the CMS `ContentInfo` is ignored ([Signature contents](../invariant/signature-contents.md), #409); it returns no certificates on any error. Before #409 a strict whole-buffer decode rejected the padding and every padded signature, justpdf's included, gave an empty `cert_chain`; `certificates_are_read_from_a_zero_padded_contents` failed with the strict decoder restored (*measured* 2026-11-01).
- **Encryption**: in an encrypted document `/Contents` goes through `resolve` and so is decrypted like any string; `crypto::decrypt_object` has no exemption for a signature dictionary's `/Contents` (inferred from `crypto/decrypt.rs`). ISO 32000-2 §7.6 exempts a signature dictionary's `/Contents` from encryption (from memory; not read raw in this run).
- **Bounds on input**: the number of fields visited is bounded by the visit budget; depth is not bounded (#122). `/V` of each signature field is cloned whole and `/Contents` copied, outside the budget, so fields sharing one large `/V` cost its size each (#135).

## Code
- `justpdf-core/src/sign/detect.rs` — `detect_signatures`, `collect_sig_fields`
- `justpdf-core/src/sign/cert.rs` — `extract_certificates`, `parse_x509_cert`, `format_rdn_sequence`
- `justpdf-core/src/sign/types.rs` — `SignatureInfo`, `CertificateInfo`
- `justpdf/src/lib.rs` — `Document::signatures`

## Reference behaviour
**None.** Clause to compare against: ISO 32000-2 §12.8 (digital signatures), §12.7 (interactive forms).

## Cross-cutting invariants
- [Text string encoding](../invariant/text-string-encoding.md) — every text entry read lossily.
- [Tree traversal cycles](../invariant/tree-traversal-cycles.md) — `collect_sig_fields`.
- [Signature contents](../invariant/signature-contents.md)

## Blast radius
- [AcroForm](acroform.md) — walks the same field tree with its own code and its own field-versus-widget rule; a rule fixed there (as #204 did) is not applied here.
- [Signing](signing.md) — makes the signatures this should find; today it writes no `/AcroForm` entry, so this finds none (below), and its zero-padded `/Contents` is read through `decode_contents` (#409).
- [Signature verification](signature-verification.md) — takes `SignatureInfo` as given; `byte_range` and `contents_raw` are copied from the file unchecked, and `cert_chain` feeds the trust check.
- [Object decryption](object-decryption.md) — decides whether `/Contents` reaches here intact in an encrypted file.
- [Facade](facade.md) — `Document::signatures` returns `SignatureInfo`, so a field change is a public API change.

## Known holes / open
- A file justpdf has just signed has no detectable signature (measured 2026-10-30, probe: `sign_pdf` on a one-page file, then `detect_signatures` returns 0). Tracked: #37.
- A signature field whose widgets are its kids is not found (inferred). Not tracked by an issue.
- `/Contents` of an encrypted document is decrypted though the standard exempts it (inferred). Not tracked by an issue.
- No depth limit: an extreme depth without a cycle can overflow the stack (inferred). Tracked: #122.
- Cost outside the visit budget for a shared `/V`. Tracked: #135.
- Text strings read as lossy UTF-8. Tracked: #33.
