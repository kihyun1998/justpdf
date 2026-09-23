# FB2 입력

## What it is
FictionBook 2 XML의 섹션 텍스트를 PDF로 바꾼다.

## Governing decisions
- [ADR-0001](../../adr/0001-crates-split-by-dependency-layer.md) — `fb2`는 미리보기 때문에 render를 끌어온다(#2).

## Design model
- 넘치는 텍스트를 버린다.

## Code
- `justpdf-formats/src/fb2/mod.rs` — `Fb2Document`, `to_pdf`, `render_page`

## Reference behaviour
**None.**

## Cross-cutting invariants
- [콘텐츠 텍스트 인코딩](../invariant/content-text-encoding.md)

## Blast radius
- [CLI](cli.md) — `convert`에 FB2 분기가 없다.
- [렌더 API](render-api.md).

## Known holes / open
- CLI에서 도달 불가.
- Tracked: #49 (convert MOBI·FB2)
