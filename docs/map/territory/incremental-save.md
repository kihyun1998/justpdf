# 증분 저장 (incremental save)

## What it is
원본 바이트를 그대로 두고 변경분 객체·새 xref·`/Prev`가 달린 trailer를 파일 끝에 덧붙이는 저장 방식. 서명은 원본 바이트 보존이 필수라 같은 방식을 쓰지만, 서명 쪽은 자기 구현을 따로 가진다([서명](signing.md)).

## Governing decisions
**None.**

## Design model
- `incremental_save`는 모든 객체를 `write!(buf, "{}", obj)`로 쓴다(주석: "Use the serialize module's logic inline"). **스트림이 디버그 형식으로 찍혀 파일이 깨진다** — 연구 에이전트 프로브에서 덧붙인 스트림을 resolve하면 `InvalidToken`.
- `from_document`가 모든 객체를 복사하므로 변경분이 아니라 **모든 객체**를 다시 덧붙인다.
- 새 trailer에는 `/Size /Root /Info /Prev`만 쓴다 — 원본의 `/Encrypt`·`/ID`가 사라진다([증분 trailer](../invariant/incremental-trailer.md)).

## Code
- `justpdf-core/src/writer/modify.rs` — `incremental_save`, `test_incremental_save`

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
- 제품 코드 호출자가 없다(`writer/mod.rs` 재수출뿐). 테스트 `test_incremental_save`는 `contains`만 보고 되읽지 않아 스트림 손상을 잡지 못한다.
- Tracked: #25 (incremental_save 스트림 손상), #26 (증분 trailer 키 소실)
