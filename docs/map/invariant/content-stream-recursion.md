# 콘텐츠 스트림 재귀

## The fact
콘텐츠 스트림을 **실행하는** 코드는 지금 실행 중인 스트림들의 참조(조상 체인)를 들고 들어간다: Form XObject(`Do`), 타일링 패턴의 타일, 소프트 마스크 `/G` 폼. 조상을 다시 만나면 그 진입을 조용히 건너뛰고 나머지를 계속 그린다 — 렌더는 오류를 내지 않는다. 조상이 아닌 스트림을 두 번 만나는 것(한 폼을 두 번 그리는 부모)은 순환이 아니며 두 번 그린다. 직접 객체는 조상이 될 수 없다 — 스트림은 간접 객체다.

두 가지 상태는 진입 때 따로 끊는다.
- **장치 색 연산자**(`g`/`rg`/`k`, `G`/`RG`/`K`)는 그쪽 패턴 선택을 지운다 — 색공간이 Device*로 바뀌기 때문이다(ISO 32000-2 §8.6.8). 지우지 않으면 `scn` 뒤의 `rg`가 무시되고, 타일 안의 `rg … f`가 같은 타일로 재귀했다.
- **타일은 페이지의 소프트 마스크 없이 그린다** — 마스크는 페이지 크기이고 타일 픽스맵은 셀 크기다.

소프트 마스크 `/G`가 조상이면 **그 `gs`의 `/SMask` 적용 전체**를 건너뛴다(현재 마스크를 그대로 둔다). 마스크 픽스맵을 만든 뒤 내용 실행만 건너뛰면 빈 마스크(휘도 0)가 설치되어 마스크 폼의 그리기가 모두 가려진다 — `soft_mask_form_applying_its_own_gstate_renders`가 잡는다.

