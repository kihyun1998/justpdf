# 객체 암호화 (쓰기)

## What it is
직렬화 직전에 각 객체의 문자열·스트림 데이터를 암호화한다. `EncryptionConfig`가 리비전별 `/Encrypt` 사전과 키를 만들고, `serialize_pdf_encrypted`가 객체마다 `encrypt_object`를 부른다. IV·파일 키·솔트·파일 ID 생성도 여기 있다.

## Governing decisions
**None.**

## Design model
- **IV·키·솔트가 시간에서 나온다**: `generate_iv`는 나노초 시각 + 상수의 MD5("simple deterministic approach for now — in production, use OsRng"), `generate_random_key`(AES-256 파일 키)는 나노초 시각의 SHA-256. 쓰기 시각으로 R6 파일 키를 예측할 수 있고 거친 시계에서 IV가 반복될 수 있다(추론).
- **파일 ID가 고정값이다**: [문서 빌더](document-builder.md)의 `build`와 [CLI](cli.md)의 `cmd_encrypt`가 모두 `generate_file_id(b"justpdf", 0)`을 쓴다. R3/R4에서는 같은 비밀번호·권한이면 모든 문서가 같은 파일 키를 갖는다(추론).
- `OsRng`로 가는 난수원은 이미 의존성 그래프에 있다(`rsa` → `rand_core`, compress-wasm의 `getrandom` `js` 기능도 이 전이 의존 때문에 켜져 있다). 코드가 쓰지 않을 뿐이다.
- 세대 번호는 항상 0. 스트림 사전 안 문자열은 암호화하지 않는다.
- 암호문 문자열은 고바이트를 포함하므로 [객체 직렬화](object-serialization.md)의 hex 경로를 탄다 — #20의 원인이 여기서 드러났다.

## Code
- `justpdf-core/src/crypto/encrypt.rs` — `EncryptionConfig`, `build_r3`, `build_r4`, `build_r6`, `encrypt_object`, `encrypt_bytes`, `generate_iv`, `generate_random_key`, `generate_random_salt`, `generate_file_id`, `make_id_array`
- `justpdf-core/src/crypto/aes_cipher.rs` — `encrypt_aes_cbc`
- `justpdf-core/tests/integration.rs` — `test_encrypt_roundtrip_across_password_combinations`

## Reference behaviour
**None.**

## Cross-cutting invariants
- [객체 구문 왕복](../invariant/object-syntax-roundtrip.md) — 암호문이 바이트 그대로 파일에 남아야 한다.

## Blast radius
- [파일 직렬화](file-serialization.md) — 호출 지점. xref 스트림 경로에는 암호화가 없다.
- [키 유도](key-derivation.md) — `/O`·`/U` 생성.
- [객체 복호화](object-decryption.md) — 대칭 규칙.
- [문서 빌더](document-builder.md), [CLI](cli.md) — 고정 파일 ID의 두 사이트.
- [증분 저장](incremental-save.md) — 원본의 `/Encrypt`·`/ID`를 새 trailer에 옮기지 않는다([증분 trailer](../invariant/incremental-trailer.md)).

## Known holes / open
- 암호학적 난수 미사용(위).
- #20 회귀 테스트는 RC4-128과 AES-128만 돌고 AES-256은 빠져 있다. 커밋 7be23c0 메시지는 AES-256에서도 재현됐다고 적는다. 라운드트립 테스트들은 페이지 수만 확인하고 복호화된 내용을 원본과 비교하지 않는다.
- Tracked: #31 (시각 기반 난수·고정 파일 ID)
