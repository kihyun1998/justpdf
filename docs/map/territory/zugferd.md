# ZUGFeRD (전자 인보이스 읽기)

## What it is
PDF 첨부파일에서 ZUGFeRD/Factur-X XML을 찾아 프로파일을 판별하고 인보이스 정보를 파싱한다. 읽기 전용(생성 없음).

## Governing decisions
- [ADR-0001](../../adr/0001-crates-split-by-dependency-layer.md)

## Design model
- core [첨부파일](embedded-files.md)의 `read_embedded_files`·`extract_file`로 XML을 꺼낸다.

## Code
- `justpdf-special/src/zugferd/mod.rs` — `is_zugferd`, `extract_zugferd`, `parse_zugferd_xml`, `detect_profile`, `ZugferdProfile`, `ZugferdInfo`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [첨부파일](embedded-files.md) — 입력 경로.
- [압축 제거](compress-stripping.md) — extreme 프리셋이 첨부파일 이름 트리를 지워 ZUGFeRD 파일을 인보이스가 아니게 만든다.

## Known holes / open
- 실제 ZUGFeRD 파일로 찾는 테스트가 없다.
- Tracked: #58 (extreme의 첨부파일 제거)
