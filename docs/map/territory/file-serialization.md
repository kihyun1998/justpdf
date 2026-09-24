# 파일 직렬화 (헤더·xref·trailer)

## What it is
객체 목록을 완전한 PDF 파일로 쓴다: 헤더, 바이너리 마커, `N 0 obj` 본문, 고전 xref 테이블(또는 `/Type /XRef` 스트림), trailer(`/Size /Root /Info`, 암호화 시 `/Encrypt /ID`). `PdfWriter`는 객체 번호 할당·보관을 맡는다.

## Governing decisions
**None.**

## Design model
- 세대 번호는 항상 0으로 쓴다. 암호화 사전 자체는 암호화하지 않는다 — 암호화 제외는 **객체 번호**로만 판정하므로(`encrypt_obj_num`), `/Encrypt` 사전의 번호가 다른 객체와 겹치면 그 객체가 평문으로 나간다.
- `PdfWriter::set_object`가 새 번호를 넣으면 `next_obj_num`을 그 위로 올린다. 번호가 다시 할당되면 위의 번호 판정 때문에, 암호화 저장에서 `/Encrypt` 사전과 번호가 겹친 객체가 평문으로 나간다.
- xref 스트림 경로는 버전을 최소 1.5로 올리며, **암호화 경로가 없다**.
- `write_xref_stream`의 `w2`는 본문 최대 오프셋으로 정하는데 xref 스트림 자신의 오프셋이 더 클 수 있다(추론: 잠재 결함, `startxref`가 스트림을 직접 가리켜 현재는 파싱됨).
- `serialize_pdf_impl`에 쓰이지 않는 `_extra_trailer` 매개변수가 있다 — trailer 키를 추가할 통로가 비어 있다.

## Code
- `justpdf-core/src/writer/serialize.rs` — `serialize_pdf`, `serialize_pdf_encrypted`, `serialize_writer_encrypted`, `serialize_pdf_impl`, `serialize_pdf_with_xref_stream`
- `justpdf-core/src/writer/object_stream.rs` — `write_xref_stream`, `bytes_needed`, `write_field`
- `justpdf-core/src/writer/mod.rs` — `PdfWriter`, `add_object`, `alloc_object_num`, `set_object`, `write_to_bytes`, `test_set_object_at_a_new_number_is_not_reused`

## Reference behaviour
**None.** 비교 대상 조항: ISO 32000-2 §7.5(파일 구조).

## Cross-cutting invariants
- [객체 구문 왕복](../invariant/object-syntax-roundtrip.md) — 모든 객체가 `serialize_object`를 거쳐 여기서 파일이 된다.

## Blast radius
- [xref](xref.md) — 여기서 쓴 xref·trailer를 읽는 쪽.
- [객체 직렬화](object-serialization.md) — 객체 단위 출력.
- [객체 암호화](object-encryption.md) — `serialize_pdf_encrypted`가 객체마다 `encrypt_object`를 부른다.
- [문서 빌더](document-builder.md), [문서 수정기](document-modifier.md) — `build`가 이 함수들로 끝난다. 암호화 저장은 둘 다 `serialize_writer_encrypted`(`/Encrypt` 객체 추가, `/ID [영구 변하는]`)를 거친다. xref 스트림 경로(`serialize_pdf_with_xref_stream`)에는 암호화가 없다.
- [object streams](object-streams.md), [압축 파이프라인](compress-pipeline.md) — xref 스트림 경로의 유일한 소비처(현재 꺼짐).

## Known holes / open
- xref 스트림 + 암호화 조합은 쓸 수 없다.
