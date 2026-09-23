# ToUnicode CMap

## What it is
폰트의 `/ToUnicode` 스트림에서 `bfchar`/`bfrange` 구간을 파싱해 코드 → 유니코드 매핑을 만든다. 쓰기 쪽(임베드 폰트용 ToUnicode 생성)은 [문서 빌더](document-builder.md)와 [CJK 폰트 임베딩](cjk-font-embedding.md)에 따로 있다.

## Governing decisions
**None.**

## Design model
- 텍스트에서 `beginbfchar`/`beginbfrange`를 스캔한다. `codespacerange`는 파싱하지 않는다.
- ToUnicode 해석이 두 곳에 중복된다: 텍스트 추출의 `resolve_to_unicode`와 렌더러의 `resolve_page_fonts`(SVG 포함). 렌더 쪽 결과는 SVG 장치만 쓴다.

## Code
- `justpdf-core/src/font/cmap.rs` — `ToUnicodeCMap`, `parse`, `lookup`, `parse_bfchar_section`, `parse_bfrange_section`, `hex_to_unicode_string`

## Reference behaviour
**None.** 비교 대상 조항: ISO 32000-2 §9.10.3.

## Cross-cutting invariants
- [폰트 해석 경로](../invariant/font-resolution.md) — 해석 코드 중복.

## Blast radius
- [텍스트 추출](text-extraction.md) — 1순위 매핑.
- [SVG 렌더러](svg-renderer.md) — `<text>` 출력.
- [폰트 서브세팅](font-subsetting.md) — 압축 테스트가 ToUnicode 기반 추출을 판정자로 쓴다.
- [문서 빌더](document-builder.md), [CJK 폰트 임베딩](cjk-font-embedding.md) — 쓰기 쪽 짝. 생성하는 CMap을 이 파서가 되읽을 수 있어야 한다.

## Known holes / open
- `codespacerange` 미파싱 → 가변 길이 코드 CMap을 구분하지 못한다.
