# 텍스트 검색

## What it is
페이지 텍스트에서 정확 일치·대소문자 무시·단순 정규식 검색을 하고, 일치 위치를 사각형(quad)으로 돌려준다.

## Governing decisions
**None.**

## Design model
- 정규식 크레이트 없이 자체 `SimpleRegex`를 쓴다. 지원하지 않는 패턴은 빈 결과를 돌려준다(에러가 아님).

## Code
- `justpdf-core/src/text/search.rs` — `search_page`, `search_exact`, `search_case_insensitive`, `search_regex`, `SimpleRegex`, `compute_quad`, `TextQuad`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [텍스트 추출](text-extraction.md) — 입력(글리프 위치).
- [파사드](facade.md) — `search`, `search_case_insensitive`의 유일한 소비처.

## Known holes / open
- 지원하지 않는 정규식이 조용히 빈 결과가 된다.
