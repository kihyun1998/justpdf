# 키 유도와 /O /U /OE /UE /Perms 계산

## What it is
비밀번호 패딩, 파일 암호화 키 계산, `/O`·`/U`(R2–R4), `/OE`·`/UE`·`/Perms`(R5–R6) 값 생성, 객체별 키 유도(Algorithm 1). 인증(읽기)과 암호화(쓰기)가 같은 함수를 공유한다.

## Governing decisions
**None.**

## Design model
- 쓰기가 지원하는 리비전은 R3·R4·R6뿐이다. R2·R5는 읽기만.
- 비밀번호는 127바이트로 자른다. SASLprep 정규화는 없다.
- `generate_values_r6`은 결정적이다: 파일 키·솔트 4개·`/Perms` 12–15바이트(`perms_random`)를 모두 호출자가 넘긴다. 난수는 [객체 암호화](object-encryption.md)의 `build_r6`이 만든다.
- `compute_file_key_r5`의 문서 주석은 "validation_salt"라고 하지만 호출자는 key salt를 넘긴다.

## Code
- `justpdf-core/src/crypto/key.rs` — `pad_password`, `PADDING`, `compute_file_encryption_key_r234`, `compute_o_value_r234`, `compute_u_value_r234`, `recover_user_password_from_owner_r234`, `compute_file_key_r5`, `compute_hash_r6`, `compute_object_key`, `generate_o_u_values_r234`, `generate_values_r6`

## Reference behaviour
**None.** 코드는 ISO 32000-1:2008 §7.6.3.3(Algorithm 2)와 PDF Reference 1.7 번호("Table 3.18")를 인용하고, R6 알고리즘 2.A/2.B는 인용 없이 구현한다(ISO 32000-2 출처). 스펙 원문과 대조하거나 제3자 도구로 만든 암호화 파일로 확인한 기록은 없다.

## Cross-cutting invariants
- [객체 구문 왕복](../invariant/object-syntax-roundtrip.md) — 여기서 만든 `/O`·`/U` 바이너리 값이 파일에 그대로 남아야 인증이 된다. #20이 이 경계에서 났다: 원인은 키 유도가 아니라 직렬화였다.

## Blast radius
- [비밀번호 인증](password-authentication.md), [객체 암호화](object-encryption.md) — 같은 함수의 두 소비처.
- [객체 직렬화](object-serialization.md) — 값이 hex로 나가야 한다.

## Known holes / open
- R3·R4·R6은 자기 출력으로만 검증한다(생성 → 인증). R5는 qpdf가 쓴 외부 픽스처로 검증한다([비밀번호 인증](password-authentication.md#known-holes--open)). R2 테스트는 없다.
