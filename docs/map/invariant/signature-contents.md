# Signature contents

## The fact
A signature dictionary's `/Contents` is a fixed-size hex string reserved before the signature exists: the CMS `ContentInfo` DER followed by zero bytes up to the reserved length. A reader must decode **the first DER element only** — take its length from its own header and ignore the padding — because a strict whole-buffer DER decode rejects the trailing zeros. Both readers do so through one function, `cert::decode_contents` (#409). The writer leaves the whole string, `<` and `>` included, out of the `/ByteRange` (#408). Check: `/Contents` written by `sign_pdf`, and by any other signer that reserves space, decodes to the same `ContentInfo` with and without the padding.

## Why it is cross-cutting
One site writes the padding and two read it. Since #409 the two readers share `cert::decode_contents`; the writer stays separate (inferred, read at 88b2a08 and #409):
- [Signing](../territory/signing.md) writes it: `sign_pdf` reserves `/Contents` in `build_pdf_with_placeholder` and inserts the CMS blob into the placeholder, the rest staying zero.
- [Signature detection](../territory/signature-detection.md) reads it: `detect.rs` keeps the raw string as `contents_raw`, and `cert.rs — extract_certificates` decodes it with `decode_contents`, returning an empty list when that fails.
- [Signature verification](../territory/signature-verification.md) reads it: `verify.rs — verify_signature` decodes `contents_raw` with `decode_contents`, reporting `Unknown("CMS parse error…")` when that fails.

A reader that decodes `/Contents` without `decode_contents` fails on every padded signature, and a change to how the writer pads moves both readers.

## Territories it holds in
- [Signing](../territory/signing.md) — `build_pdf_with_placeholder`, the placeholder fill in `sign_pdf`.
- [Signature detection](../territory/signature-detection.md) — `extract_certificates` through `decode_contents`.
- [Signature verification](../territory/signature-verification.md) — `verify_signature` through `decode_contents`.

## What a violation looks like
Every real signature — justpdf's own, and other signers' that reserve space, which is the normal practice — fails to parse: verification reports a CMS parse error instead of a verdict, and detection reports no certificates (measured 2026-10-30 by the map rewrite with a probe crate; `cert_chain` empty on zero-padded `/Contents`). Tests that build `/Contents` from an exact-length blob pass, which is why the suite does not see it.

## Discovery history
Reported by the content-text-signatures rewrite of the map on 2026-10-30, measured there; made an invariant on the maintainer's answer the same day. The same rewrite measured that justpdf's signatures are invalid for a second, separate reason — the digest is computed before `/ByteRange` is filled in ([Signing](../territory/signing.md)). Both were fixed together on 2026-11-01: #409 by `decode_contents`, and #408 by writing `/ByteRange` before the digest and leaving the delimiters outside it. The maintainer added the delimiters to #408's scope from #37. `tests/sign_roundtrip.rs` and `certificates_are_read_from_a_zero_padded_contents` pin them; each failed with the strict decoder restored (*measured* 2026-11-01).

## Where it will recur
**Code that reads or writes a signature `/Contents` (or a document timestamp's).** Command: `rg -n 'contents_raw|from_der|decode_contents' justpdf-core/src/sign`. A new reader goes through `decode_contents`; a test reads a padded `/Contents` from `sign_pdf`'s output and expects the same result as from the bare DER.
