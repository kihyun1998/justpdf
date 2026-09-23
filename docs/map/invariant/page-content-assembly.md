# 페이지 콘텐츠 조립

## The fact
페이지의 `/Contents`는 스트림 하나 또는 스트림 배열이다. 배열이면 각 스트림을 디코드한 뒤 **사이에 공백을 넣어 이어 붙인** 하나의 콘텐츠로 해석해야 한다(토큰이 스트림 경계에 걸칠 수 있다). 페이지 콘텐츠를 읽는 모든 곳이 같은 방식으로 조립해야 같은 연산자 목록을 얻는다.

## Why it is cross-cutting
조립 함수가 소비처마다 따로 있다: 렌더 인터프리터, SVG 렌더러, bbox 장치, 텍스트 추출(private), 리댁션. 서로 호출하지 않는다. 한 곳에서 간접 배열·빈 스트림·디코드 실패 처리를 고쳐도 나머지는 그대로다.

## Territories it holds in
- [렌더 인터프리터](../territory/render-interpreter.md) — `get_page_content`, `concat_content_streams`.
- [SVG 렌더러](../territory/svg-renderer.md) — `get_page_content`, `concat_content_streams`.
- [bbox 장치](../territory/bbox-device.md) — `get_page_content`("simplified version"), `concat_streams`.
- [텍스트 추출](../territory/text-extraction.md) — `get_page_content_data`(private).
- [리댁션](../territory/redaction.md) — `get_page_content_data`.
- [콘텐츠 스트림 파싱](../territory/content-stream-parsing.md) — 조립된 바이트를 받는 쪽.

다시 찾는 명령: `rg -n 'fn get_page_content|fn concat_(content_)?streams' --glob '*.rs' --glob '!target' .`

## What a violation looks like
같은 페이지에서 렌더에는 보이는 내용이 텍스트 추출이나 리댁션에는 없거나(또는 그 반대), 스트림 경계에 걸친 연산자 하나가 한쪽에서만 깨진다. 여러 스트림으로 된 `/Contents`에서만 재현된다.

## Discovery history
기록된 사고가 없다. 2026-09-23 맵 작성 중 렌더·대화형 연구 에이전트가 사본들을 보고했다. 각 사본의 경계 처리 차이는 아직 대조되지 않았다.

## Where it will recur
**페이지(또는 Form XObject)의 콘텐츠를 연산자로 읽는 새 기능은 이 불변식의 대상이다.** 새 조립 함수를 쓰기 전에 위 명령으로 기존 것을 찾는다. 조립이 core 공개 함수 하나로 모이면 이 노트는 그 함수를 가리키는 한 줄로 줄어든다.
