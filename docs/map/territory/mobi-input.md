# MOBI 입력

## What it is
Mobipocket 파일(PalmDOC 압축 또는 무압축)에서 텍스트를 읽는다.

## Governing decisions
- [ADR-0001](../../adr/0001-crates-split-by-dependency-layer.md) — `mobi` 기능은 미리보기 때문에 render를 끌어온다(#2).

## Design model
- 다른 압축 방식은 에러.

## Code
- `justpdf-formats/src/mobi/mod.rs` — `MobiDocument`, `to_pdf`, `render_page`

## Reference behaviour
**None.**

## Cross-cutting invariants
- [콘텐츠 텍스트 인코딩](../invariant/content-text-encoding.md)

## Blast radius
- [CLI](cli.md) — `convert`에 MOBI 분기가 없다.
- [렌더 API](render-api.md) — 미리보기.

## Known holes / open
- CLI에서 도달 불가(위).
- Tracked: #49 (convert MOBI·FB2)
