# xref와 증분 업데이트 체인

## What it is
`startxref`를 찾아 `/Prev` 체인을 따라가며, 각 구간(고전 xref 테이블 또는 xref 스트림)을 하나의 `Xref { entries, trailer }`로 병합한다. 객체 번호 → 파일 오프셋(또는 object stream 안 위치)의 유일한 출처다.

## Governing decisions
**None.**

## Design model
코드에서 읽어낸 규칙이다.
- **최신 구간이 이긴다**: 이전 구간의 엔트리는 이미 있는 번호를 덮어쓰지 않는다(`or_insert`).
- **trailer는 최신 것 하나만 남는다.** 이전 구간 trailer의 키는 병합되지 않는다. 증분 쓰기가 `/Encrypt`·`/ID`를 새 trailer에 다시 적지 않으면 읽기 쪽에서 사라진다 — [증분 trailer](../invariant/incremental-trailer.md).
- `/Prev` 순환은 `visited`로 끊는다. `startxref`는 파일 끝 1024바이트에서만 찾는다.
- xref 스트림: `/W[0] == 0`이면 타입 1, 모르는 타입은 건너뜀. 쓰기 쪽 `write_xref_stream`이 내는 레이아웃(`/W [1 w2 w3]`, `/Index` 없음)을 이 코드가 읽는다.
- EOF 너머 오프셋은 하드 에러이고, 자동으로 [repair](repair.md)로 떨어지지 않는다.

## Code
- `justpdf-core/src/xref/mod.rs` — `find_startxref`, `load_xref`, `load_xref_at`, `parse_xref_stream`, `read_field`
- `justpdf-core/src/xref/table.rs` — `Xref`, `XrefEntry`, `parse_xref_table`, `read_ascii_number`

## Reference behaviour
**None.** 비교 대상 조항: ISO 32000-2 §7.5.4(xref 테이블), §7.5.5(trailer), §7.5.6(증분 업데이트), §7.5.8(xref 스트림, hybrid `/XRefStm` 포함) — 기억에 의한 포인터.

## Cross-cutting invariants
- [증분 trailer](../invariant/incremental-trailer.md) — 읽기 쪽 절반이 여기다.

## Blast radius
- [문서 접근](document-access.md) — 모든 객체 조회가 `Xref::get`을 지난다.
- [object streams](object-streams.md) — 타입 2 엔트리가 압축 객체 로드로 이어진다.
- [파일 직렬화](file-serialization.md) — xref 테이블/스트림의 쓰기 쪽. 레이아웃을 바꾸면 양쪽을 같이 본다.
- [증분 저장](incremental-save.md), [서명](signing.md) — 둘 다 `find_startxref`로 `/Prev`를 쓰고 새 trailer를 만든다.
- [repair](repair.md) — xref가 깨졌을 때의 대체 경로(현재는 수동 호출만).

## Known holes / open
- hybrid 파일(`/XRefStm`)을 처리하지 않는다: `rg XRefStm justpdf-core/src` 결과가 비어 있다.
- 이전 trailer 키를 병합하지 않는다(위 Design model).
- Tracked: #26 (증분 trailer 키 소실), #53 (/XRefStm)
