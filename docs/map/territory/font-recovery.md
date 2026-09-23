# 폰트 대체 (recovery)

## What it is
누락·손상 폰트에 대해 이름에서 굵기·기울임·고정폭·세리프 여부를 추정해 표준 14 대체 폰트를 고른다.

## Governing decisions
**None.**

## Design model
- 기본값은 Helvetica("most commonly used fallback in PDF viewers"). CJK 폰트도 Helvetica로 간다.

## Code
- `justpdf-core/src/font/recovery.rs` — `find_substitute`, `fallback_font_info`, `is_bold_name`, `is_italic_name`, `is_monospace_name`, `is_serif_name`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [글리프 렌더링](glyph-rendering.md) — 대체 폰트를 쓸 쪽(현재는 자리표시 사각형).
- [텍스트 추출](text-extraction.md) — 폭 대체에 쓸 수 있는 쪽.

## Known holes / open
- 텍스트 추출도 렌더도 호출하지 않는다.
