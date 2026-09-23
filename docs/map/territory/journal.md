# 저널 (undo/redo)

## What it is
객체 추가·수정·삭제·배치 연산의 undo/redo 스택과, `JRNL` 매직으로 시작하는 자체 바이너리 직렬화 포맷. "역연산을 실제로 적용하는 것은 호출자 책임"이다.

## Governing decisions
**None.**

## Design model
- PDF 구문이 아닌 길이 접두 바이너리 포맷이므로 객체 왕복에 안전하다.

## Code
- `justpdf-core/src/journal.rs` — `Journal`, `Operation`, `BatchBuilder`, `inverse`, `to_bytes`, `from_bytes`, `serialize_pdf_object`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [객체 모델](object-model.md) — `PdfObject` 변형이 늘면 `serialize_pdf_object`도 늘려야 한다.

## Known holes / open
- `lib.rs`의 `pub mod journal` 선언 외에 소비처가 없다. [문서 수정기](document-modifier.md)와 연결되지 않았다.
