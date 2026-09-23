# 권한 (/P)

## What it is
암호화 사전의 `/P` 비트를 인쇄·복사·수정 등 권한으로 해석하는 모델.

## Governing decisions
**None.**

## Design model
- **아무것도 권한을 강제하지 않는다**: `can_*` 메서드의 제품 코드 호출처가 없다(`crypto/types.rs`와 통합 테스트만). 사용자 비밀번호로 연 문서도 CLI `decrypt`로 제한 없는 평문이 된다.
- CLI `encrypt`는 `--no-print`·`--no-copy`만 노출한다.

## Code
- `justpdf-core/src/crypto/types.rs` — `Permissions`, `can_print`, `can_print_high_quality`, `allow_all`
- `justpdf-core/src/parser.rs` — `permissions`

## Reference behaviour
**None.** 비교 대상 조항: ISO 32000-2 §7.6.4.2(Table 22).

## Cross-cutting invariants
**None.**

## Blast radius
- [CLI](cli.md) — encrypt/decrypt.
- [암호화 모델](encryption-model.md) — `/P` 저장.

## Known holes / open
- 권한을 강제하지 않는다는 선택을 정한 기록이 없다(의도인지 누락인지 알 수 없다).
- Tracked: #60 (권한 미강제)
