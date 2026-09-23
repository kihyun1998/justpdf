# 선형화 (linearization)

## What it is
웹 최적화(Fast Web View) 파일 포맷. 읽기 쪽은 첫 객체가 `/Linearized` 사전인지 판별하고 페이지 오프셋 힌트 테이블을 비트 단위로 읽는다. 쓰기 쪽은 고정폭 숫자로 선형화 사전을 쓰고, 힌트 스트림·첫 페이지 객체·나머지·메인 xref 순으로 두 번 패스해 파일을 만든다.

## Governing decisions
**None.**

## Design model
- 쓰기: 숫자를 10자리 0 패딩으로 써서 "패스 간 사전 크기가 같아 진동하지 않게" 한다. 첫 페이지 xref는 생략한다(주석은 스펙이 허용한다고 하나 의심스럽다 — 추론).
- 쓰기: 힌트 스트림 사전이 `/Type /XRef`이며 `/S`가 없다.
- 읽기: 판별만 한다(`/L`과 실제 길이 대조 없음). 헤더 항목 6–9는 읽고 버린다. 공유 객체 힌트는 파싱하지 않는다.
- 읽기 쪽 힌트 헤더를 36바이트로 읽는데, 스펙 표로는 16비트 항목이 더 있어 44바이트일 수 있다(추론 — 원문 대조 필요).
- 문서 열기 경로는 선형화 정보를 쓰지 않는다.

## Code
- `justpdf-core/src/linearized.rs` — `detect_linearization`, `is_linearized`, `read_linearization`, `parse_hint_tables`, `PageOffsetHint`, `BitReader`
- `justpdf-core/src/writer/linearize.rs` — `linearize`, `write_linearized_pdf`, `write_linearized_inner`, `build_hint_stream`, `compute_page_offsets`

## Reference behaviour
**None.** 코드가 "PDF spec section 7.4", "F.3", "Table F.1"을 인용하지만 비교 기록은 없다. 비교 대상: ISO 32000-2 Annex F.

## Cross-cutting invariants
**None.**

## Blast radius
- [파일 직렬화](file-serialization.md) — 쓰기 쪽이 xref·trailer를 직접 쓴다.
- [파사드](facade.md) — `is_linearized`가 읽기 쪽을 호출하는 유일한 소비처.
- [페이지 트리](page-tree.md) — 첫 페이지 객체 집합 계산이 페이지 순서에 기댄다.

## Known holes / open
- 쓰기 쪽(`linearize_pdf`)은 재수출만 되고 제품 코드에서 호출되지 않는다. CLI에도 선형화 명령이 없다.
- 읽기 쪽 힌트 헤더 길이·인용 표 번호가 스펙과 다를 수 있다(원문 대조 전까지 판정 보류).
- Tracked: #56 (힌트 테이블 레이아웃)
