# 서명 검증

## What it is
감지된 서명의 ByteRange 다이제스트를 CMS의 messageDigest와 비교하고, 서명자 인증서로 서명 값을 검증하고, 인증서 체인을 확인해 `SignatureValidity`를 낸다.

## Governing decisions
**None.**

## Design model
- **RSA PKCS#1 v1.5 + SHA-256/384/512만 검증한다.** ECDSA·RSA-PSS 등은 `false`를 돌려주고, 이것이 `SignatureInvalid`로 보고된다 — "지원 안 함"이 아니라 "무효"로 나간다.
- 모르는 다이제스트 OID는 SHA-256으로 떨어진다(`unwrap_or`) → `DigestMismatch`로 보인다(추론).
- signed attributes가 없으면 다이제스트를 유효로 간주한다("Assume valid").
- SubjectKeyIdentifier 서명자 식별은 "not implemented yet".
- 체인 검증(`validate_chain`)은 issuer/subject 문자열 비교뿐이다: 서명·유효기간·신뢰 저장소 확인 없음. `CertificateExpired`·`NoSignatures`는 생성되지 않는다.
- 서명되지 않은 속성(타임스탬프)은 보지 않는다.

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
- [서명 감지](signature-detection.md) — 입력.
- [서명](signing.md) — 짝(서명 → 검증 테스트가 없다).
- [타임스탬프](timestamps.md) — 검증되지 않는다.
- [파사드](facade.md), [CLI](cli.md), [언어 바인딩](language-bindings.md) — 노출하려면 "무효/지원 안 함" 구분부터.

## Known holes / open
- 저장소 안에 `verify_*` 호출자가 없다(테스트 제외). 파사드는 감지만 노출한다.
- Tracked: #38 (서명 검증 결과 모델)
