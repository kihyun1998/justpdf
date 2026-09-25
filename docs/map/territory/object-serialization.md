# 객체 직렬화

## What it is
`PdfObject` 값을 PDF 구문 바이트로 바꾸는 쪽. `impl Display for PdfObject`가 스트림을 뺀 모든 객체를 쓰고, `serialize_object`가 스트림만 직접 처리한다(`/Length`를 `data.len()`으로 세팅). 파일 하나를 만드는 모든 경로 — 생성, 수정, 압축, 암호화, 서명 — 의 최하층이다.

## Governing decisions
**None.**

## Design model
- **이름**: 구분자·공백·`#`·`<= 0x20`·`>= 0x7F` 바이트를 `#XX`로 이스케이프한다. 이 규칙이 세 벌 있다: Display의 `Name`, Display의 `Dict` 키, `write_escaped_name`. `serialize_dict`는 Display `Dict`의 두 번째 사본이다.
- **문자열**: 모든 바이트가 0x20–0x7E 또는 `\n\r\t`일 때만 literal `(…)`, 아니면 hex. literal에서는 `(`, `)`, `\`를 이스케이프하고 CR은 `\r`로 쓴다(#28). [토크나이저](tokenizer.md)가 literal 안의 줄바꿈(CR, CRLF)을 LF로 바꾸는 것은 ISO 32000 §7.3.4.2가 요구하는 동작이라, 날 CR을 쓰면 LF로 되읽힌다 — #28 전에는 원본 `/ID` 첫 원소에 CR이 든 문서를 암호화하면 되읽은 `/ID[0]`이 달라 R3/R4 파일 키가 어긋나 justpdf·qpdf 모두 열지 못했다. hex 규칙이 있는 이유는 literal 경로의 `b as char`가 고바이트를 UTF-8 다중 바이트로 바꿔 /O·/U 같은 바이너리 문자열을 깨뜨렸기 때문이다(주석이 이 이유를 적고 있다).
- **실수**: 항상 소수점을 붙여 쓴다 — 정수값이어도 `1.0`, `-0.0`, `100000000000000000000.0`(#28). 정수값이 아닌 실수는 Rust `f64` Display(가장 짧은 왕복 자릿수, 지수 표기 없음) 그대로다. NaN은 `0.0`, ±Inf는 ±`f32::MAX`로 쓴다 — PDF로 표현할 수 없고 Display는 실패할 수 없다. NaN은 계산 결과에서만 생기지만 Inf는 파일에서도 들어온다 — 토크나이저가 실수 텍스트를 `f64`로 읽으므로 `f64` 범위(약 1.8e308)를 넘는 실수(`1` 뒤에 0이 400개 `.0` 등)는 `inf`가 되고, 다시 쓰면 `f32::MAX`가 된다(assay·lens 프로브, 2026-09-25).
  - #28 전: `Real(1.0)` → `1` → `Integer(1)`, `Real(-0.0)` → `Integer(0)`, `Real(1e20)` → 정수 텍스트가 i64를 넘어 **되읽기가 파싱 에러**, NaN·inf → `NaN`·`inf`(유효한 PDF 아님).
  - **메인테이너 판단(2026-09-25, #28 triage)**: 항상 소수점, NaN·Inf는 바꿔 쓰기. 제시된 대안: MuPDF처럼 i64 범위 안의 정수값은 정수로 쓰기(ISO 32000 §7.3.3상 무해하나 되읽으면 `Integer`가 되어 값 비교에서 다른 값), NaN·Inf는 `serialize_object` 에러. 판단 근거로 제시된 사실: 위 재현, MuPDF `pdf-object.c` `fmt_obj`(정수값 실수를 `%d`로)와 `printf.c` `fmtfloat`("NaN to 0, +Inf to FLT_MAX, -Inf to -FLT_MAX").
- **스트림**: Display는 스트림을 PDF가 아닌 디버그 형식(`<stream dict=… len=N>`)으로 찍는다. 스트림은 반드시 `serialize_object`로 써야 하며, `"{}"`로 쓰면 파일이 깨진다.

## Code
- `justpdf-core/src/object/types.rs` — `PdfObject`, `test_string_display_high_byte_uses_hex`, `test_name_display_with_space`
- `justpdf-core/src/writer/serialize.rs` — `serialize_object`, `serialize_dict`, `write_escaped_name`, `test_written_objects_read_back_unchanged`, `test_non_finite_reals_are_written_as_finite_ones`, `test_serialize_pdf_with_space_font_roundtrip`, `test_serialize_string_with_parens_roundtrip`
- `justpdf-core/src/writer/modify.rs` — `test_build_with_encryption_keeps_a_permanent_id_that_holds_a_carriage_return`

## Reference behaviour
MuPDF `source/pdf/pdf-object.c` `fmt_obj`(정수값 실수를 정수로 쓴다 — justpdf는 여기서 다르게 간다, 위 판단), `source/fitz/printf.c` `fmtfloat`(NaN·Inf 처리 — 같게 간다). 비교 대상 조항: ISO 32000-2 §7.3.4(문자열), §7.3.5(이름), §7.3.3(숫자). 원문 대조(2026-09-25)는 ISO 32000-1 §7.3.4.2(literal 안 줄바꿈은 0Ah로 읽는다)와 §7.3.3(실수는 소수점과 함께, 지수 표기 금지 — NOTE 1; 실수 자리에 정수 허용 — NOTE 2)으로 했다.

## Cross-cutting invariants
- [객체 구문 왕복](../invariant/object-syntax-roundtrip.md) — 이 노트가 불변식의 중심 사이트다.
- [텍스트 문자열 인코딩](../invariant/text-string-encoding.md) — 직렬화는 바이트를 보존할 뿐, Rust `&str`을 PDF 텍스트 문자열로 바꾸는 인코더는 이 층에 없다.

## Blast radius
- [토크나이저](tokenizer.md), [객체 모델](object-model.md) — 쓰기 규칙을 바꾸면 읽기 쪽이 그대로 되읽는지 확인한다.
- [파일 직렬화](file-serialization.md), [증분 저장](incremental-save.md), [정리(clean)](clean.md) — Display를 직접 쓰는 호출자. 특히 스트림을 `"{}"`로 쓰는 곳.
- [객체 암호화](object-encryption.md) — 암호문 문자열이 hex 경로를 타는 데 기댄다.
- [압축 전체](compress.md) — 압축 결과 파일 전체가 이 직렬화를 거친다.

## Known holes / open
- 사전·배열 안에 중첩된 스트림은 디버그 형식으로 찍힌다.
- 범위 밖 실수: f32 범위를 넘는 유한 값(1e300 → 301자리)과 아주 작은 값(5e-324 → 326자)을 그대로 쓴다(ISO 32000-1 Annex C.2는 ±3.403e38 안을 권한다). MuPDF·Acrobat는 정수부가 10자리 이상인 실수를 32비트로 잘라 읽으므로, ±Inf를 쓴 `f32::MAX`와 `1e20`도 다른 값이 된다(MuPDF `pdf-lex.c` `acrobat_compatible_atof`, 계산). ±Inf→±`f32::MAX` 판단은 이 사실을 보지 못한 채 내려졌고, 메인테이너가 #28은 그대로 두고 따로 정하기로 했다(2026-09-25). Tracked: #85
- 이름 이스케이프 규칙이 세 벌이다(Display `Name`, Display `Dict` 키, `write_escaped_name`). 앞의 두 벌은 `test_written_objects_read_back_unchanged`가 잡고, `write_escaped_name`(스트림 사전)은 자기 테스트만 있다.
