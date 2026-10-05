# RFC 3161 timestamps

## What it is
DER code that builds a timestamp request (TSQ) and pulls the token out of a response (TSR). When signing, a token the caller passes as `SigningOptions.timestamp_token` goes in as an unsigned attribute.

## Governing decisions
**None.**

## Design model
- It sends no nonce and does not check PKIStatus. DER lengths go up to 2 bytes.
- The comment says "[0] IMPLICIT" but the code uses a universal BOOLEAN.
- The token has to cover the **signature value**, and the signature value is only made inside `sign_pdf` — a token obtained before the call cannot match the signature (inferred).

## Code
- `justpdf-core/src/sign/timestamp.rs` — `create_timestamp_request`, `parse_timestamp_response`, `read_der_length`, `encode_length`

## Reference behaviour
**None.** The code cites RFC 3161 (not a comparison record).

## Cross-cutting invariants
**None.**

## Blast radius
- [Signing](signing.md) — where the token is inserted. Switching to the correct flow (sign → request token → insert) changes the order on the signing side.
- [Signature verification](signature-verification.md) — does not look at the token.

## Known holes / open
- The token-signature ordering problem (above).
- Tracked: #180 (token ordering, nonce, PKIStatus), #37 (signature validity, which this depends on)
