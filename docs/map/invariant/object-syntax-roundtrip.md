# 객체 구문 왕복

## The fact
justpdf가 PDF 구문(객체·이름·문자열·숫자·콘텐츠 연산자)으로 쓰는 모든 바이트는, justpdf 자신의 파서([토크나이저](../territory/tokenizer.md)·[객체 모델](../territory/object-model.md)·[콘텐츠 스트림 파싱](../territory/content-stream-parsing.md))로 되읽었을 때 **쓰기 전과 같은 값**이어야 한다. 검사 형태: `parse(serialize(x)) == x`.

## Why it is cross-cutting
PDF 구문을 쓰는 코드가 한 곳이 아니다. 규칙은 한 곳에 있다 — `object/types.rs`의 `write_name`·`write_string`·`write_real`(`PdfObject` Display가 이것으로 쓴다)과, 그것으로 콘텐츠 연산자를 쓰는 `ContentOp::write_to`/`write_content`. 그러나 연산자 텍스트를 조립하는 코드는 여러 모듈에 흩어져 있고, 각자 이 함수들을 불러야 규칙을 받는다. `write!(buf, "/{}", name)`처럼 손으로 쓰는 순간 규칙이 빠지며, 그 결함은 어떤 territory→territory 간선으로도 옮겨지지 않는다.

