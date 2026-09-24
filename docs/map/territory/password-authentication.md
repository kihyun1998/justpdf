# 비밀번호 인증

## What it is
`/R`에 따라 사용자·소유자 비밀번호를 검증하고 파일 키를 얻는다. 문서를 열 때 빈 비밀번호를 자동 시도하고, `authenticate`가 성공하면 이후 객체 로드가 복호화된다.

## Governing decisions
**None.**

## Design model
- R2–R4: 사용자 비밀번호로 먼저, 실패하면 소유자 비밀번호로 시도한다.
- R5: `verify_password_r5`가 SHA-256(비밀번호[..127] ‖ 검증 salt ‖ 소유자일 때 `/U`[0..48])을 `/U`[0..32](소유자는 `/O`[0..32])와 비교한다. 키는 `compute_file_key_r5`가 `/UE`·`/OE`를 풀어 얻는다. R5 `/Perms`는 검증하지 않는다(R6만).
- 문서를 열 때 빈 비밀번호를 자동 시도하고, 성공하면 인증된 상태로 고정된다 — 이후 `authenticate`는 "이미 인증됨"으로 바로 돌아간다. 그래서 검증이 틀리면(항상 통과) 빈 비밀번호 자동 시도가 쓰레기 키로 문서를 고정하고, 올바른 비밀번호로도 읽을 수 없게 된다. #30 전 R5가 정확히 이 상태였다(2026-09-23 실측: 사용자 비밀번호가 있는 qpdf R5 파일이 열자마자 `is_authenticated() = true`, 어떤 비밀번호로도 `Ok`, 텍스트 추출은 `invalid PKCS#7 padding`).
- R6 `/Perms` 검증은 `/Perms`가 16바이트 미만이면 조용히 건너뛴다.

## Code
- `justpdf-core/src/crypto/auth.rs` — `authenticate`, `authenticate_r234`, `authenticate_r5`, `authenticate_r6`, `verify_password_r5`, `verify_perms_r6`
- `justpdf-core/tests/integration.rs` — `test_r5_wrong_password_rejected`, `test_r5_user_password_decrypts`, `test_r5_owner_password_decrypts`, `test_r5_is_not_authenticated_on_open_when_user_password_set`
- `justpdf-core/src/parser.rs` — `authenticate`, `detect_encryption`

## Reference behaviour
2026-09-23, 원본 소스를 직접 읽음(#30):
- **pdf.js** `src/core/crypto.js` — `PDF17`(R5)의 `_hash`는 SHA-256 한 번, `PDF20`(R6)은 Algorithm 2.B. `checkUserPassword`/`checkOwnerPassword`는 해시를 `/U`·`/O`의 앞 32바이트와 비교하고, 소유자 해시 입력에 `/U` 48바이트를 붙인다. 소유자 시도는 비밀번호가 비어 있지 않을 때만.
- **MuPDF** `source/pdf/pdf-crypt.c` — `pdf_compute_encryption_key_r5`(ExtensionLevel 3 algorithm 3.2a)가 같은 입력으로 검증 해시를 만들고, `pdf_authenticate_user_password`/`pdf_authenticate_owner_password`가 `/U`·`/O` 앞 32바이트와 비교한다.
- `/Perms`: pdf.js는 `#createEncryptionKey20`에 넘기지만 본문에서 쓰지 않고, MuPDF는 R6을 쓸 때만 계산한다 — 두 구현 모두 읽을 때 검증하지 않는다. justpdf는 R6에서만 검증한다.
- 두 구현과 justpdf의 R5 검증·키 유도가 일치한다. 판정은 qpdf가 쓴 R5 파일로 한다(아래 픽스처). R5는 PDF 1.7 Adobe Extension Level 3의 리비전이라 ISO 32000-2 원문의 대상이 아니다.

## Cross-cutting invariants
**None.**

## Blast radius
- [키 유도](key-derivation.md) — 입력.
- [문서 접근](document-access.md) — 인증이 캐시를 비우고 복호화를 켠다.
- [CLI](cli.md), [파사드](facade.md), [언어 바인딩](language-bindings.md) — `--password`/`authenticate` 노출.
- [압축 파이프라인](compress-pipeline.md) — 암호화 입력 처리 계획이 인증 뒤 재직렬화에 기댄다.

## Known holes / open
- **R5 픽스처**(`justpdf-core/tests/fixtures/`): qpdf 12.3.2(pikepdf 10.13.0)로 만든 AES-256 R5 파일 두 개. 우리 코드와 독립된 판정자다. 내용은 Helvetica 한 줄 "R5 secret text".
  - `aes256_r5_user_owner.pdf` — 사용자 `userpw`, 소유자 `ownerpw`(sha256 `42ee0c15def7f19bee64b219333bcfc476b2b771cc7ed17f718cca8a06fd627d`)
  - `aes256_r5_empty_user.pdf` — 사용자 빈 비밀번호, 소유자 `ownerpw`(sha256 `eb50511b51d24c965dd5841ea5f359e051593af37b958a1d2ee805160002876b`)
  - 다시 만들기: `pdf.save(path, encryption=pikepdf.Encryption(owner=..., user=..., R=5), static_id=True)` — 페이지는 `/Type /Font /Subtype /Type1 /BaseFont /Helvetica`를 `/F1`로 둔 `BT /F1 24 Tf 72 720 Td (R5 secret text) Tj ET`. salt가 난수라 바이트는 매번 달라진다.
- 비밀번호가 127바이트를 넘는 경우(잘라내기)를 실행하는 테스트가 없다.
- 이미 인증된 문서에 `authenticate`를 다시 부르면 비밀번호를 보지 않고 `Ok`다(사용자 비밀번호로 연 문서에 소유자 비밀번호를 넣어도 같다).
