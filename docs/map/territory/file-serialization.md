# 파일 직렬화 (헤더·xref·trailer)

## What it is
객체 목록을 완전한 PDF 파일로 쓴다: 헤더, 바이너리 마커, `N g obj` 본문, 고전 xref 테이블(또는 `/Type /XRef` 스트림), trailer(`/Size /Root /Info`, 암호화 시 `/Encrypt /ID`). `PdfWriter`는 객체 번호 할당·보관을 맡는다.

## Governing decisions
**None.**

## Design model
- 세대 번호: 공개 함수(`serialize_pdf`, `serialize_pdf_encrypted`, `serialize_pdf_with_xref_stream`, `write_xref_stream`)는 번호·객체 슬라이스만 받으므로 모든 객체를 세대 0으로 쓴다. `PdfWriter`를 받는 crate 내부 함수(`serialize_writer`, `serialize_writer_with_state`, `serialize_writer_with_xref_stream`, `write_xref_stream_with_generations`)는 writer의 `generations` 세대로 헤더·xref 항목·암호화 키를 쓴다 — 공개 시그니처를 바꾸지 않으려고 두 벌이다(#72, [원본 세대](../invariant/source-generation.md)). `PdfWriter::write_to_bytes`도 writer 쪽이다. writer 쪽 함수는 catalog·info를 참조가 아니라 **번호**로 받고, trailer `/Root`·`/Info`를 writer가 그 객체를 쓰는 세대로 쓴다(`reference_to`, #110) — 공개 `write_to_bytes(catalog_ref)`는 `catalog_ref`의 세대를 쓰지 않는다. `serialize_writer_with_xref_stream`은 세대 ≠ 0인 압축 객체·object stream을 `InvalidObject`로 거부한다([object streams](object-streams.md), #106).
- xref 스트림의 세 번째 필드 폭(`w3`)은 인덱스·255·최대 세대 중 큰 값에 맞춘다. 세대만 빼고 맞추면 세대 300이 1바이트에 44로 잘린다(`test_build_with_xref_stream_keeps_a_generation_wider_than_a_byte`).
- 암호화 사전 자체는 암호화하지 않는다 — 암호화 제외는 **객체 번호**로만 판정하므로(`encrypt_obj_num`), `/Encrypt` 사전의 번호가 다른 객체와 겹치면 그 객체가 평문으로 나간다.
- `PdfWriter::set_object`가 새 번호를 넣으면 `next_obj_num`을 그 위로 올린다. 번호가 다시 할당되면 위의 번호 판정 때문에, 암호화 저장에서 `/Encrypt` 사전과 번호가 겹친 객체가 평문으로 나간다.
- xref 스트림 경로는 버전을 최소 1.5로 올리며, **암호화 경로가 없다**.
- `write_xref_stream`의 `w2`는 본문 최대 오프셋으로 정하는데 xref 스트림 자신의 오프셋이 더 클 수 있다(추론: 잠재 결함, `startxref`가 스트림을 직접 가리켜 현재는 파싱됨).
- `serialize_pdf_impl`에 쓰이지 않는 `_extra_trailer` 매개변수가 있다 — trailer 키를 추가할 통로가 비어 있다.

## Code
- `justpdf-core/src/writer/serialize.rs` — `serialize_pdf`, `serialize_pdf_encrypted`, `serialize_writer_encrypted`, `serialize_pdf_impl`, `serialize_pdf_with_xref_stream`, `serialize_writer`, `serialize_writer_with_state`, `serialize_writer_with_xref_stream`, `serialize_with_xref_stream_impl`
- `justpdf-core/src/writer/object_stream.rs` — `write_xref_stream`, `write_xref_stream_with_generations`, `bytes_needed`, `write_field`
- `justpdf-core/src/writer/mod.rs` — `PdfWriter`, `add_object`, `alloc_object_num`, `set_object`, `write_to_bytes`, `test_set_object_at_a_new_number_is_not_reused`, `generation`, `generation_of`, `reference_to`

## Reference behaviour
**None.** 비교 대상 조항: ISO 32000-2 §7.5(파일 구조).

## Cross-cutting invariants
- [객체 구문 왕복](../invariant/object-syntax-roundtrip.md) — 모든 객체가 `serialize_object`를 거쳐 여기서 파일이 된다.
- [xref 항목 형식](../invariant/xref-entry-format.md) — `serialize_pdf_impl`의 고전 xref 테이블.
- [원본 세대](../invariant/source-generation.md) — writer 쪽 함수는 원본 세대로, 공개 슬라이스 함수는 0으로 쓴다.

## Blast radius
- [xref](xref.md) — 여기서 쓴 xref·trailer를 읽는 쪽.
- [객체 직렬화](object-serialization.md) — 객체 단위 출력.
- [객체 암호화](object-encryption.md) — `serialize_pdf_encrypted`가 객체마다 `encrypt_object`를 부른다.
- [문서 빌더](document-builder.md), [문서 수정기](document-modifier.md) — `build`가 이 함수들로 끝난다. 암호화 저장은 둘 다 `serialize_writer_encrypted`(`/Encrypt` 객체 추가, `/ID [영구 변하는]`)를 거친다. xref 스트림 경로(`serialize_pdf_with_xref_stream`)에는 암호화가 없다.
- [object streams](object-streams.md), [압축 파이프라인](compress-pipeline.md) — xref 스트림 경로의 유일한 소비처(현재 꺼짐).

## Known holes / open
- xref 스트림 + 암호화 조합은 쓸 수 없다.
