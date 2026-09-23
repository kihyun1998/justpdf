# CJK 폰트 임베딩 (쓰기)

## What it is
TrueType 폰트를 CID 폰트(Type0 + CIDFontType2)로 감싸 문서에 임베드하는 쓰기 쪽 코드. 언어 순서(ordering) 감지, `/W` 배열 생성, ToUnicode CMap 생성을 포함한다.

## Governing decisions
**None.**

## Design model
- `/CIDToGIDMap /Identity`를 쓴다.
- 모든 글리프에 기본 폭 1000을 쓴다("For now, use a default width of 1000 for all glyphs").
- bfchar 구간을 100개씩 끊는다(코드 주석: "PDF spec limit").

## Code
- `justpdf-core/src/font/cjk.rs` — `build_cid_font`, `detect_ordering`, `CJKOrdering`, `generate_to_unicode_cmap`, `build_w_array`

## Reference behaviour
**None.**

## Cross-cutting invariants
- [콘텐츠 텍스트 인코딩](../invariant/content-text-encoding.md) — 비 ASCII 텍스트를 올바르게 쓸 수 있는 유일한 재료가 여기 있지만 연결되어 있지 않다.

## Blast radius
- [CID 폰트](cid-fonts.md), [ToUnicode](tounicode.md) — 여기서 만든 폰트를 읽는 쪽.
- [문서 빌더](document-builder.md) — 연결되어야 할 쪽(`show_text`는 표준 폰트만 전제).

## Known holes / open
- `cjk.rs` 밖에 호출처가 없다. formats·OCR 등 비 ASCII 텍스트를 쓰는 모든 경로가 이 모듈 대신 표준 Type1 폰트를 쓴다.
- Tracked: #34 (비 ASCII 콘텐츠 텍스트)
