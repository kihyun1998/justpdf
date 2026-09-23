# 포맷 감지

## What it is
입력 파일의 포맷을 판별한다. 확장자 기반(`detect_format`)과 바이트 기반(`detect_format_from_bytes`)이 있다.

## Governing decisions
**None.**

## Design model
- 바이트 기반 감지는 ZIP이면 무조건 `Unknown`을 돌려준다("simplified … let the caller try each format"). EPUB·Office·XPS·CBZ가 모두 ZIP이므로 사실상 확장자에 의존한다.

## Code
- `justpdf-formats/src/detect.rs` — `detect_format`, `detect_format_from_bytes`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [CLI](cli.md) — `cmd_convert`가 감지 결과로 분기한다(MOBI·FB2 분기 누락).
- [포맷 변환 계약](format-document.md) — 감지된 포맷의 구현 선택.

## Known holes / open
- 확장자가 틀린 ZIP 계열 파일은 판별되지 않는다.
