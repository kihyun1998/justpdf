# RFC 3161 timestamps

## What it is
Hand-written DER for the two ends of an RFC 3161 exchange: `create_timestamp_request` builds a TimeStampReq (TSQ) for a digest, and `parse_timestamp_response` takes the token out of a TimeStampResp (TSR). The HTTP exchange with a timestamp authority is the caller's. When signing, a token passed as `SigningOptions::timestamp_token` is put into the SignerInfo as the `id-aa-timeStampToken` unsigned attribute ([Signing](signing.md)).

## Governing decisions
**None.** That core stays network-free and the HTTP client belongs to the CLI is a decision recorded on the tracker (#180, cited in #309, 2026-10-08), not in an ADR.

## Design model
From reading the source (`sign/timestamp.rs`, `sign/sign_pdf.rs — build_signer_info`).
- **Request**: `SEQUENCE { INTEGER 1, MessageImprint { AlgorithmIdentifier { OID } , OCTET STRING digest }, BOOLEAN TRUE }`. No nonce and no policy are sent. The AlgorithmIdentifier carries no NULL parameters here, while the signer's carries NULL (`build_algorithm_identifier`). `certReq` is written as a universal BOOLEAN, which matches RFC 3161's `certReq BOOLEAN DEFAULT FALSE` (from memory; not read raw in this run); the code comment calling it "context tag [0] IMPLICIT" is what is wrong.
- **Response**: skips the outer SEQUENCE header and the first element (PKIStatusInfo), and returns every byte after it as the token, without reading the token's own length. PKIStatus is not checked, so a rejection without a token is an error only because no bytes follow, and a response with trailing data returns them inside the token (inferred).
- **DER lengths**: `read_der_length` accepts short form and the 1- and 2-byte long forms and errors on longer ones; `encode_length` writes at most 2 length bytes and is wrong for 65,536 bytes or more (inferred; a digest is at most 64 bytes, so the request is not affected).
- **Ordering**: a signature timestamp must cover the signature value, and the signature value is made inside `sign_pdf`, after the caller already had to pass the token, so no token obtained this way can match (inferred, #180).
- **Bounds on input**: the response comes from the caller; lengths over 2 bytes are refused, and nothing reads past the slice. The token's size counts against [Signing](signing.md)'s 16,384-byte placeholder.

## Code
- `justpdf-core/src/sign/timestamp.rs` — `create_timestamp_request`, `parse_timestamp_response`, `wrap_sequence`, `encode_length`, `read_der_length`
- `justpdf-core/src/sign/sign_pdf.rs` — `build_signer_info`
- `justpdf-core/src/sign/types.rs` — `SigningOptions`

## Reference behaviour
**None.** The code cites RFC 3161; no exchange with a real or test TSA is recorded.

## Cross-cutting invariants
**None.**

## Blast radius
- [Signing](signing.md) — inserts the token; the correct flow (sign, request a token over the signature value, insert it) moves this step inside `sign_pdf` behind a caller-supplied client (#180).
- [Signature verification](signature-verification.md) — never reads the token, so a wrong token passes unnoticed.
- [CLI](cli.md) — `--timestamp-url` is to call a TSA through this code once #180 and #181 land (#309).

## Known holes / open
- A token passed in cannot cover the signature value. Tracked: #180.
- No nonce, no PKIStatus check, and the token is taken as "everything after the status". Tracked: #180.
- CLI access to a TSA. Tracked: #309.
- Signature validity, which a timestamp depends on. Tracked: #37.
