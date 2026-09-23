# Object streams (ObjStm)

## What it is
`/Type /ObjStm` 스트림에 작은 비스트림 객체들을 묶는 포맷. 읽기 쪽은 xref 타입 2 엔트리를 해석해 스트림을 복호화·디코드·캐시하고 `/First + offset`에서 객체를 파싱한다. 쓰기 쪽은 적격 객체를 `num off … 데이터` 레이아웃으로 묶고 타입 2 엔트리 정보를 돌려준다. 한 포맷을 두 사이트가 서로 호출 없이 공유한다.

## Governing decisions
**None.**

## Design model
- **`/First`가 권위다.** 인덱스 쌍을 다 읽은 뒤의 토크나이저 위치로 데이터 시작을 추론하면 공백 패딩만큼 어긋난다 — `load_compressed_object`의 주석이 이 규칙을 적고 있고, 쓰기 쪽 `build_object_stream`이 바로 그 패딩(데이터 앞 공백)을 낸다.
- 쓰기 쪽 적격 규칙: 스트림, catalog, pages 루트, 암호화 사전, `/Type /XRef`, Null은 묶지 않는다.
- 읽기 쪽은 쌍의 객체 번호를 무시하고 인덱스만 쓴다. 스트림은 항상 세대 0으로 조회한다.
- **암호화**: 읽기 쪽은 ObjStm 전체를 복호화한 뒤, 꺼낸 객체에 대해 [객체 복호화](object-decryption.md)가 한 번 더 돈다(추론: 이중 복호화). 쓰기 쪽 패킹은 암호화를 받지 않는다.
- 쓰기 쪽 패킹은 [압축 파이프라인](compress-pipeline.md)에서 꺼져 있다(`pack_into_object_streams`는 dead code).

## Code
- `justpdf-core/src/parser.rs` — `load_compressed_object`, `decoded_obj_streams`, `test_objstm_first_padding_regression`
- `justpdf-core/src/writer/object_stream.rs` — `pack_object_streams`, `is_eligible`, `build_object_stream`, `PackResult`, `CompressedObjInfo`
- `justpdf-core/src/writer/compress.rs` — `pack_into_object_streams`

## Reference behaviour
**None.** 비교 대상 조항: ISO 32000-2 §7.5.7(object streams; 안의 문자열은 따로 암호화되지 않는다는 규정 포함) — 기억에 의한 포인터.

## Cross-cutting invariants
**None.** `/First` 규칙은 두 사이트가 공유하지만 한 포맷에 속한 설계 규칙이므로 여기 Design model에 둔다.

## Blast radius
- [xref](xref.md) — 타입 2 엔트리의 출처. xref 스트림 쓰기(`write_xref_stream`)와 짝을 이룬다.
- [객체 복호화](object-decryption.md) — 압축 객체의 복호화 경로(이중 복호화 의심).
- [압축 파이프라인](compress-pipeline.md) — 패킹을 다시 켜는 곳. 켜면 뷰어 호환성 문제(주석 사유)를 다시 검증해야 한다.
- [파일 직렬화](file-serialization.md) — `serialize_pdf_with_xref_stream`이 패킹 결과를 받는다.
- [문서 접근](document-access.md) — 디코드된 스트림 캐시를 소유한다.

## Known holes / open
- 암호화 + object stream 파일을 여는 테스트가 없다. 이중 복호화 의심을 판정할 첫 테스트가 여기서 시작된다.
- 패킹은 구현돼 있지만 압축 파이프라인에서 꺼져 있다(CHANGELOG 0.1.3 항목과 설계 문서가 이 상태를 적고 있다).
- Tracked: #32 (ObjStm 이중 복호화)
