# 압축 — 메타데이터·부가 데이터 제거

## What it is
두 노브. `strip_metadata`는 문서 카탈로그의 XMP·구조 트리·출력 의도·PieceInfo·MarkInfo와 페이지 썸네일 등을 지운다. `strip_extras`는 첨부파일·JavaScript 이름 트리와 페이지 `/AA`를 지운다. 어떤 키를 지우는지는 `strip_non_essential` 본문이 소유한다.

## Governing decisions
**None.**

## Design model
- 구조 트리(`StructTreeRoot`)와 `MarkInfo`를 지우므로 태그드 PDF의 접근성이 사라진다. 이 선택을 정한 기록은 없다.
- `strip_extras`는 `/OpenAction`, 카탈로그 `/AA`, `/Annots`, `/AcroForm`, FileAttachment 주석, `/OCProperties`, `/Outlines`는 건드리지 않는다.

## Code
- `justpdf-core/src/writer/compress.rs` — `strip_non_essential`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [첨부파일](embedded-files.md) — `EmbeddedFiles` 이름 트리 제거. ZUGFeRD 인보이스의 XML이 첨부파일이므로 [ZUGFeRD](zugferd.md) 파일은 이 노브로 망가진다.
- [액션](actions.md) — JavaScript 이름 트리와 `/AA`.
- [프리셋](compress-presets.md) — 어느 프리셋이 어느 노브를 켜는지.

## Known holes / open
- JavaScript를 "지운다"는 약속이 `/OpenAction`·카탈로그 `/AA`·주석 액션의 JS를 포함하지 않는다.
- Tracked: #58 (extreme의 첨부파일 제거)
