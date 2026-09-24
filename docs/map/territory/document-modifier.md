# 문서 수정기 (DocumentModifier)

## What it is
기존 문서의 모든 객체를 `PdfWriter`로 풀어 놓고(`from_document`), 페이지 삭제·삽입·재정렬, Info 설정, 병합, 가비지 컬렉션을 한 뒤 전체를 다시 쓴다. 주석·폼·OCG·아웃라인·페이지 레이블·첨부파일·압축 등 "기존 파일을 바꾸는" 거의 모든 기능이 이 위에서 돈다.

## Governing decisions
결정 기록(ADR)은 없다. 유지보수자의 판단(2026-09-24, #75 triage): 기존 문서의 암호화 저장은 core의 `DocumentModifier`가 맡고 CLI `encrypt`는 호출만 한다. 보여 준 것: CLI가 trailer를 손으로 조립해 `/Info`를 잃고 `/ID` 둘째 원소를 복사하던 재현 결과. 대안: CLI에 `info_ref` 접근자만 더하는 국소 수정.

## Design model
- `from_document`는 resolve에 실패한 객체를 조용히 건너뛴다(`if let Ok`). 암호화 문서를 인증 없이 넣으면 대부분 빈 결과가 된다(추론).
- `build`는 GC를 하지 않는다. GC는 `garbage_collect`를 따로 불러야 한다 — 그래서 [리댁션](redaction.md)이 걷어낸 옛 콘텐츠 스트림이 파일에 남는다.
- 원본 세대 번호는 버려지고 모두 `0 obj`로 쓴다(추론).
- `set_info`는 [텍스트 문자열 인코딩](../invariant/text-string-encoding.md) 문제를 [문서 빌더](document-builder.md)와 공유한다.
- **암호화 저장**: `set_encryption(EncryptionConfig)` 후의 `build`는 [파일 직렬화](file-serialization.md)의 `serialize_writer_encrypted`로 암호화된 파일을 쓴다. `/Info`는 그대로 옮기고, `/ID`는 [객체 암호화](object-encryption.md)의 파일 ID 규칙(원본 첫 원소 유지·둘째 새 난수, 원본 `/ID`가 없으면 두 원소 같은 새 난수)을 따른다. `from_document`가 원본 `/ID` 첫 원소를 `extract_file_id`로 받아 둔다.
- `set_encryption`을 부르지 않은 `build`는 암호화하지 않는다. 인증된 암호화 원본에서 만든 modifier도 평문으로 쓰며, CLI `decrypt`가 이 동작에 기댄다(원본 `SecurityState`는 [증분 저장](incremental-save.md)만 쓴다).
- `set_encryption` 후 `build_with_xref_stream`과 `incremental_save`는 `UnsupportedEncryption` 에러다 — 앞의 것은 xref 스트림 경로에 암호화 구현이 없고, 뒤의 것은 원본의 암호화(또는 평문)를 따라가므로, 둘 다 설정을 조용히 무시하고 쓰지 않도록 막는다(#75에서 정한 기술적 판단).
- 원본이 암호화되어 있었다면 옛 `/Encrypt` 사전 객체가 `from_document`로 함께 복사되어, 다시 쓴 파일에 참조되지 않는 객체로 남는다(2026-09-24 측정: AES-128 문서를 CLI `decrypt`한 출력에 `/Filter /Standard` 사전 1개). 평문으로 다시 쓰면 옛 `/O`·`/U`(`/OE`·`/UE`)가 평문으로, 새로 암호화하면 새 키로 암호화되어 남는다 — 새 비밀번호를 아는 사람은 옛 비밀번호를 오프라인으로 공격할 재료를 얻는다(추론).
- 병합(`merge_documents`)은 `graft_page`/`deep_copy_object`로 페이지와 의존 객체를 복사하고 리소스 이름 충돌을 처리한다.

## Code
- `justpdf-core/src/writer/modify.rs` — `DocumentModifier`, `from_document`, `delete_page`, `insert_page`, `reorder_pages`, `set_info`, `garbage_collect`, `set_encryption`, `build`, `build_with_xref_stream`, `merge_documents`, `graft_page`, `deep_copy_object`, `test_build_with_encryption_roundtrip`, `test_build_with_encryption_keeps_permanent_id_and_changes_the_other`, `test_build_with_encryption_treats_empty_id_as_absent`, `test_build_without_encryption_decrypts_an_authenticated_source`, `test_build_with_xref_stream_refuses_encryption`, `test_incremental_save_refuses_encryption`, `test_build_with_encryption_reencrypts_an_authenticated_source`, `test_build_with_encryption_encrypts_an_object_set_at_a_new_number`

## Reference behaviour
**None.**

## Cross-cutting invariants
- [텍스트 문자열 인코딩](../invariant/text-string-encoding.md) — `set_info`.

## Blast radius
- [파일 직렬화](file-serialization.md) — `build`의 출력 경로.
- [객체 암호화](object-encryption.md) — `set_encryption` 후 `build`의 파일 ID 규칙과 암호화.
- [페이지 트리](page-tree.md) — 페이지 조작이 트리를 다시 쓴다.
- 이 수정기 위에 선 기능들 — [주석](annotations.md), [리댁션](redaction.md), [폼 채우기](form-fill.md), [폼 평탄화](form-flatten.md), [optional content](optional-content.md), [아웃라인](outlines.md), [페이지 레이블](page-labels.md), [첨부파일](embedded-files.md), [압축 파이프라인](compress-pipeline.md). `from_document`/`build` 의미를 바꾸면 모두 확인한다.
- [CLI](cli.md) — split/encrypt/decrypt/clean/merge. [파사드](facade.md) — `Modifier` 래퍼.

## Known holes / open
- resolve 실패 객체를 경고 없이 버린다.
- Tracked: #33 (텍스트 문자열 인코딩)