근거(**도출**, 2026-09-29, #119): MuPDF `pdf-op-run.c` — `pdf_run_xobject`가 `pdf_cycle` 조상 목록으로 반복 진입에서 돌아가고, `begin_softmask`가 마스크 폼을 같은 `pdf_run_xobject`로 실행하며, `pdf_show_pattern`이 타일에 들어갈 때 소프트 마스크를 비운다. MuPDF는 컬러 패턴에 들어갈 때 칠하는 쪽의 패턴만 지운다(`pdf_unset_pattern(what)`) — 여기서는 조상 체인이 같은 일을 하므로 따로 지우지 않는다. **This equivalence holds only while the pattern a tile inherits is the object selected outside it**: the selection is the object resolved at `scn`/`SCN` (`PatternSelection`), held in the graphics state as MuPDF holds it, so it is not resolved again by name inside the tile — [resource name scope](resource-scope.md) (2026-09-30, #127). MuPDF와 다른 점: MuPDF는 소프트 마스크를 칠할 때까지 미루므로, 순환하는 `/G`는 빈 마스크로 끝난다. 여기서는 `gs`에서 바로 실행하므로 적용 자체를 건너뛴다. MuPDF는 타일이 자기 패턴을 다시 고르는 경우를 조상 목록이 아니라 gstate 중첩 한도(4096)로 막는다.

두 가지는 **메인테이너의 판단**이다(2026-09-29, #119): 타일 진입 시 패턴 이름을 지우지 않는 것(승인받은 범위에서 벗어난 것 — 보여준 것: MuPDF `pdf_unset_pattern(what)`, 그 지우기를 끄는 mutation이 어떤 테스트도 빨갛게 하지 못한다는 결과, 위의 리소스 조건), 순환하는 `/G`에서 `/SMask` 적용 전체를 건너뛰는 것(보여준 것: MuPDF의 빈 마스크 결과와의 차이, 지연 실행 전환이 범위 밖이라는 것). 이 둘은 메인테이너가 뒤집을 일이다.

## Why it is cross-cutting
콘텐츠 스트림 실행기가 두 벌이고 서로 호출하지 않는다: `RenderInterpreter`(래스터)와 `SvgRenderer`(SVG, 인터프리터의 사본 — Form XObject만 재귀한다). 방어가 없던 시절 두 곳 모두 깊이(>10)만 막았고, 인터프리터의 패턴·소프트 마스크 경로는 깊이도 세지 않았다. 텍스트 추출·폰트 서브세팅이 Form XObject 안으로 들어가게 되면 세 번째·네 번째 실행기가 된다(#48, #68).

## Territories it holds in
- [렌더 인터프리터](../territory/render-interpreter.md) — `with_running_stream`이 조상 체인(`running_streams`)을 관리한다. `do_xobject`가 Form XObject를 이것으로 감싼다.
- [타일링 패턴](../territory/render-tiling-patterns.md) — `render_pattern`이 타일을 감싸고, `render_tiling_pattern`이 소프트 마스크를 비운다. 장치 색 연산자가 패턴 선택을 지운다.
- [투명도](../territory/render-transparency.md) — `apply_soft_mask`가 `render_soft_mask` 전체를 감싼다.
- [SVG 렌더러](../territory/svg-renderer.md) — `do_xobject`가 `running_forms`로 같은 규칙을 따른다.

## What a violation looks like
(#119 전, 2026-09-29 프로브·`tests/render_recursion.rs`로 재현)
- 순환 없는 정상 파일: 페이지 `/Pattern cs /P0 scn … f`, 타일 `1 0 0 rg 0 0 5 5 re f` → 스택 오버플로로 프로세스가 죽는다.
- `/G` 내용이 자기를 설치한 `gs`를 다시 적용 → 스택 오버플로.
- 자기를 부르는 폼 → 깊이 한도까지 11번 그린다. 자기를 k번 부르면 k^11번 실행(k=4에서 16.5초).
- 조상 체인 대신 전역 visited를 쓰면 한 폼을 두 번 그리는 부모에서 두 번째가 사라진다 — `form_drawn_twice_by_its_parent_is_not_a_cycle`가 잡는다. 직계 부모만 보면 두 폼이 서로 부르는 순환을 놓친다 — `form_cycle_through_another_form_stops_at_the_first_repeat`가 잡는다.

## Discovery history
2026-09-29 #52 작업의 완전성 검토 중 발견하고 프로브로 재현(#119). 같은 날 고쳤다.

## Where it will recur
**콘텐츠 스트림 연산자를 실행하면서 `Do`, 패턴 선택(`scn`/`SCN`), `gs`의 `/SMask`를 따라 다른 스트림을 실행하는 새 코드는 이 불변식의 대상이다.** 찾는 명령: `rg -n 'execute_ops\(|parse_content_stream\(' justpdf-*/src` 중 재귀 호출 안에 있는 것. 한 번에 모으고 끝나는 수집기(예: `writer/compress.rs`의 미사용 리소스 탐색)는 전역 `checked` 집합으로 반복하며, 그리는 것이 아니라 모으는 것이라 전역 집합이 맞다 — 대상이 아니다.

이 불변식이 다루지 않는 것: 서로 다른 폼이 층마다 둘씩 부르는 fan-out(순환 없음, 깊이 10까지 2^10) — [트리 순회 순환](tree-traversal-cycles.md)의 공유 노드 재방문과 같은 모양이다. Which `/Resources` a resource name resolves in — [resource name scope](resource-scope.md). The ancestor chain compares object references, so it is independent of that rule. 중첩 스트림 경계의 **다른 상태**도 다루지 않는다(2026-09-29 검토에서 재현): 그래픽 상태 스택에 스트림별 바닥이 없어 짝이 맞지 않는 `q`/`Q`가 경계를 넘어 부모의 상태를 꺼내거나 남기고(MuPDF는 `gbot`으로 막는다), 채우고 남긴 경로(`B`, `b`)가 타일 안으로 들어가며, 소프트 마스크 `/G`·투명도 그룹 폼이 부르는 쪽의 `ca`/`CA`·블렌드·소프트 마스크를 물려받는다(MuPDF `begin_softmask`는 알파 1·Normal·마스크 없음으로 시작한다). Tracked: #128. 서로 다른 패턴·소프트 마스크가 순환 없이 깊게 이어지는 것에는 깊이 제한이 없다 — debug 66단계, release 177단계에서 스택 오버플로(2026-09-29 측정). Tracked: #122
