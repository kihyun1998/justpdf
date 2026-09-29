# 트리 순회 순환

## The fact
PDF 객체 그래프를 트리로 걷는 코드(`/Kids`, `/First`·`/Next`)는 **조상 체인**을 들고 걷는다: 지금 걷는 노드 위의 참조들. 조상을 다시 만나면 순환이다. 조상이 아닌 노드를 두 번 만나는 것(같은 kid를 두 번 나열한 공유 노드)은 순환이 아니며 두 번 걷는다. 간접 참조만 조상이 될 수 있다 — 직접 dict는 자기를 가리킬 수 없다.

순환을 만났을 때:
- **문서 구조 트리**(페이지 트리, AcroForm 필드 트리, 서명 감지의 필드 트리, 아웃라인) — `JustPdfError::CircularReference`(되돌아간 조상의 번호)로 실패한다.
- **조회 테이블 트리**(페이지 레이블 숫자 트리, 첨부파일 이름 트리, 이름 있는 목적지 이름 트리) — 그 kid만 건너뛰고 나머지를 돌려준다. 루트가 간접 참조면 호출자가 루트를 조상에 먼저 넣는다(루트로 돌아가는 kid가 루트 항목을 한 번 더 내지 않도록).

이 분할은 **메인테이너의 판단**이다(2026-09-29, #52). 판단 근거로 본 것: MuPDF 원문 — `pdf-page.c`(`pdf_load_page_tree_imp`, "cycle in page tree" throw), `pdf-form.c`(`pdf_lookup_field_imp`, "cycle in fields" throw), `pdf-outline.c`(`pdf_test_outline`, "Cycle detected in outlines" throw), `pdf-nametree.c`(`pdf_lookup_name_imp`·`pdf_load_name_tree_imp`·`pdf_lookup_number_imp`, 순환 가지에서 조용히 중단). 조상 체인 방식은 MuPDF `pdf_cycle`(부모 쪽 연결 리스트)에서 끌어낸 **도출**이다 — MuPDF 아웃라인만 전역 mark bits를 쓴다.

## Why it is cross-cutting
트리 워커가 여덟 벌이고 서로 호출하지 않는다: `walk_page_tree`, `walk_page_tree_find`, `walk_field_tree`, `collect_sig_fields`, `read_outline_siblings`, `parse_number_tree`, `collect_name_tree_values`, `parse_name_tree`. #52는 다섯을 적었고 나머지 셋(첨부파일·목적지 이름 트리, 서명 감지)은 grep으로 찾았다. 방어가 없던 여덟 곳 모두 순환 입력에서 스택 오버플로로 프로세스가 죽었다(2026-09-29 재현, `tests/tree_cycles.rs`) — `Result`로 잡을 수 없는 실패다.

## Territories it holds in
- [페이지 트리](../territory/page-tree.md) — `walk_page_tree`, `walk_page_tree_find`: `/Pages` 노드를 조상으로 기록. 오류.
- [AcroForm](../territory/acroform.md) — `walk_field_tree`: `/Kids`가 있는 필드를 조상으로 기록. 오류.
- [서명 감지](../territory/signature-detection.md) — `collect_sig_fields`: 같은 규칙. 오류.
- [아웃라인](../territory/outlines.md) — `read_outline_siblings`: 자식을 읽는 동안 항목을 조상으로 기록. 오류. 같은 층의 `/Next` 고리는 기존대로 조용히 멈춘다(#52 판단이 다루지 않은 이웃).
- [페이지 레이블](../territory/page-labels.md) — `parse_number_tree`: 건너뜀.
- [첨부파일](../territory/embedded-files.md) — `collect_name_tree_values`: 건너뜀.
- [아웃라인](../territory/outlines.md) — `parse_name_tree`(이름 있는 목적지): 건너뜀.

## What a violation looks like
- (#52 전) `/Kids`가 조상을 가리키는 페이지 트리에서 `collect_pages`가 스택 오버플로로 프로세스를 죽였다. `get_page`는 `/Count` 가지치기 덕에 작은 인덱스에서는 죽지 않고 **같은 페이지를 다른 인덱스로** 돌려줬다(인덱스 3 → 인덱스 0과 같은 객체 4).
- 조상 체인 대신 전역 visited를 쓰면 공유 노드를 순환으로 오판한다 — `*_shared_kid_*` 테스트가 잡는다.

## Discovery history
2026-09-23 맵 작성 중 코드 읽기로 발견(#52, 추론). 2026-09-29 여덟 사이트 모두 실행으로 재현하고 고쳤다.

## Where it will recur
**`/Kids`·`/First`·`/Next`·`/Parent`를 따라 재귀하거나 반복하는 새 코드는 이 불변식의 대상이다.** 찾는 명령: `rg -n 'b"Kids"|b"First"|b"Next"' --type rust justpdf-*/src` — 쓰기 쪽(트리를 **만드는** `writer/`, `outline/builder.rs`)은 대상이 아니다. 새 워커는 `tests/tree_cycles.rs`에 조상 순환 케이스와 공유 노드 케이스를 함께 추가한다.

이 불변식이 다루지 않는 것: 순환 없는 극단적 깊이(조상 체인은 깊이를 제한하지 않는다 — 수만 단계의 정상 체인은 여전히 스택을 넘길 수 있다, 추론), 공유 노드의 지수적 재방문(층마다 같은 kid를 두 번 나열하면 k층에서 2^k번 걷는다, 추론) — 이 비용을 보고 메인테이너가 조상 체인 유지를 판단했다(2026-09-29). 렌더러의 콘텐츠 스트림 재귀(`Do` → Form XObject, 패턴 타일, SMask `/G`)도 이 불변식의 대상이 아니다 — 위의 명령에 잡히지 않고, 트리 링크가 아니라 리소스 이름으로 재귀한다(메인테이너 판단, 2026-09-29). `/Parent`를 위로 따라가며 읽는 코드는 2026-09-29 기준 없다(`b"Parent"` 사용처는 모두 쓰기).
