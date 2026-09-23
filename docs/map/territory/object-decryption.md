# 객체 복호화 (읽기)

## What it is
인증된 문서에서 객체를 로드할 때 문자열과 스트림 데이터를 객체별 키로 복호화한다. 스트림별 `/Crypt` 필터와 `DecodeParms /Name`을 처리한다.

## Governing decisions
**None.**

## Design model
- 훅 위치는 [문서 접근](document-access.md)의 `load_object`(일반 객체)와 `load_compressed_object`(ObjStm 전체)다.
- **ObjStm 안 객체는 두 번 복호화된다**(추론): 스트림 전체를 복호화하고, 꺼낸 객체에 대해 자기 번호로 `decrypt_object`가 다시 돈다.
- 스트림 사전 안의 문자열은 복호화하지 않는다(암호화 쪽도 같은 공백).
- 서명 `/Contents`는 스펙상 암호화되지 않지만 `resolve`를 지나며 복호화된다(추론) — [서명 감지](signature-detection.md).

## Code
- `justpdf-core/src/crypto/decrypt.rs` — `decrypt_object`, `decrypt_bytes`, `stream_crypt_method`, `remove_crypt_filter`
- `justpdf-core/src/parser.rs` — `load_object`, `load_compressed_object`

## Reference behaviour
**None.** 비교 대상 조항: ISO 32000-2 §7.5.7(object stream 안 문자열은 따로 암호화하지 않음), §7.6.2.

## Cross-cutting invariants
**None.**

## Blast radius
- [object streams](object-streams.md) — 이중 복호화의 다른 쪽.
- [객체 암호화](object-encryption.md) — 쓰기 쪽 짝. 어느 값을 암호화하는지의 규칙을 공유해야 한다.
- [암호화 모델](encryption-model.md) — 방식 선택.
- [서명 감지](signature-detection.md), [서명 검증](signature-verification.md) — `/Contents`.

## Known holes / open
- 암호화 + object stream 파일 테스트가 없다.
- Tracked: #32 (ObjStm 이중 복호화)
