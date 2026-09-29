# 트리 순회 순환

## The fact
PDF 객체 그래프를 트리로 걷는 코드(`/Kids`, `/First`·`/Next`)는 **조상 체인**을 들고 걷는다: 지금 걷는 노드 위의 참조들. 조상을 다시 만나면 순환이다. 조상이 아닌 노드를 두 번 만나는 것(같은 kid를 두 번 나열한 공유 노드)은 순환이 아니며 두 번 걷는다. 간접 참조만 조상이 될 수 있다 — 직접 dict는 자기를 가리킬 수 없다.

순환을 만났을 때:
- **문서 구조 트리**(페이지 트리, AcroForm 필드 트리, 서명 감지의 필드 트리, 아웃라인) — `JustPdfError::CircularReference`(되돌아간 조상의 번호)로 실패한다.
- **조회 테이블 트리**(페이지 레이블 숫자 트리, 첨부파일 이름 트리, 이름 있는 목적지 이름 트리) — 그 kid만 건너뛰고 나머지를 돌려준다. 루트가 간접 참조면 호출자가 루트를 조상에 먼저 넣는다(루트로 돌아가는 kid가 루트 항목을 한 번 더 내지 않도록).

공유 노드는 순환이 아니지만 방문 수를 곱한다 — 층마다 같은 kid를 두 번 나열하면 k층에서 2^k번 걷는다. 그래서 각 워크는 `VisitBudget`(`justpdf-core/src/tree_walk.rs`)을 들고 걷는다. 방문 하나는 읽은 노드의 **크기**(자기 자신과 직접·중첩으로 담은 배열 원소·사전 값의 수 — 스트림은 사전만)만큼 청구되고, 청구 합이 **지금까지 resolve한 서로 다른 노드 크기 합의 4배**를 넘게 될 방문은 거절된다. 방문 수가 아니라 크기로 세는 것은, 방문 수만 세면 서로 다른 노드 N개가 항목 M개짜리 잎 하나를 세 번씩 나열해 비율 4 안에서 N×M 결과(파일 크기의 제곱)를 내기 때문이다. 진짜 트리는 청구 합이 크기 합과 같아 크기와 상관없이 넘지 않고, 공유 노드로 부푼 워크의 일은 파일이 담은 서로 다른 객체의 크기 합에 비례한다. 이미 resolve한 노드의 재방문은 기록한 크기로 **resolve하기 전에** 판정한다 — resolve(캐시 hit도 복제다)와 크기 계산을 거절마다 하면 거절이 다시 제곱이 된다. 거절된 방문은 청구하지 않는다. 거절됐을 때는 순환과 같은 분할을 따른다: 문서 구조 트리는 `JustPdfError::LimitExceeded`(거절된 노드의 번호), 조회 테이블 트리는 그 kid를 건너뛴다. 조회 테이블 트리의 루트는 세지 않는다 — 한 번만 방문된다. 예산은 `/Count`를 보지 않으므로 거짓 `/Count`로 가지치기를 끈 `get_page`도 멈춘다.

