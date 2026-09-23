# 암호화 모델 (보안 핸들러·암호화 사전)

## What it is
`/Encrypt` 사전의 파싱·직렬화와, `/V`·`/CF`·`/StmF`·`/StrF`로부터 문자열·스트림에 쓸 암호 방식(RC4/AESV2/AESV3/None)을 정하는 상태(`SecurityState`). 읽기·쓰기 암호화 경로가 모두 이 모델을 공유한다.

## Governing decisions
**None.**

## Design model
- `/Filter /Standard`만 받는다(문서 열기에서 판정).
- V=4면 이름 붙은 crypt filter를 쓰고, 모르는 V는 V2(RC4)로 떨어진다.
- `to_pdf_dict`는 항상 `/AuthEvent /DocOpen`을 쓴다.
- `/EncryptMetadata false`는 어느 스트림을 복호화하는지에 영향을 주지 않는다.

## Code
- `justpdf-core/src/crypto/types.rs` — `EncryptionDict`, `from_dict`, `to_pdf_dict`, `key_length_bytes`, `CryptFilterMap`, `CryptFilter`, `CryptMethod`, `SecurityState`, `resolve_crypt_method`

## Reference behaviour
**None.** 비교 대상 조항: ISO 32000-2 §7.6.2–7.6.3, §7.6.5(crypt filters).

## Cross-cutting invariants
**None.**

## Blast radius
- [키 유도](key-derivation.md), [비밀번호 인증](password-authentication.md) — `/R`·`/O`·`/U` 등 사전 값의 소비처.
- [객체 복호화](object-decryption.md), [객체 암호화](object-encryption.md) — 방식 선택의 소비처.
- [문서 접근](document-access.md) — 핸들러 판정.
- [권한](permissions.md) — `/P`.

## Known holes / open
- `/EncryptMetadata false` 미반영.
