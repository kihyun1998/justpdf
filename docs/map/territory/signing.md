# Digital signing (CMS, ByteRange)

## What it is
Leaves the original bytes untouched and appends an incremental section holding the signature dictionary and widget, leaves a `/Contents` placeholder to compute the ByteRange, then builds a CMS SignedData (`adbe.pkcs7.detached`) from the digest of that range and fills it into the placeholder as hex. The DER is built by hand.

## Governing decisions
**None.**

## Design model
- The signing key is accepted only as RSA PKCS#8. The `/Contents` placeholder is `PLACEHOLDER_SIZE` bytes.
- `contents_offset` is taken right after `<`, so the `<` and `>` delimiters fall inside the signed range (inferred: differs from the practice of excluding the delimiters as well).
- `fix_byte_range` patches the first match of the placeholder text in the whole file.
- **The incremental section is not wired into the document**: the field is not added to `/AcroForm /Fields`, the Catalog is not updated, and the widget is not added to the page's `/Annots`. "3. Updated AcroForm 4. Updated Catalog" in the module comment is not implemented. `/M` is not written and `contact_info` is dropped.
- The new trailer is built with `incremental_trailer` — [Incremental trailer](../invariant/incremental-trailer.md). Encrypted input is rejected with `UnsupportedEncryption` (it takes no password, so there is no key to encrypt the appended objects with — #26 maintainer decision).
- The signature dictionary and widget are assembled by hand with `write!`, but values are written with shared functions: `/Name`, `/Reason` and `/Location` with `string_syntax` (the UTF-8 bytes of a Rust `&str` — hex if non-ASCII), `/Rect` with `real_syntax` (NaN → `0.0`, ±Inf → ±`f32::MAX`), and the appearance stream dictionary with `serialize_dict`. They read back the same (`test_placeholder_values_read_back_unchanged`, #29) — [Object syntax roundtrip](../invariant/object-syntax-roundtrip.md), [Text string encoding](../invariant/text-string-encoding.md).

## Code
- `justpdf-core/src/sign/sign_pdf.rs` — `sign_pdf`, `build_pdf_with_placeholder`, `create_cms_signed_data`, `build_signer_info`, `build_utctime_now`, `fix_byte_range`, `PLACEHOLDER_SIZE`
- `justpdf-core/src/sign/byterange.rs` — `compute_byterange_digest`, `detect_modification_after_signing`

## Reference behaviour
**None.** Clauses to compare against: ISO 32000-2 §12.8.1 (ByteRange, Contents), §12.8.3. No record of checking with a third-party validator (Acrobat or others).

## Cross-cutting invariants
- [Object syntax roundtrip](../invariant/object-syntax-roundtrip.md) — `string_syntax`, `real_syntax`, `serialize_dict`.
- [Text string encoding](../invariant/text-string-encoding.md) — `/Name`, `/Reason`, `/Location`.
- [Incremental trailer](../invariant/incremental-trailer.md) — a write-side site.
- [Xref entry format](../invariant/xref-entry-format.md) — the xref table of the appended section.

## Blast radius
- [Incremental save](incremental-save.md) — another implementation of the same approach. A defect found on one side is looked for on the other.
- [Xref](xref.md) — the side that reads the appended section.
- [Signature appearance](signature-appearance.md) — called by `build_pdf_with_placeholder`.
- [Timestamps](timestamps.md) — `build_signer_info` puts the token in as an unsigned attribute.
- [Signature detection](signature-detection.md), [Signature verification](signature-verification.md) — the sides that read the result.
- [CLI](cli.md) — the `sign` subcommand does not call this module (a stub).

## Known holes / open
- There is no sign → verify round-trip test.
- CLI `sign` fails with exit code 1 without signing (measured). The CLI takes `--cert`, but core has no PKCS#12 parser — #181.
- Encrypted documents cannot be signed (above).
- Tracked: #33 (text string encoding), #37 (signature wiring and validity), #180 (timestamp ordering), #181 (CLI sign)
