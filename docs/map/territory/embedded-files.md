# 첨부파일 (embedded files)

## What it is
`/Names /EmbeddedFiles` 이름 트리의 첨부파일을 읽고(MD5 확인 포함) 추가한다. ZUGFeRD 인보이스 XML이 이 경로로 읽힌다.

## Governing decisions
**None.**

## Design model
- 저장소에서 PDF 텍스트 문자열을 올바르게 디코드하는 몇 안 되는 곳이다(`obj_to_string`, UTF-16BE BOM 처리). 다른 읽기 코드는 이것을 재사용하지 않는다.
- 추가 시 이름 트리 루트 `/Names`에 정렬 없이 덧붙인다(루트에 `/Kids`가 있어도 — 추론).
- MIME 타입은 `Name(b"application/pdf")`로 두고, `/`의 `#2F` 이스케이프는 직렬화기에 맡긴다(#29). 읽을 때 디코드한 이름에 남은 `#2F`를 `/`로 바꾼다 — #29 전의 justpdf는 `/application#232Fpdf`(이중 이스케이프)로 썼고, 그 파일은 `application#2Fpdf`로 디코드되기 때문이다(`test_mime_type_written_escaped_twice_still_reads`) — [객체 구문 왕복](../invariant/object-syntax-roundtrip.md).
- `/F`·`/UF`를 UTF-8 그대로 쓴다 — [텍스트 문자열 인코딩](../invariant/text-string-encoding.md).

## Code
- `justpdf-core/src/embedded_file.rs` — `read_embedded_files`, `extract_file`, `add_embedded_file`, `wire_into_name_tree`, `obj_to_string`

## Reference behaviour
**None.** 비교 대상 조항: ISO 32000-2 §7.11.4, §7.9.6(이름 트리).

## Cross-cutting invariants
- [텍스트 문자열 인코딩](../invariant/text-string-encoding.md)
- [객체 구문 왕복](../invariant/object-syntax-roundtrip.md)

## Blast radius
- [ZUGFeRD](zugferd.md) — 읽기 경로의 유일한 외부 소비처.
- [압축 제거](compress-stripping.md) — extreme에서 이름 트리를 통째로 지운다.
- [스트림 필터](stream-filters.md) — 파일 스트림 디코드.
- [파사드](facade.md) — `embedded_files`.

## Known holes / open
- 추출 왕복 테스트가 없다(추가 후 MIME 되읽기만 있다: `test_mime_type_is_written_once_escaped`).
- Tracked: #33 (텍스트 문자열 인코딩), #58 (extreme의 첨부파일 제거)
