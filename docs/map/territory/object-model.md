# 객체 모델

## What it is
PDF 값 트리(`PdfObject`, 정렬된 `PdfDict`, `IndirectRef`)와, 토큰에서 직접 객체·`N M obj … endobj` 래퍼·스트림 본문을 만드는 재귀 파서다. 거의 모든 크레이트가 이 타입으로 PDF를 다룬다(`lib.rs`에서 재수출). 같은 타입의 `Display` 구현은 쓰기 쪽 직렬화기이며, 그 부분은 [객체 직렬화](object-serialization.md)가 다룬다.

## Governing decisions
**None.** [ADR-0001](../../adr/0001-crates-split-by-dependency-layer.md)은 "객체모델"이 core 레이어에 있다고만 적는다.

## Design model
코드에서 읽어낸 규칙이다.
- 정수 뒤에 정수와 `R`이 오면 참조, 아니면 `seek`로 되감는다.
- 스트림 `/Length`는 직접 `Integer`일 때만 쓴다. 간접 참조면 `endstream`을 스캔하는 쪽으로 떨어진다.
- `endstream`/`endobj` 누락은 허용(되감기).
- `PdfDict` getter는 참조를 따라가지 않는다. 참조 해석은 [문서 접근](document-access.md)의 `resolve` 몫이다. 이 경계 때문에 간접 배열로 된 페이지 박스 같은 값이 조용히 무시된다([페이지 트리](page-tree.md)).

## Code
- `justpdf-core/src/object/types.rs` — `PdfObject`, `PdfDict`, `IndirectRef`, `get_i64`, `get_ref`, `get_array`, `get_dict`, `get_name`
- `justpdf-core/src/object/mod.rs` — `parse_object`, `parse_indirect_object`, `parse_dict_body`, `read_stream_data`, `find_stream_data_by_endstream`

## Reference behaviour
**None.** 비교 대상 조항: ISO 32000-2 §7.3(객체), §7.3.8(스트림) — 기억에 의한 포인터, 원문 대조 안 함.

## Cross-cutting invariants
- [객체 구문 왕복](../invariant/object-syntax-roundtrip.md) — 이 파서가 쓰기 쪽 출력의 판정자다.

## Blast radius
- [토크나이저](tokenizer.md) — 파서가 토큰 되감기에 의존한다.
- [객체 직렬화](object-serialization.md) — `PdfObject` 변형을 추가·변경하면 `Display`도 같이 바뀌어야 한다.
- [문서 접근](document-access.md) — 모든 객체 로드가 `parse_indirect_object`를 지난다.
- [object streams](object-streams.md) — 압축 객체도 `parse_object`로 읽는다.
- `PdfObject`/`PdfDict`의 공개 API 변경은 모든 소비 크레이트에 닿는다. 소비처 목록은 명령으로 얻는다: `rg -l 'PdfObject|PdfDict' --glob '*.rs' --glob '!target' .`.

## Known holes / open
- 간접 `/Length`는 항상 `endstream` 스캔으로 처리되므로, 스트림 데이터 안에 `endstream` 바이트열이 있으면 잘린다(추론).
