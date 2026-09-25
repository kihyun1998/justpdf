# 콘텐츠 스트림 파싱

## What it is
페이지·Form XObject·외관 스트림의 콘텐츠를 연산자 목록(`ContentOp { operator, operands }`)으로 파싱한다. 인라인 이미지는 `Operand::InlineImage`를 가진 `BI` 연산 하나가 된다. 렌더러·텍스트 추출·리댁션·압축이 모두 이 결과를 입력으로 쓴다. `ContentOp::write_to`/`Operand::write_to`/`write_content`(`pub(crate)`)는 연산자를 다시 바이트로 쓰는 쪽이다. **메인테이너 판단(2026-09-25, #29 check-it)**: 공개하지 않는다 — 제시된 사실: 호출자가 크레이트 안뿐이고, 역연산은 파서가 만든 연산에만 보장된다(아래). 나중에 공개하는 것은 비파괴적이다.

## Governing decisions
**None.**

## Design model
- 파일 구조 [토크나이저](tokenizer.md)와 별개의 렉서다(문자 분류 함수만 공유).
- 이름의 `#XX`를 디코드한다. EI는 앞에 공백이 있어야 한다. 모르는 바이트는 건너뛴다.
- 인라인 이미지 사전의 값은 `read_inline_dict`·`read_array`와 같이 읽는다 — `<<`는 사전, `null`은 `Null`. #29 전에는 `<<`를 hex 문자열로 읽어 `/DP << /Predictor 15 … >>`가 쓰레기 문자열이 되었다(`test_inline_image_dict_values_read_like_other_dicts`, 일반·arena 두 파서).
- **다시 쓰는 쪽은 이 파서가 만든 연산의 역연산이다**: 이 파서가 만든 `ContentOp` 목록을 `write_content`로 쓰고 되읽으면 같은 목록이다(`test_written_ops_read_back_unchanged`, #29). 손으로 만든 연산은 그렇지 않을 수 있다 — 인라인 이미지 데이터에 공백+`EI`+공백/구분자가 들어 있으면 파서가 거기서 이미지를 끊는다(이 파서가 만든 데이터에는 그 패턴이 없다). `BI`가 아닌 연산자에 인라인 이미지 피연산자를 넣으면 이미지 뒤에 그 연산자를 쓴다. 이름·문자열·실수는 `PdfObject` Display와 같은 함수(`write_name`·`write_string`·`write_real`)로 쓴다. 인라인 이미지는 `BI <dict> ID <data> EI`로 데이터를 바이트 그대로 쓴다 — `ID` 뒤 공백 하나와 `EI` 앞 공백 하나를 파서가 떼어 내므로 그 둘을 쓴다. 사전(`Operand::Dict`)은 `PdfDict`(`BTreeMap`)로 바꾸지 않고 쓴다 — 바꾸면 `BDC` 속성 사전의 키 순서가 바뀐다 — [객체 구문 왕복](../invariant/object-syntax-roundtrip.md).
- `ContentOp`의 Display는 `write_to`의 결과를 보여 준다. `String`이라 인라인 이미지의 UTF-8 아닌 데이터는 손실되어 보이므로, 구문을 쓰는 쪽은 Display가 아니라 `write_to`를 쓴다.
- literal 문자열 안의 이스케이프 없는 CR·CRLF를 LF로 읽는다 — 토크나이저와 같고, ISO 32000-1 §7.3.4.2("An end-of-line marker appearing within a literal string without a preceding REVERSE SOLIDUS shall be treated as a byte value of (0Ah)")를 따른다(`test_literal_line_ends_read_as_line_feed`, 일반·arena 두 파서, #87). #87 전에는 CR을 그대로 두어, 이 파서로 되읽는 테스트가 쓰는 쪽의 CR 이스케이프 누락을 볼 수 없었다.
- i64를 넘는 정수 텍스트는 `Integer(0)`이 된다(`parse().unwrap_or(0)`, 토크나이저는 같은 텍스트를 에러로 낸다 — #84).
- arena 파서(`parse_content_stream_arena`, feature `arena`)는 core 밖에서 호출되지 않는다.

## Code
- `justpdf-core/src/content/mod.rs` — `parse_content_stream`, `parse_content_stream_arena`, `ContentParser`, `next_op`, `read_name`, `read_inline_image`
- `justpdf-core/src/content/operator.rs` — `Operand`, `ContentOp`, `ContentOp::write_to`, `Operand::write_to`, `write_content`

## Reference behaviour
**None.** 비교 대상 조항: ISO 32000-2 §7.8.2, Annex A(연산자 요약).

## Cross-cutting invariants
- [객체 구문 왕복](../invariant/object-syntax-roundtrip.md) — `write_content`.
- [페이지 콘텐츠 조립](../invariant/page-content-assembly.md) — 파서 입력을 만드는 방식이 소비처마다 따로 있다.

## Blast radius
- [렌더 인터프리터](render-interpreter.md), [SVG 렌더러](svg-renderer.md), [bbox 장치](bbox-device.md) — `ContentOp` 소비처.
- [텍스트 추출](text-extraction.md) — 같은 연산자 스트림.
- [리댁션](redaction.md) — 파싱 후 `write_content`로 다시 쓴다.
- [compress-grayscale](compress-grayscale.md) — 파싱 후 `ContentOp::write_to`로 다시 쓴다.
- [compress-images](compress-images.md), [compress-unused-resources](compress-unused-resources.md) — CTM·리소스 사용 수집.
- `Operand` 변형을 추가하면 `Operand::write_to`를 고친다(`match`가 빠진 변형을 컴파일 에러로 알린다).

## Known holes / open
- Tracked: #84 (i64를 넘는 정수 텍스트)

