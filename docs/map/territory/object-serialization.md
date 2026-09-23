# 객체 직렬화

## What it is
`PdfObject` 값을 PDF 구문 바이트로 바꾸는 쪽. `impl Display for PdfObject`가 스트림을 뺀 모든 객체를 쓰고, `serialize_object`가 스트림만 직접 처리한다(`/Length`를 `data.len()`으로 세팅). 파일 하나를 만드는 모든 경로 — 생성, 수정, 압축, 암호화, 서명 — 의 최하층이다.

## Governing decisions
**None.**

## Design model
- **이름**: 구분자·공백·`#`·`<= 0x20`·`>= 0x7F` 바이트를 `#XX`로 이스케이프한다. 이 규칙이 세 벌 있다: Display의 `Name`, Display의 `Dict` 키, `write_escaped_name`. `serialize_dict`는 Display `Dict`의 두 번째 사본이다.
- **문자열**: 모든 바이트가 0x20–0x7E 또는 `\n\r\t`일 때만 literal `(…)`, 아니면 hex. literal에서는 `(`, `)`, `\`만 이스케이프한다. hex 규칙이 있는 이유는 literal 경로의 `b as char`가 고바이트를 UTF-8 다중 바이트로 바꿔 /O·/U 같은 바이너리 문자열을 깨뜨렸기 때문이다(주석이 이 이유를 적고 있다).
- **스트림**: Display는 스트림을 PDF가 아닌 디버그 형식(`<stream dict=… len=N>`)으로 찍는다. 스트림은 반드시 `serialize_object`로 써야 하며, `"{}"`로 쓰면 파일이 깨진다.

## Code
- `justpdf-core/src/object/types.rs` — `PdfObject`, `test_string_display_high_byte_uses_hex`, `test_name_display_with_space`
- `justpdf-core/src/writer/serialize.rs` — `serialize_object`, `serialize_dict`, `write_escaped_name`, `test_serialize_pdf_with_space_font_roundtrip`, `test_serialize_string_with_parens_roundtrip`

## Reference behaviour
**None.** 비교 대상 조항: ISO 32000-2 §7.3.4(문자열), §7.3.5(이름), §7.3.3(숫자).

## Cross-cutting invariants
- [객체 구문 왕복](../invariant/object-syntax-roundtrip.md) — 이 노트가 불변식의 중심 사이트다.
- [텍스트 문자열 인코딩](../invariant/text-string-encoding.md) — 직렬화는 바이트를 보존할 뿐, Rust `&str`을 PDF 텍스트 문자열로 바꾸는 인코더는 이 층에 없다.

## Blast radius
- [토크나이저](tokenizer.md), [객체 모델](object-model.md) — 쓰기 규칙을 바꾸면 읽기 쪽이 그대로 되읽는지 확인한다.
- [파일 직렬화](file-serialization.md), [증분 저장](incremental-save.md), [정리(clean)](clean.md) — Display를 직접 쓰는 호출자. 특히 스트림을 `"{}"`로 쓰는 곳.
- [객체 암호화](object-encryption.md) — 암호문 문자열이 hex 경로를 타는 데 기댄다.
- [압축 전체](compress.md) — 압축 결과 파일 전체가 이 직렬화를 거친다.

## Known holes / open
프로브로 확인된 왕복 구멍(연구 에이전트가 scratchpad 프로브 크레이트로 재현, 저장소에는 테스트 없음):
- literal 문자열의 CR: `a\rb\r\nc` → 되읽으면 `a\nb\nc`([토크나이저](tokenizer.md)의 줄바꿈 정규화).
- 정수값 실수: `Real(1.0)` → `1` → 되읽으면 `Integer(1)`.
- `Real(NaN)` → 리터럴 텍스트 `NaN`(유효한 PDF 아님).
- 사전·배열 안에 중첩된 스트림은 디버그 형식으로 찍힌다.
- Tracked: #28 (Display 왕복 구멍(CR·Real·NaN))
