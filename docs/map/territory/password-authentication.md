# 비밀번호 인증

## What it is
`/R`에 따라 사용자·소유자 비밀번호를 검증하고 파일 키를 얻는다. 문서를 열 때 빈 비밀번호를 자동 시도하고, `authenticate`가 성공하면 이후 객체 로드가 복호화된다.

## Governing decisions
**None.**

## Design model
- R2–R4: 사용자 비밀번호로 먼저, 실패하면 소유자 비밀번호로 시도한다.
- **R5는 항상 통과한다**: `verify_password_r5`가 해시를 계산하고 버린 뒤 `true`를 돌려준다(주석: 검증이 키 유도 성공에 "내장"). 잘못된 비밀번호로도 쓰레기 키를 얻는다(추론).
- R6 `/Perms` 검증은 `/Perms`가 16바이트 미만이면 조용히 건너뛴다.

## Code
- `justpdf-core/src/crypto/auth.rs` — `authenticate`, `authenticate_r234`, `authenticate_r5`, `authenticate_r6`, `verify_password_r5`, `verify_perms_r6`
- `justpdf-core/src/parser.rs` — `authenticate`, `detect_encryption`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [키 유도](key-derivation.md) — 입력.
- [문서 접근](document-access.md) — 인증이 캐시를 비우고 복호화를 켠다.
- [CLI](cli.md), [파사드](facade.md), [언어 바인딩](language-bindings.md) — `--password`/`authenticate` 노출.
- [압축 파이프라인](compress-pipeline.md) — 암호화 입력 처리 계획이 인증 뒤 재직렬화에 기댄다.

## Known holes / open
- R5 오검증(위). R5 테스트가 없다.
- Tracked: #30 (R5 인증)
