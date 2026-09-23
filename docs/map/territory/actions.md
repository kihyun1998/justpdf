# 액션

## What it is
액션 사전(GoTo, URI, Launch, Named, JavaScript 등)을 모델로 파싱하고 체인을 따라가며, 모델에서 액션 사전을 만든다.

## Governing decisions
**None.**

## Design model
- 문서 없이 `&PdfDict`만 받으므로 간접 참조를 해석할 수 없다. `/Next` 참조를 따라가지 못한다. `/F` 파일 스펙 사전은 무시한다.
- JavaScript `/JS` 스트림을 디코드 없이 원 `data`로 읽는다(추론).
- 목적지 타입은 [아웃라인](outlines.md)의 `Destination`을 재수출한다.

## Code
- `justpdf-core/src/action/types.rs` — `PdfAction`, `NamedAction`
- `justpdf-core/src/action/parse.rs` — `parse_action`, `parse_action_chain`
- `justpdf-core/src/action/builder.rs` — `build_action`

## Reference behaviour
**None.** 비교 대상 조항: ISO 32000-2 §12.6.

## Cross-cutting invariants
- [텍스트 문자열 인코딩](../invariant/text-string-encoding.md) — 빌더가 URI·JS 등 Rust 문자열을 BOM 없는 문자열로 쓴다.

## Blast radius
- [주석](annotations.md), [아웃라인](outlines.md) — `/A`를 이 모듈 없이 손으로 파싱한다. 액션 해석을 고치려면 세 곳을 본다.
- [압축 제거](compress-stripping.md) — JavaScript 제거가 이 모델을 쓰지 않는다.

## Known holes / open
- 모듈 밖 소비처가 없다.
- Tracked: #33 (텍스트 문자열 인코딩)
