# CID 폰트 읽기 (Type0)

## What it is
Type0 복합 폰트와 하위 CIDFont를 읽는 경로: 2바이트 코드 분할, `/W`·`/DW` 폭, `/CIDToGIDMap`. 텍스트 추출과 렌더러가 각자 구현한다.

## Governing decisions
**None.**

## Design model
- 코드 폭은 두 소비처 모두 **2바이트로 하드코딩**한다: 텍스트는 `Identity || Subtype == Type0`, 렌더는 `Subtype == Type0`. 가변 폭 CMap(예: 1/2바이트 혼합)은 지원하지 않는다(추론).
- `/W` 파싱은 텍스트 쪽(`parse_cid_widths`, `resolve_type0_descendant`)에만 있다. `resolve_type0_descendant`는 인코딩을 `Identity`로 강제한다.
- `/CIDToGIDMap` 해석은 렌더 쪽(`parse_cid_to_gid_map`, `parse_cid_gid_stream`)과 압축 서브세팅(`cid_font_glyph_ids`, [폰트 서브세팅](font-subsetting.md))에 따로 있다. 텍스트 추출은 필요 없다.

## Code
- `justpdf-core/src/text/mod.rs` — `parse_cid_widths`, `resolve_type0_descendant`, `show_string`
- `justpdf-render/src/interpreter.rs` — `parse_cid_to_gid_map`, `parse_cid_gid_stream`, `render_text_string`

## Reference behaviour
**None.** 비교 대상 조항: ISO 32000-2 §9.7.

## Cross-cutting invariants
- [폰트 해석 경로](../invariant/font-resolution.md) — 이 노트가 불일치의 대표 사례(폭은 텍스트에만, GID 매핑은 렌더에만).

## Blast radius
- [텍스트 추출](text-extraction.md), [글리프 렌더링](glyph-rendering.md) — 두 구현.
- [폰트 로딩](font-loading.md) — CID 폭을 채우지 않는 쪽.
- [폰트 서브세팅](font-subsetting.md) — CID 폰트의 `/CIDToGIDMap`/`/W` 갱신.
- [CJK 폰트 임베딩](cjk-font-embedding.md) — 쓰기 쪽이 만드는 CID 폰트를 이 경로가 읽는다.

## Known holes / open
- 한 폰트의 폭 정보가 렌더에 전달되지 않는다(렌더는 1000 고정 폭으로 전진 — 추론).
- Tracked: #45 (/Differences·CID 폭·CFF)
