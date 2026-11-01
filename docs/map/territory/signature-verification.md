# Signature verification

## What it is
For a signature [Signature detection](signature-detection.md) found, `verify_signature` checks the digest of the `/ByteRange` bytes against the CMS messageDigest (the glossary's **integrity check**), the signature value against the signer certificate's key (**signature check**), and the certificate list's issuer/subject linkage (standing in for the **trust check**), and folds them into one `SignatureValidity`. `verify_all_signatures` runs it over every detected signature. Nothing in the product calls either: the facade, CLI and bindings expose detection only.

## Governing decisions
**None.** `GLOSSARY.md` ("Signature verification") defines the target vocabulary — integrity check, signature check, trust check, trust anchor, unsupported, coverage, verdict — and its `_Avoid_` lines name today's code terms (`digest_valid`, `signature_valid`, `chain valid`, `modified after signing`, `validity`). The code does not implement that model yet (#38).

## Design model
From reading the source (`sign/verify.rs`, `sign/cert.rs`, `sign/byterange.rs`) and a probe run on 2026-10-30 against a file signed by [Signing](signing.md).
- **CMS parsing** goes through `cert::decode_contents`, shared with certificate extraction: only the first DER element of `/Contents` is decoded, so the zero padding signers write after it is ignored ([Signature contents](../invariant/signature-contents.md), #409). Before #409 `ContentInfo::from_der` rejected the padding and every padded signature was `Unknown("CMS parse error: trailing data …")` (measured 2026-10-30). Now a `sign_pdf` signature verifies with `digest_valid` and `signature_valid` from its padded `/Contents` (`tests/sign_roundtrip.rs`, *measured* 2026-11-01).
- **Only the first SignerInfo** is checked.
- **Integrity check**: the digest algorithm comes from the SignerInfo; an OID other than SHA-256/384/512 (SHA-1 included) falls back to SHA-256 (`unwrap_or`) and so shows up as `DigestMismatch` (inferred). With signed attributes, the messageDigest attribute must equal the computed digest. Without signed attributes the integrity check is taken as passed, but the signature check then returns `false`, so the result is `SignatureInvalid` (inferred from `verify_cms_signature`).
- **Signature check**: only an RSA key (SPKI `rsaEncryption`) with PKCS#1 v1.5 and SHA-256/384/512 is verified, over the DER of the signed attributes. ECDSA, RSA-PSS and anything else return `false` and are reported `SignatureInvalid` — "invalid" where the glossary asks for **unsupported**. The signer certificate is found by issuer and serial; a SubjectKeyIdentifier signer is not found ("not implemented yet") and so fails the same way.
- **Trust check** (`cert::validate_chain`): each certificate's issuer string must equal the next one's subject string, over the `cert_chain` detection extracted — no signature on any certificate is checked, no validity period, no trust anchor. An empty list fails. A self-signed certificate is reported `CertificateNotTrusted` even when the digest and the signature are valid (measured 2026-11-01 on `sign_pdf` output).
- **Coverage**: `detect_modification_after_signing` reports any bytes after the second range; the overall result then becomes `Unknown("modified after signing")`, so every signature but the last in a multiply-signed or later-updated file gets no verdict (inferred). Whether the ranges start at 0 and leave exactly the `/Contents` value uncovered is not checked.
- **Result order**: `Valid` only if all checks pass and coverage is complete; otherwise the first failing of integrity, signature, trust. `CertificateExpired` and `NoSignatures` are never produced (`git grep` finds no construction outside `types.rs`).
- Unsigned attributes (timestamps) are not read.
- **Bounds on input**: `/ByteRange` values come from the file. Each is cast from `i64` to `usize` and added unchecked (`compute_byterange_digest`, `detect_modification_after_signing`), so a negative value panics: `[10 -1 0 0]` panics with "attempt to add with overflow" in a debug build and "slice index starts at 10 but ends at 9" in release (measured 2026-10-30, probe, with a parseable CMS). `verify_all_signatures` copies the whole file (`raw_data().to_vec()`).

## Code
- `justpdf-core/src/sign/verify.rs` — `verify_signature`, `oid_to_digest_algorithm`, `find_message_digest`, `verify_cms_signature`, `find_signer_certificate`, `verify_rsa_signature`
- `justpdf-core/src/sign/cert.rs` — `validate_chain`
- `justpdf-core/src/sign/byterange.rs` — `compute_byterange_digest`, `detect_modification_after_signing`
- `justpdf-core/src/sign/mod.rs` — `verify_all_signatures`
- `justpdf-core/src/sign/types.rs` — `SignatureValidity`, `VerificationResult`

## Reference behaviour
**None** as a comparison of verdicts. OpenSSL `cms -verify` was run on `sign_pdf` output (see [Signing](signing.md)). It accepts a padded `/Contents`, and so does this code since #409. Both also accept non-zero bytes after the first DER element (measured 2026-11-01, review of #408/#409). MuPDF reads only the first element as well (`d2i_PKCS7_bio`, `source/helpers/pkcs7/pkcs7-openssl.c` @ `b487eef`, read by the review). MuPDF's signature error codes (NoSignatures, DigestFailure, SelfSigned, NotTrusted, …) are listed in `docs/mupdf-feature-analysis.md` §3.4, not compared.

## Cross-cutting invariants
- [Signature contents](../invariant/signature-contents.md)

## Blast radius
- [Signature detection](signature-detection.md) — supplies `byte_range`, `contents_raw` and `cert_chain` unchecked; both decode `/Contents` through `cert::decode_contents`.
- [Signing](signing.md) — writes the signatures `tests/sign_roundtrip.rs` checks here: its `/ByteRange` and padding decide whether they verify.
- [Timestamps](timestamps.md) — a signature timestamp is never read, so a time-of-signing claim is never checked.
- [Facade](facade.md), [CLI](cli.md), [Language bindings](language-bindings.md) — exposing verification first needs the unsupported-versus-invalid split and real trust checks (#38).
- [Published docs](published-docs.md) — `README.md` lists "signature detection and verification (RSA)" among the features, which a padded signature now passes for its digest and signature value; the trust check compares names only (above).

## Known holes / open
- A negative `/ByteRange` value panics (measured, above). Not tracked by an issue.
- Unsupported algorithms are reported as invalid, and the trust check compares names only. Tracked: #38.
- Coverage turns the verdict into `Unknown` instead of standing beside it, as the glossary's **coverage** asks (inferred). Tracked: #38.
- Nothing in the repo calls `verify_*` outside tests.
