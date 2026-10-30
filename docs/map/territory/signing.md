# Digital signing (CMS, ByteRange)

## What it is
`sign_pdf` leaves the original bytes untouched and appends an incremental section holding a signature dictionary with a `/Contents` placeholder, a signature widget and, for a visible signature, its appearance stream. It then takes the digest of the bytes outside the placeholder, builds a CMS SignedData (`/SubFilter /adbe.pkcs7.detached`, RSA PKCS#1 v1.5) over it by hand-written DER, writes it into the placeholder as hex, and fills in `/ByteRange`. Nothing in the product calls it: CLI `sign` is a stub and the facade has no signing call.

## Governing decisions
**None.** Refusing encrypted input is a maintainer decision (#26): `sign_pdf` takes no password, so there is no key to encrypt the appended objects with.

## Design model
From reading the source (`sign/sign_pdf.rs`), the tests named, and a probe run on 2026-10-30 (a one-page file signed with a fresh 2048-bit RSA key and a self-signed certificate, checked with `openssl cms -verify -binary -noverify`, OpenSSL 3.6.4).
- **Inputs**: a PKCS#8 DER RSA key (other key types are refused at parse), DER certificates with the signer first, `SigningOptions`. Encrypted input is refused with `UnsupportedEncryption` (`test_placeholder_rejects_encrypted_input`).
- **Placeholder**: `/Contents` is `PLACEHOLDER_SIZE` (16,384) bytes, written as 32,768 hex zeros; a CMS blob larger than that is an error. `contents_offset` is taken right after `<`, so the `<` and `>` delimiters are inside the signed ranges (measured: the bytes on either side of the ByteRange hole are `<` and `>`). ISO 32000 practice excludes the delimiters too (#37, inferred there).
- **Order of the steps** (`sign_pdf`): the digest is taken over the file while `/ByteRange` still reads `[0 0000000000 0000000000 0000000000]`; `fix_byte_range` writes the real values afterwards, inside the signed ranges. The signed digest therefore never matches the file (measured: `openssl cms -verify` fails over the final file's ranges and succeeds over the same ranges with the placeholder text put back; `verify_signature` with the `/Contents` trimmed to its DER gives `DigestMismatch` with `signature_valid = true`). `fix_byte_range` patches the first match of the placeholder text anywhere in the file.
- **Not wired into the document**: the field is not added to `/AcroForm /Fields`, the catalog is not updated, the widget is not added to any page's `/Annots` and has no `/P` (`build_pdf_with_placeholder`; the module comment no longer claims otherwise). [Signature detection](signature-detection.md) finds nothing in the output (measured). `/M` is not written and `SigningOptions::contact_info` is dropped.
- **Widget**: `/T (Signature1)` always. A visible one gets `/Rect` (default `[0 0 200 80]`), `/AP /N` and `/F 4`; an invisible one gets `/Rect [0 0 0 0]` and `/F 132`, which is Print + Locked, not Print + Hidden as the code comment says (annotation flag bits, ISO 32000-2 §12.5.3, from memory; not read raw in this run).
- **Object numbers**: the new objects are numbered from `object_count() + 11`, where `object_count` is the number of xref entries, not the highest object number; in a file with gaps in its numbering they can reuse an existing number, and the trailer `/Size` can come out smaller than the original's (inferred; `DocumentModifier::from_document` uses the highest number + 1 instead).
- **Values in the appended objects** are written with shared functions: `/Name`, `/Reason`, `/Location` with `string_syntax` (the UTF-8 bytes of a `&str`, hex when not printable ASCII), `/Rect` with `real_syntax` (NaN → `0.0`, ±∞ → ±`f32::MAX`), the appearance dictionary with `serialize_dict` (`test_placeholder_values_read_back_unchanged`, #29) — [Object syntax roundtrip](../invariant/object-syntax-roundtrip.md), [Text string encoding](../invariant/text-string-encoding.md).
- **Xref and trailer**: one subsection per new object with 20-byte entries (`test_placeholder_xref_entries_are_20_bytes`) — [Xref entry format](../invariant/xref-entry-format.md); the trailer from `incremental_trailer`, keeping the original trailer's keys (`test_placeholder_keeps_trailer_keys`) — [Incremental trailer](../invariant/incremental-trailer.md).
- **CMS**: signed attributes contentType, signingTime (UTCTime from the system clock, `build_utctime_now`) and messageDigest; the signer identified by issuer and serial; the whole certificate list in `certificates`; a caller's `SigningOptions::timestamp_token` added as an unsigned attribute ([Timestamps](timestamps.md)). DER lengths are written up to 3 bytes (`encode_der_length`).
- **Bounds on input**: the whole input file is parsed and copied; the CMS size is bounded by the placeholder; signer name and reason lengths are not bounded.

## Code
- `justpdf-core/src/sign/sign_pdf.rs` — `sign_pdf`, `build_pdf_with_placeholder`, `compute_digest`, `create_cms_signed_data`, `build_signer_info`, `build_utctime_now`, `build_algorithm_identifier`, `append_attribute`, `encode_der_length`, `hex_encode`, `fix_byte_range`, `PLACEHOLDER_SIZE`
- `justpdf-core/src/sign/byterange.rs` — `compute_byterange_digest`
- `justpdf-core/src/sign/types.rs` — `SigningOptions`, `DigestAlgorithm`

## Reference behaviour
- OpenSSL 3.6.4 `cms -verify` on the probe's output (measured 2026-10-30, above): the CMS structure parses and the signature verifies over the placeholder bytes, not over the file.
- No comparison with Acrobat, pyHanko or MuPDF's signer is recorded. Clauses to compare against: ISO 32000-2 §12.8.1 (`/ByteRange`, `/Contents`), §12.8.3.

## Cross-cutting invariants
- [Object syntax roundtrip](../invariant/object-syntax-roundtrip.md) — `string_syntax`, `real_syntax`, `serialize_dict`.
- [Text string encoding](../invariant/text-string-encoding.md) — `/Name`, `/Reason`, `/Location`.
- [Incremental trailer](../invariant/incremental-trailer.md) — a write-side site.
- [Xref entry format](../invariant/xref-entry-format.md) — the appended section's xref table.
- [Signature contents](../invariant/signature-contents.md)

## Blast radius
- [Incremental save](incremental-save.md) — appends a revision by its own code; the two pick object numbers and build trailers separately, so a defect found on one side is looked for on the other.
- [Xref](xref.md) — reads the appended section through `/Prev`; an entry or `/Size` it misreads hides the signature objects.
- [Signature appearance](signature-appearance.md) — draws the visible signature from the signer name and reason passed here.
- [Timestamps](timestamps.md) — the token goes in as an unsigned attribute; it must cover the signature value made here (#180).
- [Signature detection](signature-detection.md), [Signature verification](signature-verification.md) — read what this writes: the field placement decides whether detection finds it, the padding and `/ByteRange` decide whether verification can pass.
- [CLI](cli.md) — `sign` refuses with "not implemented yet (#181)" and exit code 1; wiring it would ship the defects above.

## Known holes / open
- Every signature is invalid: the digest is taken before `/ByteRange` is filled in (measured, above). Tracked: #408.; #37 tracks the related wiring and delimiter defects.
- The signature is not reachable from the catalog, so no reader finds it (measured). Tracked: #37.
- The `<`/`>` delimiters are inside the signed ranges (measured). Tracked: #37.
- New object numbers come from the entry count, not the highest number (inferred). Not tracked by an issue.
- There is no sign → detect → verify round-trip test.
- CLI `sign` does not sign. Tracked: #181 (keys and certificates), #308 (PKCS#12), #309 (`--timestamp-url`).
- Encrypted documents cannot be signed (above).
- Text strings are written as BOM-less UTF-8. Tracked: #33.
