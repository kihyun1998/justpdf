# CFF 파서

## What it is
Compact Font Format(FontFile3) 폰트의 헤더·INDEX·Top DICT·charset을 파싱하고 다시 인코딩한다. Type 2 CharString은 해석하지 않는다.

## Governing decisions
**None.**

## Design model
- "does not interpret Type 2 CharString programs", Global Subr INDEX는 건너뛴다.

## Code
- `justpdf-core/src/font/cff.rs` — `parse_cff`, `CffFont`, `CffTopDict`, `CffCharset`

## Reference behaviour
**None.** 코드가 Adobe TN #5176을 인용한다(비교 기록 아님).

## Cross-cutting invariants
**None.**

## Blast radius
- [글리프 렌더링](glyph-rendering.md) — CFF 폰트를 렌더하려면 여기를 써야 하지만 현재는 `ttf_parser`에 원바이트를 넘긴다.
- [폰트 서브세팅](font-subsetting.md) — CFF 서브세팅이 생기면 여기가 재료다.

## Known holes / open
- 제품 코드 소비처가 없다. 렌더러는 순수 CFF(FontFile3)를 `ttf_parser::Face::parse`에 넘기고, 실패하면 자리표시 사각형을 그린다(추론).
- Tracked: #45 (/Differences·CID 폭·CFF)
