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
- `justpdf-core/src/writer/linearize.rs` — `linearize`, `write_linearized_pdf`, `write_linearized_inner`, `build_hint_stream`, `compute_page_offsets`, `create_pdf_at_generations`, `linearize_keeps_source_generations`

## Reference behaviour
**None.** 코드가 "PDF spec section 7.4", "F.3", "Table F.1"을 인용하지만 비교 기록은 없다. 비교 대상: ISO 32000-2 Annex F.

## Cross-cutting invariants
- [xref 항목 형식](../invariant/xref-entry-format.md) — main xref 테이블.
- [원본 세대](../invariant/source-generation.md) — 원본 객체를 `object_refs()`의 xref 세대로 헤더·main xref 항목에 쓴다. 새로 만드는 선형화 사전·힌트 스트림은 세대 0이다. trailer `/Root`·`/Info`는 원본 그대로 복사한다 — 원본에서 resolve되지 않던 참조를 되살리지 않는다.

## Blast radius
- [파일 직렬화](file-serialization.md) — 쓰기 쪽이 xref·trailer를 직접 쓴다.
- [파사드](facade.md) — `is_linearized`가 읽기 쪽을 호출하는 유일한 소비처.
- [페이지 트리](page-tree.md) — 첫 페이지 객체 집합 계산이 페이지 순서에 기댄다.

## Known holes / open
- 쓰기 쪽(`linearize_pdf`)은 재수출만 되고 제품 코드에서 호출되지 않는다. CLI에도 선형화 명령이 없다.
- 읽기 쪽 힌트 헤더 길이·인용 표 번호가 스펙과 다를 수 있다(원문 대조 전까지 판정 보류).
- Tracked: #56 (힌트 테이블 레이아웃)
- 첫 페이지 구간이 모든 페이지를 담는다: 첫 구간에 catalog의 의존 객체를 모으는데 catalog → `/Pages` → `/Kids`로 모든 페이지에 닿는다. 나머지 구간에는 어디서도 참조되지 않는 객체만 남는다(2026-09-29 측정: `linearize_keeps_source_generations`에서 둘째 페이지 객체의 헤더를 세대 0으로 바꾸는 mutation은 나머지 구간 루프에 걸리지 않았고, 참조되지 않는 객체를 넣어야 걸렸다). catalog만의 문제가 아니다: 참조 수집기가 모든 참조를 따라가므로 첫 페이지 하나의 의존 객체도 `/Parent` → `/Pages` → `/Kids`로 형제 페이지 전부를 담는다(2026-09-30 측정: 페이지 객체 4, 6, 8인 3페이지 문서에서 첫 페이지 의존 객체만으로 `[1, 3, 4, 5, 6, 7, 8]`, 페이지 6·8이 `/E` 앞). 그래서 catalog 쪽만 좁혀서는 고쳐지지 않는다. 페이지별 분배가 첫 구간 객체를 건너뛰므로 힌트 테이블의 2..N 페이지는 객체 수 0이 된다(추론 — 코드 읽기). Tracked: #124
