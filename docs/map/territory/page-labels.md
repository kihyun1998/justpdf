# 페이지 레이블

## What it is
`/PageLabels` 숫자 트리를 읽어 페이지 인덱스별 표시 레이블(로마 숫자·알파벳·접두사)을 계산하고, 레이블을 설정한다.

## Governing decisions
**None.**

## Design model
- 숫자 트리 파서에 순환 방어가 없다. 접두사는 손실 디코드된다 — [텍스트 문자열 인코딩](../invariant/text-string-encoding.md).
- 설정 시 `/Nums`를 정렬한다.

## Code
- `justpdf-core/src/page_label.rs` — `read_page_labels`, `label_for_page`, `to_roman`, `to_alpha`, `set_page_labels`, `parse_number_tree`

## Reference behaviour
**None.** 비교 대상 조항: ISO 32000-2 §12.4.2.

## Cross-cutting invariants
- [텍스트 문자열 인코딩](../invariant/text-string-encoding.md)

## Blast radius
- [페이지 트리](page-tree.md) — 인덱스 기준.
- [문서 수정기](document-modifier.md) — 페이지 삽입·삭제 후 레이블이 어긋나는지 확인한 기록 없음.
- [파사드](facade.md) — `page_labels`.

## Known holes / open
- Tracked: #33 (텍스트 문자열 인코딩), #52 (순환 방어)
