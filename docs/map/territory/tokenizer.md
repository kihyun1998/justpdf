# 토크나이저

## What it is
PDF 파일 본문(객체 구문)을 바이트 단위로 읽어 토큰(숫자, 문자열, 이름, 구분자, 키워드)으로 쪼갠다. 공백·주석을 건너뛰고, literal/hex 문자열과 이름의 이스케이프를 해석한다. 파일 구조 계층([객체 모델](object-model.md), [xref](xref.md), [repair](repair.md), [linearization](linearization.md))의 공통 입구이며, 콘텐츠 스트림은 이 토크나이저를 쓰지 않는다.

## Governing decisions
**None.** [ADR-0001](../../adr/0001-crates-split-by-dependency-layer.md)은 이 코드가 `justpdf-core`에 속한다는 것만 정한다. 토큰화 규칙 자체를 정한 기록은 없다.

## Design model
코드에서 읽어낸 규칙이다(상위 설계 문서 없음).
- 알려진 키워드 집합 밖의 단어는 에러다. 그래서 콘텐츠 스트림 연산자는 이 토크나이저로 읽을 수 없고, [콘텐츠 스트림 파싱](content-stream-parsing.md)은 문자 분류 함수만 빌려 쓰는 별도 렉서를 가진다.
- 홀수 자리 hex 문자열은 뒤에 0을 붙인다.
- literal 문자열 안의 줄바꿈(CR, CRLF)은 `\n`으로 정규화된다. **쓰기 쪽이 CR을 literal에 그대로 쓰면 왕복이 깨지는 지점이 바로 여기다** — [객체 구문 왕복](../invariant/object-syntax-roundtrip.md)의 읽기 쪽 절반.
- 알 수 없는 이스케이프는 백슬래시만 버린다.

## Code
- `justpdf-core/src/tokenizer/mod.rs` — `Tokenizer`, `next_token`, `seek`, `read_literal_string`, `read_hex_string`, `read_name`, `classify_keyword`
- `justpdf-core/src/tokenizer/token.rs` — `Token`, `Keyword`
- `justpdf-core/src/tokenizer/reader.rs` — `PdfReader`, `is_pdf_whitespace`, `is_pdf_delimiter`, `is_pdf_regular`

## Reference behaviour
**None.** 참조와 비교한 기록이 없다. 비교 대상 조항: ISO 32000-2 §7.2(어휘 규칙), §7.3.4(문자열), §7.3.5(이름) — 기억에 의한 조항 포인터이며 원문 대조는 하지 않았다.

## Cross-cutting invariants
- [객체 구문 왕복](../invariant/object-syntax-roundtrip.md) — 쓰기 쪽이 내는 모든 구문은 이 토크나이저로 다시 읽혀 같은 값이 되어야 한다.

## Blast radius
- [객체 모델](object-model.md) — 토큰 모양이 바뀌면 `parse_object`의 참조(`N M R`) 되감기 로직이 영향받는다.
- [객체 직렬화](object-serialization.md) — 문자열·이름 해석 규칙을 바꾸면 쓰기 쪽 이스케이프 규칙도 함께 맞춰야 한다.
- [xref](xref.md), [repair](repair.md), [linearization](linearization.md) — 모두 이 토크나이저로 파일 구조를 읽는다.
- [콘텐츠 스트림 파싱](content-stream-parsing.md) — `reader`의 문자 분류 함수를 공유한다.

## Known holes / open
- literal 문자열의 CR 정규화 때문에, 쓰기 쪽이 CR을 hex로 보내지 않으면 값이 바뀐다(현재 쓰기 쪽은 CR을 literal로 보낸다 — [객체 직렬화](object-serialization.md#known-holes--open)).
- Tracked: #28 (Display 왕복 구멍(CR·Real·NaN))
