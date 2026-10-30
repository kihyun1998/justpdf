# Signature contents

## The fact
A signature dictionary's `/Contents` is a fixed-size hex string reserved before the signature exists: the CMS `ContentInfo` DER followed by zero bytes up to the reserved length. A reader must decode **the first DER element only** — take its length from its own header and ignore the padding — because a strict whole-buffer DER decode rejects the trailing zeros. Check: `/Contents` written by `sign_pdf`, and by any other signer that reserves space, decodes to the same `ContentInfo` with and without the padding.

## Why it is cross-cutting
One site writes the padding and two read it, and none calls another or a shared decoder (inferred, read at 88b2a08):
- [Signing](../territory/signing.md) writes it: `sign_pdf` reserves `/Contents` in `build_pdf_with_placeholder` and inserts the CMS blob into the placeholder, the rest staying zero.
- [Signature detection](../territory/signature-detection.md) reads it: `detect.rs` keeps the raw string as `contents_raw`, and `cert.rs — extract_certificates` calls `ContentInfo::from_der` on the whole buffer, returning an empty list when it fails.
- [Signature verification](../territory/signature-verification.md) reads it: `verify.rs — verify_signature` calls `ContentInfo::from_der` on the whole `contents_raw`, reporting `Unknown("CMS parse error…")` when it fails.

A fix in one reader leaves the other failing, and a change to how the writer pads (or a signer that pads differently) moves both.

## Territories it holds in
- [Signing](../territory/signing.md) — `build_pdf_with_placeholder`, the placeholder fill in `sign_pdf`.
- [Signature detection](../territory/signature-detection.md) — `extract_certificates` on `contents_raw`.
- [Signature verification](../territory/signature-verification.md) — `verify_signature`'s `ContentInfo::from_der`.

## What a violation looks like
Every real signature — justpdf's own, and other signers' that reserve space, which is the normal practice — fails to parse: verification reports a CMS parse error instead of a verdict, and detection reports no certificates (measured 2026-10-30 by the map rewrite with a probe crate; `cert_chain` empty on zero-padded `/Contents`). Tests that build `/Contents` from an exact-length blob pass, which is why the suite does not see it.

## Discovery history
Reported by the content-text-signatures rewrite of the map on 2026-10-30, measured there; made an invariant on the maintainer's answer the same day. The same rewrite measured that justpdf's signatures are invalid for a second, separate reason — the digest is computed before `/ByteRange` is filled in ([Signing](../territory/signing.md)). Tracked: #409 (parsing), #408 (digest).

## Where it will recur
**Code that reads or writes a signature `/Contents` (or a document timestamp's).** Command: `rg -n 'contents_raw|from_der' justpdf-core/src/sign`. A shared helper that decodes the first DER element and returns the rest as padding would make the two readers one site; a test pads a valid CMS blob with zeros and expects the same result as without.
