# 증분 저장 (incremental save)

## What it is
원본 바이트를 그대로 두고 변경분 객체·새 xref·`/Prev`가 달린 trailer를 파일 끝에 덧붙이는 저장 방식. 서명은 원본 바이트 보존이 필수라 같은 방식을 쓰지만, 서명 쪽은 자기 구현을 따로 가진다([서명](signing.md)).

## Governing decisions
**None.**

## Design model
- 덧붙이는 객체는 `serialize_object`로 쓴다(#26 전에는 Display라 스트림이 디버그 형식으로 깨졌다 — #25의 직렬화 부분).
- `from_document`가 모든 객체를 복사하므로 변경분이 아니라 **모든 객체**를 다시 덧붙인다.
- 새 trailer는 `incremental_trailer`로 만든다 — 원본 trailer의 키를 옮긴다([증분 trailer](../invariant/incremental-trailer.md)).
- 원본이 암호화되어 있으면 `DocumentModifier`가 가진 `SecurityState`의 파일 키로 덧붙이는 객체를 `encrypt_object`한다(`/Encrypt` 객체는 건너뜀). 인증되지 않은 문서에서 만든 modifier면 에러다.
  - **메인테이너 판단(2026-09-23, #26)**: 증분 저장은 암호화 입력을 지원하고 서명은 거부한다. 대안이었던 "둘 다 거부"와 "둘 다 지원(서명에 비밀번호 인자 추가)"이 함께 제시되었다. #25는 직렬화만 포함하고 "모든 객체를 다시 덧붙임"은 #25에 남긴다(메인테이너 판단).
- `DocumentModifier`는 세대 번호를 버린다 — 덧붙이는 객체는 모두 `N 0 obj`이고, 암호화 키도 세대 0으로 유도한다. 원본에 세대가 0이 아닌 객체가 있으면 참조(`N g R`)와 어긋난다(추론).

## Code
- `justpdf-core/src/writer/modify.rs` — `incremental_save`, `incremental_trailer`, `DocumentModifier`, `test_incremental_save_keeps_encryption`, `test_incremental_save_reopens_with_intact_objects`, `test_incremental_save_after_xref_stream_section`

## Reference behaviour
**None.** 비교 대상 조항: ISO 32000-2 §7.5.6.

## Cross-cutting invariants
- [객체 구문 왕복](../invariant/object-syntax-roundtrip.md) — 스트림을 Display로 쓰는 사이트.
- [증분 trailer](../invariant/incremental-trailer.md) — 쓰기 쪽 사이트 둘 중 하나.

## Blast radius
- [xref](xref.md) — 덧붙인 구간을 읽는 쪽.
- [서명](signing.md) — 같은 방식의 두 번째 구현. 한쪽을 고치면 다른 쪽도 같은 결함이 있는지 본다.
- [객체 직렬화](object-serialization.md) — 올바른 경로는 `serialize_object`다.
- [리댁션](redaction.md) — 증분 저장은 설계상 원본 바이트(지운 텍스트 포함)를 남긴다.

## Known holes / open
- 제품 코드 호출자가 없다(`writer/mod.rs` 재수출뿐).
- 변경분이 아니라 모든 객체를 다시 덧붙인다. Tracked: #25
- 세대 번호가 0이 아닌 객체(위). Tracked: #72