순환의 오류/건너뜀 분할은 **메인테이너의 판단**이다(2026-09-29, #52). 판단 근거로 본 것: MuPDF 원문 — `pdf-page.c`(`pdf_load_page_tree_imp`, "cycle in page tree" throw), `pdf-form.c`(`pdf_lookup_field_imp`, "cycle in fields" throw), `pdf-outline.c`(`pdf_test_outline`, "Cycle detected in outlines" throw), `pdf-nametree.c`(`pdf_lookup_name_imp`·`pdf_load_name_tree_imp`·`pdf_lookup_number_imp`, 순환 가지에서 조용히 중단). 조상 체인 방식은 MuPDF `pdf_cycle`(부모 쪽 연결 리스트)에서 끌어낸 **도출**이다 — MuPDF 아웃라인만 전역 mark bits를 쓴다.

방문 예산을 **비율**로 두는 것도 **메인테이너의 판단**이다(2026-09-29, #120). 판단 근거로 본 것: 실측(release, 2KB 파일, k=22에서 `collect_pages` 9.3초·`parse_acroform` 14.9초·`read_page_labels` 17.2초, 결과 400만 개 — `justpdf::Document::open`이 `collect_pages`를 부른다; 거짓 `/Count`의 `get_page` 9.1초; 아웃라인 k=20 4.8초), ISO 32000-1 읽기(§7.7.3 페이지 노드의 `/Parent`는 하나, §12.7.3.1 "필드는 많아야 하나의 Kids 배열에" — 공유 노드는 비적합), pdf.js가 공유 노드를 throw한다는 것, 그리고 네 선택지(고정 방문 상한, 비율 예산, 전역 visited, 문서화만)와 각각이 빨갛게 만드는 테스트. 고정 상한은 상한보다 큰 정상 문서를 거부하고, 전역 visited는 위의 공유 노드 판단(#52)을 뒤집어 기각됐다. 초과를 새 variant로 알리는 것도 같은 날의 판단이다 — `JustPdfError`가 `#[non_exhaustive]`가 아니라 semver break임을 보고 골랐다. 배수 4는 **도출**이다: 진짜 트리는 1, `*_shared_kid_*` 테스트는 최대 약 1.7, 두 배로 갈라지는 트리는 5층 안에 넘는다. 이 컴퓨터의 PDF 82개(워크 71번)에서는 재방문이 한 번도 없었다(2026-09-29, 최대 비율 1.0).

예산을 방문 수가 아니라 **노드 크기**로 세는 것도 **메인테이너의 판단**이다(2026-09-29, #120). 판단 근거로 본 것: 방문 수 예산 위에서 잰 제곱 — 102KB 파일(노드 1000개가 항목 1000개짜리 잎을 세 번씩)에서 `read_page_labels` 300만 개·11~17초, 195KB 파일(정수 2만 개를 담은 노드를 1만5천 번 나열, 정의되지 않은 참조 5천 개로 예산을 삼)에서 `read_named_destinations` 17.9초 — 와 두 선택지(크기 가중 / 방문 수 예산으로 두고 후속 이슈). 이 판단이 다루지 않은 것: 문자열·스트림 **바이트** 페이로드(`/Contents` 20KB를 방문마다 복사 — 크기는 원소 수라 문자열 하나를 1로 센다), 공유 노드가 없는 정직한 트리에서도 결과마다 하는 복사(상속 `/Resources`, 필드 `full_name`).

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
- (#120 전) 층마다 kid를 두 번 나열한 2KB 파일을 여는 데 수 초가 걸리고 결과가 2^k개였다(위 실측). `get_page`는 `/Count`가 모두 부풀려진 같은 파일에서 2^k 잎을 다 걷고 "not found"를 냈고, 음수 `/Count`는 debug 빌드에서 덧셈 overflow 패닉을 냈다. 예산 이후 같은 파일은 수백 µs 안에 `LimitExceeded`나 잘린 결과를 낸다 — `*_stops_at_visit_limit` 테스트가 잡는다.
- 방문 수로만 세면 위의 102KB 파일이 300만 개를 낸다 — `*_is_charged_by_leaf_size` 테스트가 잡고, `collect_pages_reads_the_wide_shared_leaf_fixture`가 같은 모양이 잎이 작을 때는 예산 안임을 보인다. 크기 가중 이후 102KB 파일은 16ms·4,000개, 195KB 파일은 32ms, 인라인 `/Nums` 2만 쌍을 1만5천 번 나열한 504KB 파일은 425ms·8만 개다(release).
- 이미 본 노드를 resolve한 뒤에 거절하면 거절마다 복제·크기 계산을 해 504KB 파일이 136초 걸린다 — `a_refused_revisit_does_not_resolve_the_node`(`tree_walk.rs` 단위 테스트)가 잡는다.
- 예산을 비율이 아닌 고정 상한으로 바꾸면 큰 정상 트리가 막힌다 — `collect_pages_reads_a_large_unshared_tree`(노드 약 5,100개)가 그보다 낮은 상한을 잡는다. 그 이상의 고정 상한은 잡지 못한다.

## Discovery history
2026-09-23 맵 작성 중 코드 읽기로 발견(#52, 추론). 2026-09-29 여덟 사이트 모두 실행으로 재현하고 고쳤다. 같은 날 #52 작업 중 공유 노드의 지수적 재방문을 추론으로 적었고(#120), 실측으로 재현해 방문 예산을 더했다 — 이슈가 "층 안 visited로 막힌다"고 적어 뺐던 아웃라인도 형제 둘이 `/First`를 공유하면 2^k였다. 방문 수 예산을 검토한 완전성 읽기 두 벌이 따로 같은 제곱(방문마다의 일을 세지 않음)을 찾아 크기 가중으로 바꿨고, 바꾼 첫 판이 거절 경로에서 제곱을 다시 만든 것을 프로브가 잡았다.

## Where it will recur
**`/Kids`·`/First`·`/Next`·`/Parent`를 따라 재귀하거나 반복하는 새 코드는 이 불변식의 대상이다.** 찾는 명령: `rg -n 'b"Kids"|b"First"|b"Next"' --type rust justpdf-*/src` — 쓰기 쪽(트리를 **만드는** `writer/`, `outline/builder.rs`)은 대상이 아니다. 새 워커는 `VisitBudget`을 들고 걷고, `tests/tree_cycles.rs`에 조상 순환 케이스, 공유 노드 케이스, 두 배로 갈라지는 트리 케이스, 큰 잎을 넓게 공유하는 케이스를 함께 추가한다.

이 불변식이 다루지 않는 것: 순환 없는 극단적 깊이(조상 체인은 깊이를 제한하지 않는다 — 수만 단계의 정상 체인은 여전히 스택을 넘길 수 있다, 추론), `get_page`와 `collect_pages`가 같은 파일에 다르게 답하는 경우 — 정직한 `/Count`가 있으면 `get_page`는 공유 서브트리를 가지치기로 건너뛰어 예산을 쓰지 않고 페이지를 찾지만, 같은 파일의 `collect_pages`는 `LimitExceeded`다(#120 판단이 다루지 않은 이웃). 렌더러의 콘텐츠 스트림 재귀(`Do` → Form XObject, 패턴 타일, SMask `/G`)도 이 불변식의 대상이 아니다 — 위의 명령에 잡히지 않고, 트리 링크가 아니라 리소스 이름으로 재귀한다(메인테이너 판단, 2026-09-29). 트리 워커 중 `/Parent`를 위로 따라가며 읽는 것은 2026-09-29 기준 없다(`b"Parent"` 사용처는 모두 쓰기). 객체를 도달 가능성으로 복사하는 코드는 `/Parent`도 따라간다 — `graft_page`가 원본 페이지 트리 전체를 복사한다(Tracked: #121).