## Territories it holds in
중심 사이트:
- [객체 직렬화](../territory/object-serialization.md) — `write_name`·`write_string`·`write_real`, Display, `serialize_object`, `serialize_dict`.
- [콘텐츠 스트림 파싱](../territory/content-stream-parsing.md) — `ContentOp::write_to`, `Operand::write_to`, `write_content`(콘텐츠 연산자를 다시 쓰는 유일한 경로, #29).
- [증분 저장](../territory/incremental-save.md) — 덧붙이는 객체와 trailer를 `serialize_object`/`serialize_dict`로 쓴다(#26 전에는 Display로 써서 스트림이 디버그 형식으로 깨졌다).
- [파일 직렬화](../territory/file-serialization.md) — 모든 객체가 `serialize_object`를 지난다.
- [토크나이저](../territory/tokenizer.md), [객체 모델](../territory/object-model.md) — 판정자(되읽는 쪽). CR 정규화가 여기 있다. 콘텐츠 스트림 쪽 판정자는 [콘텐츠 스트림 파싱](../territory/content-stream-parsing.md)이고, 같은 CR 정규화를 한다(#87).

Display를 잘못 쓰는 사이트: 없다. [정리(clean)](../territory/clean.md)는 Display 텍스트를 객체 동일성으로 썼으나(스트림 데이터 누락, `Real(1.0)`=`Integer(1)`), #27에서 값 비교로 바꿨다 — Display는 중복 후보의 버킷 키로만 남았다.

공유 함수를 불러 구문을 쓰는 사이트(#29에서 옮겼다):
- [리댁션](../territory/redaction.md), [compress-grayscale](../territory/compress-grayscale.md) — `write_content`/`ContentOp::write_to`.
- [서명](../territory/signing.md) — `/Name`·`/Reason`·`/Location`은 `string_syntax`, `/Rect`는 `real_syntax`, 외관 스트림 사전은 `serialize_dict`.
- [서명 외관](../territory/signature-appearance.md), [폼 외관](../territory/form-appearance.md), [주석 외관](../territory/annotation-appearance.md) — `Tj` 문자열은 `string_syntax`.
- [문서 빌더](../territory/document-builder.md) — `PageBuilder`의 `set_font`·`draw_image`·`draw_inline_image` 이름은 `name_syntax`, `show_text`는 `write_string`.
- [첨부파일](../territory/embedded-files.md) — MIME을 `Name(b"application/pdf")`로 두고 직렬화기에 맡긴다.

아직 손으로 쓰는 것 — 실수: 외관 생성기(폼·주석·서명 외관)와 `PageBuilder`의 좌표·색, 리댁션의 덮개 사각형·색은 `write!("{}", f64)`로 쓴다. NaN·inf가 들어오면 `NaN`·`inf`가 나가고, 정수값 실수는 소수점 없이 나간다. **메인테이너 판단(2026-09-25, #29 read-it)**: #29는 이슈가 명시한 실수(콘텐츠 연산자 재작성, 서명 `/Rect`·외관 사전)만 고치고 이 사이트들(약 38곳)은 제외한다. 제시된 것: 사이트 수, NaN이 그대로 나간다는 사실, 포함하면 `DocumentBuilder` 출력 바이트가 바뀐다는 사실(정수값 실수 → `1.0`). 이 판단은 생성기 실수에 대한 것이고, 생성기의 이름·문자열은 위처럼 옮겼다. Tracked: #90

바이트 보존에 기대는 사이트(도구가 볼 수 없는 절반 — 호출이 아니라 가정이다):
- [객체 암호화](../territory/object-encryption.md), [키 유도](../territory/key-derivation.md) — 암호문·`/O`·`/U`가 고바이트를 포함해도 그대로 남아야 한다. #20이 여기서 드러났다.

손으로 쓰는 사이트를 다시 찾는 명령(나머지는 테스트 메시지·예제·벤치로 걸러 읽는다):
`rg -n '"/\{\}|/\{[a-z_]+\}|\(\{[a-z_]*\}\)|write!\(buf, "\{\}", obj\)|format!\("\{\}", obj\)' --glob '*.rs' --glob '!target' .`

## What a violation looks like
- 한글 폰트 이름 등 공백 있는 Name이 깨져 폰트 파싱 실패 → **한글 텍스트가 라운드트립 후 사라진다**(0be6cc7).
- 특정 비밀번호 조합으로만 암호화 파일이 **어떤 비밀번호로도 열리지 않는다**(#20). 조건부로만 재현된다: 암호문 바이트가 모두 literal 경로(0x20–0x7E)를 타지 않는 비밀번호에서만. 그래서 흔한 테스트 비밀번호(`user123`/`owner456`)로는 보이지 않았다.
- 스트림을 되읽을 때 `InvalidToken … invalid hex digit 0x73`(디버그 형식 `<stream …>`).
- 되읽은 값의 타입이 바뀐다(`Real(1.0)` → `Integer(1)`) 또는 CR이 LF로 바뀐다.

## Discovery history
같은 사실이 세 번 따로 발견·수정됐다:
1. 0be6cc7 — Name의 공백이 `#20`으로 이스케이프되지 않음 → 한글 텍스트 소실. 회귀 테스트 `test_serialize_pdf_with_space_font_roundtrip`, `test_compress_font_name_with_space_roundtrip`, `test_name_display_with_space`.
2. 0be6cc7 — String의 괄호·백슬래시 이스케이프.
3. 7be23c0 / #20 — String 고바이트(≥0x7F)가 UTF-8 다중 바이트로 바뀌어 `/O`·`/U` 손상. 이슈는 처음 키 유도를 의심했고 원인은 직렬화였다.

세 번 모두 중심 사이트(Display)만 고쳤다. 손으로 쓰는 사이트들은 같은 규칙을 적용받지 않았다 — 2026-09-23 맵 작성 중 위 목록의 결함들이 발견됐고, 그중 `incremental_save` 스트림 손상, `clean_objects`의 스트림 병합, Display의 CR·`Real(1.0)`·NaN 왕복 구멍은 같은 날 임시 프로브로 재현했다.

4. #28 — Display가 literal 안의 CR을 날로 쓰고(→ LF), 정수값 실수를 소수점 없이 쓰고(→ `Integer`, i64를 넘으면 파싱 에러), NaN·inf를 그대로 썼다. 원본 `/ID`에 CR이 든 문서를 암호화하면 아무도 열 수 없었다. 이번에는 고정 사례 표 `test_written_objects_read_back_unchanged`를 두어, 지난 세 결함(Name 공백, String 괄호, 고바이트)과 함께 Display·`serialize_object` 두 경로를 되읽어 검사한다.

5. #29 — 손으로 쓰는 사이트들. 리댁션(`format_operand`)은 짝 안 맞는 괄호 문자열(`1)`)을 이스케이프 없이 써 텍스트를 바꿨고, 리댁션·grayscale 둘 다 인라인 이미지를 잃었다(`<inline-image> BI`, `BI `). 공백 든 이름은 두 피연산자로 쪼개졌고, 큰 실수는 다른 값으로 되읽혔다(grayscale은 모든 실수를 소수 4자리로 반올림). 첨부파일 MIME은 이중 이스케이프(`/application#232Fpdf`)였다. 규칙을 공유 함수로 뽑아 모든 사이트가 부르게 했다. 고정 사례 표 `test_written_ops_read_back_unchanged`(콘텐츠 연산자)와 사이트별 되읽기 테스트를 두었다.

- `incremental_save`는 #26에서 `serialize_object`로 바꿨다.

## Where it will recur
**PDF 구문 바이트를 `PdfObject` Display / `serialize_object` 밖에서 만드는 함수는 이 불변식의 대상이다.** 새로 쓰거나 고칠 때 확인할 것:
- 문자열: 0x20–0x7E와 `\n\r\t` 밖의 바이트(특히 ≥0x7F)를 literal로 내보내는가? 그러면 hex로. CR을 literal에 날로 쓰는가? 그러면 `\r`로.
- 실수: 정수값을 소수점 없이 쓰는가(→ `Integer`로 되읽힘, i64를 넘으면 `Real`로 — #84 전에는 파싱 에러)? NaN·inf를 그대로 쓰는가?
- 이름: 구분자·공백·`#`·≥0x7F를 `#XX`로 이스케이프하는가? 이미 인코딩된 값을 다시 이스케이프하지 않는가?
- 스트림: `"{}"`로 쓰지 않는가?
- 가능하면 손으로 쓰지 말고 `PdfObject`를 만들어 Display에 맡기거나, `name_syntax`·`string_syntax`·`real_syntax`·`ContentOp::write_to`를 부른다.
테스트는 쓰기 결과를 **justpdf 파서로 되읽어** 비교해야 한다. `contains`·`starts_with(b"%PDF")` 류 단언은 이 불변식을 검사하지 못한다.
- 판정자는 표준대로 읽어야 한다. #87 전의 콘텐츠 파서는 literal 안의 CR을 그대로 두어, 콘텐츠 스트림 사이트에서 CR 이스케이프가 빠져도 되읽은 값이 같았다 — 되읽기 테스트가 볼 수 없는 규칙이 있었다. 판정자가 쓰는 쪽과 같은 모델을 공유하면 되읽기는 그 모델의 결함을 볼 수 없다.
