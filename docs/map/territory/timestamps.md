# RFC 3161 타임스탬프

## What it is
타임스탬프 요청(TSQ)을 만들고 응답(TSR)에서 토큰을 꺼내는 DER 코드. 서명 시 호출자가 `SigningOptions.timestamp_token`으로 토큰을 넘기면 서명되지 않은 속성으로 들어간다.

## Governing decisions
**None.**

## Design model
- nonce를 보내지 않고 PKIStatus를 확인하지 않는다. DER 길이는 2바이트까지.
- 주석은 "[0] IMPLICIT"이라 하지만 코드는 universal BOOLEAN을 쓴다.
- 토큰은 **서명 값**을 덮어야 하는데, 서명 값은 `sign_pdf` 안에서만 만들어진다 — 호출 전에 받은 토큰은 서명과 맞을 수 없다(추론).

## Code
- `justpdf-core/src/sign/timestamp.rs` — `create_timestamp_request`, `parse_timestamp_response`, `read_der_length`, `encode_length`

## Reference behaviour
**None.** 코드가 RFC 3161을 인용한다(비교 기록 아님).

## Cross-cutting invariants
**None.**

## Blast radius
- [서명](signing.md) — 토큰 삽입 지점. 올바른 흐름(서명 → 토큰 요청 → 삽입)으로 바꾸려면 서명 쪽 순서가 바뀐다.
- [서명 검증](signature-verification.md) — 토큰을 보지 않는다.

## Known holes / open
- 토큰-서명 순서 문제(위).
- Tracked: #37 (서명 연결·CLI sign)
