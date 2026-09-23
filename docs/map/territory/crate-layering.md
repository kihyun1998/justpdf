# 크레이트 레이어링과 기능 격리

## What it is
워크스페이스를 외부 의존성 계층으로 나누고(core → +render → +formats/+special), 선택 기능이 무거운 레이어(render)를 필요할 때만 끌어오도록 기능 플래그로 게이트하는 구조. `scripts/check-feature-isolation.sh`가 CI에서 이 약속 일부를 검사한다.

## Governing decisions
- [ADR-0001](../../adr/0001-crates-split-by-dependency-layer.md) — 일차 원칙은 외부 의존성 계층화. 단일 크레이트 + 기능 플래그 대안을 기각.
- [ADR-0002](../../adr/0002-language-bindings-outside-workspace.md) — 바인딩은 워크스페이스 밖, compress-wasm은 안.
- [ADR-0003](../../adr/0003-cli-is-end-product-not-a-dependency-layer.md) — CLI는 이 원칙의 대상이 아니다.

## Design model
- 격리 스크립트는 `cargo tree --depth 1 --prefix none`으로 render의 있음/없음을 확인하고(트리 문자는 CI에서 ASCII, 터미널에서 UTF-8이라 쓰지 않는다 — #62 전까지 CI에서 "있음" 검사는 늘 실패, "없음" 검사는 늘 통과했다), `cargo build`는 `special/barcode`, `special/all`, `formats/all`에만 한다.
- **스크립트가 보지 못하는 것**: 기능별 단독 빌드. `epub`·`office`는 `cargo tree`로는 render가 없어 통과하지만 단독으로 컴파일되지 않는다([EPUB](epub-input.md)). 워크스페이스 빌드에서는 CLI가 켜는 `formats/all`이 기능 통합으로 이를 가린다.
- 스크립트의 범위는 손으로 쓴 목록이다. 새 기능이 생기면 스크립트에 추가하지 않는 한 검사되지 않는다.

## Code
- `scripts/check-feature-isolation.sh` — `assert_dep_absent`, `assert_dep_present`, `assert_build_ok`
- `Cargo.toml` — `members`
- `justpdf-formats/Cargo.toml` — `plaintext`, `mobi`, `fb2`
- `justpdf-special/Cargo.toml` — `ocr`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [포맷 변환 계약](format-document.md), [plaintext](plaintext-input.md), [EPUB](epub-input.md), [Office](office-input.md), [OCR](ocr.md), [바코드](barcode.md) — 기능 게이트 대상.
- [파사드](facade.md) — core+render 구성.
- [언어 바인딩](language-bindings.md) — 워크스페이스 경계(ADR과 불일치).
- [CI](ci.md) — 스크립트 실행 지점.

## Known holes / open
- 기능별 단독 빌드 검사 부재(위). #2의 수락 기준이 요구했던 검사다.
- Tracked: #35 (epub·office 단독 빌드), #59 (CI 게이트 공백)
