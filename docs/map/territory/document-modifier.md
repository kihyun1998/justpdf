# 문서 수정기 (DocumentModifier)

## What it is
기존 문서의 모든 객체를 `PdfWriter`로 풀어 놓고(`from_document`), 페이지 삭제·삽입·재정렬, Info 설정, 병합, 가비지 컬렉션을 한 뒤 전체를 다시 쓴다. 주석·폼·OCG·아웃라인·페이지 레이블·첨부파일·압축 등 "기존 파일을 바꾸는" 거의 모든 기능이 이 위에서 돈다.

## Governing decisions
**None.**

## Design model
- `from_document`는 resolve에 실패한 객체를 조용히 건너뛴다(`if let Ok`). 암호화 문서를 인증 없이 넣으면 대부분 빈 결과가 된다(추론).
- `build`는 GC를 하지 않는다. GC는 `garbage_collect`를 따로 불러야 한다 — 그래서 [리댁션](redaction.md)이 걷어낸 옛 콘텐츠 스트림이 파일에 남는다.
- 원본 세대 번호는 버려지고 모두 `0 obj`로 쓴다(추론).
- `set_info`는 [텍스트 문자열 인코딩](../invariant/text-string-encoding.md) 문제를 [문서 빌더](document-builder.md)와 공유한다.
- 병합(`merge_documents`)은 `graft_page`/`deep_copy_object`로 페이지와 의존 객체를 복사하고 리소스 이름 충돌을 처리한다.

## Code
- `justpdf-core/src/writer/modify.rs` — `DocumentModifier`, `from_document`, `delete_page`, `insert_page`, `reorder_pages`, `set_info`, `garbage_collect`, `build`, `build_with_xref_stream`, `merge_documents`, `graft_page`, `deep_copy_object`

## Reference behaviour
**None.**

## Cross-cutting invariants
- [텍스트 문자열 인코딩](../invariant/text-string-encoding.md) — `set_info`.

## Blast radius
- [파일 직렬화](file-serialization.md) — `build`의 출력 경로.
- [페이지 트리](page-tree.md) — 페이지 조작이 트리를 다시 쓴다.
- 이 수정기 위에 선 기능들 — [주석](annotations.md), [리댁션](redaction.md), [폼 채우기](form-fill.md), [폼 평탄화](form-flatten.md), [optional content](optional-content.md), [아웃라인](outlines.md), [페이지 레이블](page-labels.md), [첨부파일](embedded-files.md), [압축 파이프라인](compress-pipeline.md). `from_document`/`build` 의미를 바꾸면 모두 확인한다.
- [CLI](cli.md) — split/encrypt/decrypt/clean/merge. [파사드](facade.md) — `Modifier` 래퍼.

## Known holes / open
- resolve 실패 객체를 경고 없이 버린다.
- Tracked: #33 (텍스트 문자열 인코딩)
