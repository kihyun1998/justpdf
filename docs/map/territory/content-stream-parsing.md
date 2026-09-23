# 콘텐츠 스트림 파싱

## What it is
페이지·Form XObject·외관 스트림의 콘텐츠를 연산자 목록(`ContentOp { operator, operands }`)으로 파싱한다. 인라인 이미지는 `Operand::InlineImage`를 가진 `BI` 연산 하나가 된다. 렌더러·텍스트 추출·리댁션·압축이 모두 이 결과를 입력으로 쓴다. `ContentOp`의 Display(`format_operand`)는 연산자를 다시 텍스트로 쓰는 쪽이다.

## Governing decisions
**None.**

## Design model
- 파일 구조 [토크나이저](tokenizer.md)와 별개의 렉서다(문자 분류 함수만 공유).
- 이름의 `#XX`를 디코드한다. EI는 앞에 공백이 있어야 한다. 모르는 바이트는 건너뛴다.
- **다시 쓰는 쪽은 역연산이 아니다**: `format_operand`는 이름을 `#XX`로 다시 이스케이프하지 않고(UTF-8이 아니면 `/?`), UTF-8로 읽히는 문자열은 이스케이프 없이 literal로 쓰며, 인라인 이미지는 `<inline-image>`라는 텍스트가 된다 — [객체 구문 왕복](../invariant/object-syntax-roundtrip.md).
- arena 파서(`parse_content_stream_arena`, feature `arena`)는 core 밖에서 호출되지 않는다.

## Code
- `justpdf-core/src/content/mod.rs` — `parse_content_stream`, `parse_content_stream_arena`, `ContentParser`, `next_op`, `read_name`, `read_inline_image`
- `justpdf-core/src/content/operator.rs` — `Operand`, `ContentOp`, `format_operand`

## Reference behaviour
**None.** 비교 대상 조항: ISO 32000-2 §7.8.2, Annex A(연산자 요약).

## Cross-cutting invariants
- [객체 구문 왕복](../invariant/object-syntax-roundtrip.md) — `format_operand`.
- [페이지 콘텐츠 조립](../invariant/page-content-assembly.md) — 파서 입력을 만드는 방식이 소비처마다 따로 있다.

## Blast radius
- [렌더 인터프리터](render-interpreter.md), [SVG 렌더러](svg-renderer.md), [bbox 장치](bbox-device.md) — `ContentOp` 소비처.
- [텍스트 추출](text-extraction.md) — 같은 연산자 스트림.
- [리댁션](redaction.md) — 파싱 후 `format_operand`로 다시 쓴다.
- [compress-grayscale](compress-grayscale.md) — 파싱 후 자체 `write_operand`로 다시 쓴다.
- [compress-images](compress-images.md), [compress-unused-resources](compress-unused-resources.md) — CTM·리소스 사용 수집.
- `Operand` 변형을 추가하면 `format_operand`와 `write_operand` 두 사본을 모두 고친다.

## Known holes / open
- 파싱 → 다시 쓰기 왕복 테스트가 없다.
- Tracked: #29 (손으로 쓰는 구문 이스케이프)
