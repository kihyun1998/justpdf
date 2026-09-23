# PDF 함수

## What it is
셰이딩·전이 함수 등에 쓰이는 PDF 함수 객체의 파서와 평가기. 지수(Type 2), 스티칭(Type 3), PostScript 계산기(Type 4)를 지원한다.

## Governing decisions
**None.**

## Design model
- **Type 0(샘플) 함수가 없다.** `parse`는 2·3·4만 받는다.
- 입력 클램프는 첫 Domain 쌍만 쓴다. PostScript 평가기는 입력 수가 Domain 쌍보다 많으면 슬라이스 범위를 넘을 수 있다(추론: panic).
- 스티칭의 하위 함수가 참조면 걸러진다(`Dict`/`Stream`만 받음 — 추론).
- PostScript 코드는 `Stream.data` 원바이트에 `from_utf8`을 한다. 렌더러가 스트림을 디코드하지 않고 넘기므로 Flate 압축된 Type 4는 파싱에 실패한다(추론).

## Code
- `justpdf-core/src/function.rs` — `PdfFunction`, `PsOp`, `parse`, `evaluate`, `parse_ps_code`, `execute_ps_ops`, `clamp_input`, `clamp_output`

## Reference behaviour
**None.** 코드가 "PDF 2.0 spec, section 7.10"을 인용한다(비교 기록 아님).

## Cross-cutting invariants
**None.**

## Blast radius
- [렌더 셰이딩](render-shading.md) — 함수 기반 셰이딩만 `PdfFunction`을 쓰고, 축·방사 셰이딩은 C0/C1/Bounds를 손으로 다시 읽는다. 함수 지원을 늘려도 축·방사에는 반영되지 않는다.

## Known holes / open
- 스티칭 함수 테스트가 없다.
- Tracked: #47 (함수 Type 0·Type 4)
