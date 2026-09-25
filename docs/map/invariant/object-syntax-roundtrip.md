# 객체 구문 왕복

## The fact
justpdf가 PDF 구문(객체·이름·문자열·숫자·콘텐츠 연산자)으로 쓰는 모든 바이트는, justpdf 자신의 파서([토크나이저](../territory/tokenizer.md)·[객체 모델](../territory/object-model.md)·[콘텐츠 스트림 파싱](../territory/content-stream-parsing.md))로 되읽었을 때 **쓰기 전과 같은 값**이어야 한다. 검사 형태: `parse(serialize(x)) == x`.

## Why it is cross-cutting
PDF 구문을 쓰는 코드가 한 곳이 아니다. 중심 사이트(`PdfObject`의 Display + `serialize_object`) 외에, 여러 모듈이 **Display를 부르지 않고** 자체 이스케이프 함수나 `write!`로 구문을 손으로 쓴다. 이 사이트들은 서로 호출하지 않으므로, 한 곳을 고쳐도 나머지로 전파되지 않는다. 그래서 어떤 territory→territory 간선으로도 이 사실을 옮길 수 없다.

## Territories it holds in
중심 사이트:
- [객체 직렬화](../territory/object-serialization.md) — Display의 Name/String/Real, `serialize_object`, `write_escaped_name`(이름 이스케이프 세 벌).
- [증분 저장](../territory/incremental-save.md) — 덧붙이는 객체와 trailer를 `serialize_object`/`serialize_dict`로 쓴다(#26 전에는 Display로 써서 스트림이 디버그 형식으로 깨졌다).
- [파일 직렬화](../territory/file-serialization.md) — 모든 객체가 `serialize_object`를 지난다.
- [토크나이저](../territory/tokenizer.md), [객체 모델](../territory/object-model.md) — 판정자(되읽는 쪽). CR 정규화가 여기 있다.

Display를 잘못 쓰는 사이트: 없다. [정리(clean)](../territory/clean.md)는 Display 텍스트를 객체 동일성으로 썼으나(스트림 데이터 누락, `Real(1.0)`=`Integer(1)`), #27에서 값 비교로 바꿨다 — Display는 중복 후보의 버킷 키로만 남았다.

손으로 구문을 쓰는 사이트(도구가 볼 수 있는 절반 — 아래 명령으로 다시 얻는다):
- [콘텐츠 스트림 파싱](../territory/content-stream-parsing.md) — `format_operand`.
- [리댁션](../territory/redaction.md) — `format_operand`로 콘텐츠를 다시 쓴다.
- [compress-grayscale](../territory/compress-grayscale.md) — `write_operand`.
- [서명](../territory/signing.md) — `write_pdf_value`(이스케이프 전무), `escape_pdf_string`.
- [서명 외관](../territory/signature-appearance.md), [폼 외관](../territory/form-appearance.md), [주석 외관](../territory/annotation-appearance.md) — 각자의 `escape_pdf_string`/인라인 이스케이프.
- [문서 빌더](../territory/document-builder.md) — `PageBuilder`의 `set_font`·`draw_image`(이름 미이스케이프), `show_text`.
- [첨부파일](../territory/embedded-files.md) — MIME `#2F` 선인코딩 뒤 이중 이스케이프.

바이트 보존에 기대는 사이트(도구가 볼 수 없는 절반 — 호출이 아니라 가정이다):
- [객체 암호화](../territory/object-encryption.md), [키 유도](../territory/key-derivation.md) — 암호문·`/O`·`/U`가 고바이트를 포함해도 그대로 남아야 한다. #20이 여기서 드러났다.

손으로 쓰는 사이트를 다시 찾는 명령:
`rg -n 'fn escape_pdf_string|fn write_operand|fn format_operand|fn write_pdf_value|write!\(buf, "\{\}", obj\)|format!\("\{\}", obj\)' --glob '*.rs' --glob '!target' .`

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

- `incremental_save`는 #26에서 `serialize_object`로 바꿨다. Tracked: #28 (Display 왕복 구멍(CR·Real·NaN)), #29 (손으로 쓰는 구문 이스케이프)

## Where it will recur
**PDF 구문 바이트를 `PdfObject` Display / `serialize_object` 밖에서 만드는 함수는 이 불변식의 대상이다.** 새로 쓰거나 고칠 때 확인할 것:
- 문자열: 0x20–0x7E와 `\n\t` 밖의 바이트(특히 CR, ≥0x7F)를 literal로 내보내는가? 그러면 hex로.
- 이름: 구분자·공백·`#`·≥0x7F를 `#XX`로 이스케이프하는가? 이미 인코딩된 값을 다시 이스케이프하지 않는가?
- 스트림: `"{}"`로 쓰지 않는가?
- 가능하면 손으로 쓰지 말고 `PdfObject`를 만들어 Display에 맡긴다.
테스트는 쓰기 결과를 **justpdf 파서로 되읽어** 비교해야 한다. `contains`·`starts_with(b"%PDF")` 류 단언은 이 불변식을 검사하지 못한다.
