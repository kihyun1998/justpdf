# Signature verification

## What it is
Compares a detected signature's ByteRange digest with the CMS messageDigest, verifies the signature value with the signer certificate, and checks the certificate chain to produce a `SignatureValidity`.

## Governing decisions
**None.**

## Design model
- **It verifies only RSA PKCS#1 v1.5 + SHA-256/384/512.** ECDSA, RSA-PSS and others return `false`, which is reported as `SignatureInvalid` — it goes out as "invalid", not "unsupported".
- An unknown digest OID falls back to SHA-256 (`unwrap_or`) → shows up as `DigestMismatch` (inferred).
- With no signed attributes, the digest is taken as valid ("Assume valid").
- Identifying the signer by SubjectKeyIdentifier is "not implemented yet".
- Chain validation (`validate_chain`) is only an issuer/subject string comparison: no check of signatures, validity periods or a trust store. `CertificateExpired` and `NoSignatures` are never produced.
- Unsigned attributes (timestamps) are not looked at.

## Code
- `justpdf-core/src/sign/verify.rs` — `verify_signature`, `verify_cms_signature`, `verify_rsa_signature`, `find_signer_certificate`, `oid_to_digest_algorithm`, `find_message_digest`
- `justpdf-core/src/sign/cert.rs` — `validate_chain`
- `justpdf-core/src/sign/mod.rs` — `verify_all_signatures`
- `justpdf-core/src/sign/types.rs` — `SignatureValidity`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [Signature detection](signature-detection.md) — the input.
- [Signing](signing.md) — the counterpart (there is no sign → verify test).
- [Timestamps](timestamps.md) — not verified.
- [Facade](facade.md), [CLI](cli.md), [Language bindings](language-bindings.md) — exposing it first needs the "invalid/unsupported" distinction.

## Known holes / open
- Nothing in the repo calls `verify_*` (tests aside). The facade exposes only detection.
- Tracked: #38 (signature verification result model)
