# 객체 암호화 (쓰기)

## What it is
직렬화 직전에 각 객체의 문자열·스트림 데이터를 암호화한다. `EncryptionConfig`가 리비전별 `/Encrypt` 사전과 키를 만들고, `serialize_pdf_encrypted`가 객체마다 `encrypt_object`를 부른다. IV·파일 키·솔트·`/Perms` 난수 바이트·파일 ID 생성도 여기 있다.

## Governing decisions
결정 기록(ADR)은 없다. 유지보수자의 판단 두 가지(2026-09-24, #31) — 판단이므로 더 나은 논증이 아니라 유지보수자만 뒤집는다:
- CLI `encrypt`는 원본 `/ID` 첫 원소를 유지한다(아래 Design model). 보여 준 것: MuPDF `pdf-write.c`의 유지 동작, ISO의 영구 식별자 규정, 같은 원본·비밀번호·권한의 R3/R4 재암호화가 같은 파일 키를 갖는다는 결과. 대안: 항상 새 무작위.
- R6 `/Perms` 12–15바이트를 난수로 채운다. 보여 준 것: 스펙이 난수를 요구하고 MuPDF가 `fz_memrnd`로 채운다는 것. 대안: 이번 변경에서 제외.

## Design model
- **난수는 전부 OS 엔트로피다**: IV(`generate_iv`), R6 파일 키(`generate_random_key`), R6 솔트 4개(`generate_random_salt`), R6 `/Perms` 12–15바이트, 새 파일 ID(`random_file_id`)가 모두 `fill_random` → `getrandom` 0.2를 거친다. 난수원이 없으면 `EncryptionError`로 실패하고 다른 값으로 대체하지 않는다. MuPDF도 같은 자리(IV·솔트·R6 키·`/Perms` 12–15·새 `/ID`)를 모두 `fz_memrnd`로 채운다(`pdf-crypt.c`, `pdf-write.c` 원문 대조, 2026-09-24).
- **`getrandom`을 고른 이유**: 0.2가 이미 core의 일반 의존성 그래프에 있었다(`rsa`/`pkcs8`/`signature` → `rand_core` → `getrandom`). 직접 의존으로 올려도 새 크레이트가 없고, wasm32-unknown-unknown 빌드가 `js` 기능을 요구하는 조건도 이전과 같다(compress-wasm이 이미 켠다). `getrandom`의 메이저를 올리면 이 조건이 바뀐다.
- **파일 ID**: [문서 빌더](document-builder.md)의 `build`는 `random_file_id`로 16바이트를 만들고 `/ID`의 두 원소를 같은 값으로 쓴다(새로 쓰는 파일). [CLI](cli.md)의 `cmd_encrypt`는 원본 trailer에 `/ID`가 있으면 첫 원소를 그대로 파일 ID로 쓰고, 없거나 첫 원소가 빈 문자열·문자열 아닌 값이면 `random_file_id`를 쓴다(빈 ID는 키 유도에 아무것도 보태지 않는다). 읽기 쪽 `extract_file_id`(parser)도 같은 첫 문자열 원소를 읽는다 — 첫 원소는 문서의 영구 식별자이기 때문이다(MuPDF도 원본 첫 원소를 유지하고 그것으로 키를 유도한다). 그래서 같은 원본·비밀번호·권한으로 R3/R4 재암호화를 두 번 하면 파일 키가 같다. 이 선택은 유지보수자의 판단이다(2026-09-24, #31): MuPDF·ISO의 영구 식별자 규정과 이 같은-키 결과를 보고 "항상 새 무작위" 대신 골랐다. 이 판단이 다루지 않은 것: CLI가 다시 쓴 파일의 `/ID` 둘째 원소는 첫 원소와 같다(`make_id_array`) — MuPDF는 둘째를 새 난수로 바꾼다.
- `generate_file_id(title, timestamp)`는 공개 API로 남아 있지만 core·CLI 어디서도 부르지 않는다. 같은 입력이면 같은 ID를 낸다.
- 세대 번호는 항상 0. 스트림 사전 안 문자열은 암호화하지 않는다.
- 암호문 문자열은 고바이트를 포함하므로 [객체 직렬화](object-serialization.md)의 hex 경로를 탄다 — #20의 원인이 여기서 드러났다.

## Code
- `justpdf-core/src/crypto/encrypt.rs` — `EncryptionConfig`, `build_r3`, `build_r4`, `build_r6`, `encrypt_object`, `encrypt_bytes`, `fill_random`, `generate_iv`, `generate_random_key`, `generate_random_salt`, `random_file_id`, `generate_file_id`, `make_id_array`, `test_r6_perms_tail_is_random`, `test_r6_file_keys_and_salts_differ_across_builds`, `test_aes_ivs_are_distinct`
- `justpdf-core/src/crypto/aes_cipher.rs` — `encrypt_aes_cbc`
- `justpdf-core/tests/integration.rs` — `test_encrypt_roundtrip_across_password_combinations`, `test_encrypted_builds_get_distinct_file_ids`
- `justpdf-cli/tests/encrypt.rs` — `encrypt_keeps_the_source_permanent_id`, `encrypt_without_source_id_gets_a_fresh_one`, `encrypt_with_empty_source_id_gets_a_fresh_one`

## Reference behaviour
MuPDF 원문(`source/pdf/pdf-crypt.c`, `source/pdf/pdf-write.c`, master, 2026-09-24)과 대조: IV·R6 솔트·R6 파일 키·`/Perms` 12–15·새 `/ID`를 모두 `fz_memrnd`(OS 엔트로피로 시드한 ChaCha20)로 채운다 — justpdf와 같다. 새 `/ID`는 32바이트 난수를 반으로 나눠 두 원소가 **다르고**, 원본에 `/ID`가 있으면 첫 원소를 유지하고 둘째만 새 난수로 바꾼다 — justpdf는 두 원소가 같다.

ISO 32000-1:2008 §14.4 원문(Adobe 무료 사본, 2026-09-24)과 대조: `/ID`는 "optional but should be used"이고, 첫 원소는 영구 식별자로 증분 갱신에서 바뀌지 않으며, 둘째는 마지막 갱신 때의 내용에 따른 변하는 식별자다. "When a file is first written, both identifiers shall be set to the same value" — 새 문서에서 두 원소를 같게 쓰는 [문서 빌더](document-builder.md)가 이를 따른다. "If only the first identifier matches, a different version of the correct file has been found" — CLI가 다시 쓴 파일은 다른 버전이므로 둘째 원소가 달라야 하는데 지금은 첫째와 같다(아래 Known holes). 계산은 시각·경로·크기·Info 값의 MD5를 권고(should)하지만, NOTE가 "all that matters is that the identifier is likely to be unique"라고 하므로 무작위 16바이트는 조항의 목적을 채운다. ISO 32000-2 원문과는 대조하지 않았다 — PDF 2.0에서 `/ID`가 필수인지는 확인되지 않은 공백이다.

## Cross-cutting invariants
- [객체 구문 왕복](../invariant/object-syntax-roundtrip.md) — 암호문이 바이트 그대로 파일에 남아야 한다.

## Blast radius
- [파일 직렬화](file-serialization.md) — 호출 지점. xref 스트림 경로에는 암호화가 없다.
- [키 유도](key-derivation.md) — `/O`·`/U` 생성.
- [객체 복호화](object-decryption.md) — 대칭 규칙.
- [문서 빌더](document-builder.md), [CLI](cli.md) — 파일 ID를 정하는 두 사이트.
- [compress-wasm](compress-wasm.md) — `getrandom`의 wasm `js` 기능을 켜는 곳.
- [증분 저장](incremental-save.md) — 원본의 `/Encrypt`·`/ID`를 새 trailer에 옮겨 적고, 덧붙이는 객체를 원본 파일 키로 암호화한다([증분 trailer](../invariant/incremental-trailer.md)).

## Known holes / open
- CLI `encrypt`가 다시 쓴 파일의 `/ID` 둘째 원소가 첫째와 같다(`make_id_array`). ISO 32000-1 §14.4로는 다른 버전이므로 달라야 한다.
- 키·솔트·IV 테스트는 "빌드마다 다르다"만 본다. 예측 가능성은 바깥에서 관찰되지 않으므로, 시각 기반 생성기로 되돌려도 이 테스트들은 통과한다(2026-09-24 측정: 이전 코드에서 macOS 나노초 시계로 IV 2000회 → 2000종). 상수·재사용 값으로의 회귀만 잡는다.
- #20 회귀 테스트는 RC4-128과 AES-128만 돌고 AES-256은 빠져 있다. 커밋 7be23c0 메시지는 AES-256에서도 재현됐다고 적는다. 라운드트립 테스트들은 페이지 수만 확인하고 복호화된 내용을 원본과 비교하지 않는다.
