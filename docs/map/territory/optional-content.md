# Optional content (레이어, OCG)

## What it is
`/OCProperties`의 레이어(OCG)와 멤버십 사전(OCMD)을 읽고 기본 구성에서 가시성을 판정하며, 레이어를 추가·표시 변경·제거한다. 렌더러가 BDC 구간을 건너뛸지 결정하는 데 쓴다.

## Governing decisions
**None.**

## Design model
- `/VE` 가시성 표현식, `/RBGroups`, `/Locked`, `/AS`는 처리하지 않는다. Usage·Intent는 파싱되지만 가시성에 영향이 없다.
- `add_ocg`는 그룹만 만들고 콘텐츠를 태그하지 않는다.
- **렌더러에서 레이어는 사실상 항상 보인다**: 흔한 형태(`/OC /Name BDC`)는 "TODO: look up in page /Resources /Properties dict"로 항상 보임, 인라인 `/Type /OCG`도 보임, 인라인 OCMD만 판정된다. XObject `/OC`와 주석 `/OC`는 확인하지 않는다.
- SVG 렌더러는 BDC/EMC를 무시하고, 텍스트 추출은 OC를 보지 않는다.

## Code
- `justpdf-core/src/ocg/parse.rs` — `read_oc_properties`, `is_ocg_visible`, `is_ocmd_visible`, `parse_ocmd`
- `justpdf-core/src/ocg/builder.rs` — `add_ocg`, `set_ocg_visibility`, `remove_ocg`
- `justpdf-core/src/ocg/types.rs` — `OCConfig`
- `justpdf-render/src/interpreter.rs` — `check_oc_visibility`, `check_oc_dict_visibility`

## Reference behaviour
**None.** 비교 대상 조항: ISO 32000-2 §8.11.

## Cross-cutting invariants
**None.**

## Blast radius
- [렌더 인터프리터](render-interpreter.md) — 판정 호출 지점(`/Properties` 조회를 넣을 곳).
- [SVG 렌더러](svg-renderer.md), [텍스트 추출](text-extraction.md) — OC 무시.
- [렌더 주석](render-annotations.md) — 주석 `/OC` 미확인.
- [compress-unused-resources](compress-unused-resources.md) — `/Properties`는 정리 대상이 아니다.

## Known holes / open
- `/Properties` 이름 조회는 렌더러 TODO로 남아 있다.
- Tracked: #44 (OCG 항상 보임)
