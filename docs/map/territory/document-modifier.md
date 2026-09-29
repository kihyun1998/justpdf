# 문서 수정기 (DocumentModifier)

## What it is
기존 문서의 모든 객체를 `PdfWriter`로 풀어 놓고(`from_document`), 페이지 삭제·삽입·재정렬, Info 설정, 병합, 가비지 컬렉션을 한 뒤 전체를 다시 쓴다. 주석·폼·OCG·아웃라인·페이지 레이블·첨부파일·압축 등 "기존 파일을 바꾸는" 거의 모든 기능이 이 위에서 돈다.

## Governing decisions
결정 기록(ADR)은 없다. 유지보수자의 판단(2026-09-24, #75 triage): 기존 문서의 암호화 저장은 core의 `DocumentModifier`가 맡고 CLI `encrypt`는 호출만 한다. 보여 준 것: CLI가 trailer를 손으로 조립해 `/Info`를 잃고 `/ID` 둘째 원소를 복사하던 재현 결과. 대안: CLI에 `info_ref` 접근자만 더하는 국소 수정.

## Design model
- `from_document`는 인증되지 않은 암호화 문서를 `EncryptedDocument` 에러로 거부한다. 거부하기 전에는 거의 모든 객체가 resolve에 실패해 조용히 빠졌고, CLI `encrypt`가 깨진 파일을 성공으로 썼다.
- 그 밖에 resolve에 실패한 객체는 여전히 조용히 건너뛴다. 복사 규칙(resolve 성공, `/Encrypt` 제외)은 `source_objects` 하나에 있고, [증분 저장](incremental-save.md)의 삭제 판정이 같은 함수를 쓴다.
- 새 객체 번호는 원본이 쓰는 **모든** 번호(건너뛴 것 포함) 위에서 시작한다. 건너뛴 번호를 다시 주면, 암호화 제외가 번호로만 판정되는 탓에([파일 직렬화](file-serialization.md)) 새 객체가 옛 `/Encrypt` 번호를 받아 증분 저장에서 평문으로 나간다 — `test_incremental_save_keeps_encryption`이 잡는다.
- `build`는 GC를 하지 않는다. GC는 `garbage_collect`를 따로 불러야 한다 — 그래서 [리댁션](redaction.md)이 걷어낸 옛 콘텐츠 스트림이 파일에 남는다.
- **세대 번호**: `from_document`가 원본 xref의 세대 번호(0이 아닌 것만)를 `PdfWriter`의 `generations`에 담고, `build`(평문·`set_encryption`·원본 유지)·`build_with_xref_stream`·[증분 저장](incremental-save.md)이 그 세대로 객체 헤더(`N g obj`)·xref 항목·객체 암호화 키를 쓴다(#72). 문서 안의 참조 `N g R`은 원본 그대로 복사되므로, 세대를 버리면 참조가 가리키는 객체가 파일에 없다. #72 전 출력을 qpdf 12.4.1(pikepdf 10.15.0)로 읽으면 전체 재작성·xref 스트림 출력의 trailer `/Info N 1 R`이 null이었고, 증분 저장 출력에서는 이전 구간의 옛 객체(바뀌기 전 값)가 읽혔다(2026-09-29, 세대 1·300 픽스처, 평문·RC4·AES-128). 고친 뒤 같은 13개 출력을 qpdf가 경고 없이 `(N, 1)`/`(N, 300)`으로 읽는다.
- justpdf 리더는 참조의 세대를 보지 않는다([문서 접근](document-access.md), #98). 그래서 자기 왕복 테스트로는 세대 결함이 드러나지 않는다 — 테스트(`assert_info_at_generation`)는 헤더를 바이트로, xref 항목을 파싱 결과로 보고, 키는 RC4·AES-128 왕복으로 본다. R6(AES-256)은 객체 키가 번호·세대와 무관해 증거가 되지 않는다. 픽스처(`with_info_at_generation`)는 justpdf 자신이 쓰므로 위 qpdf 판정이 독립 판정자다.
- 세대는 writer가 그 번호의 객체를 들고 있는 동안만 유지된다: `set_object`로 들고 있는 객체를 바꾸면 세대를 유지하고, 들고 있지 않은 번호(GC로 지운 번호, resolve 실패로 복사되지 않은 번호, 원본 `/Encrypt` 번호)에 넣으면 세대 0이다. `add_object`도 0. `set_object`는 참조를 돌려주지 않으므로 호출자가 새 객체를 가리킬 참조는 `N 0 R`이다. 번호 재사용 시 세대 + 1(ISO 32000-1 §7.5.4, MuPDF `gen_list[num]++`)은 하지 않는다 — 새 번호는 원본 최대 번호 위에서 주므로 crate 안에서 재사용하는 호출자가 없다.
- `generations`는 `objects` 옆의 별도 맵이다. 공개 필드 `objects: Vec<(u32, PdfObject)>`의 타입을 바꾸면 공개 API가 깨지고, compress 등 crate 안에서 직접 고치는 곳도 많다(기술적 판단, #72). 대가: 참조의 번호를 바꾸는 코드는 세대도 함께 맞춰야 한다 — [원본 세대](../invariant/source-generation.md).
  - **메인테이너 판단(2026-09-29, #72)**: 세대 ≠ 0 객체를 object stream에서 빼는 일은 이번 변경에서 뺐다(ISO 32000-1 §7.5.7: object stream 안 객체의 세대는 0). 보여 준 것: crate 안의 유일한 패커 `pack_into_object_streams`는 비활성(`#[allow(dead_code)]`, 호출부 주석 처리)이고, 공개 `pack_object_streams`는 세대 정보 없이 번호·객체만 받아 세대를 걸러내려면 공개 시그니처를 바꿔야 한다는 것. 대안: 비활성 내부 함수만 고침, 공개 API까지 변경. 이 판단은 패킹만 다뤘다 — 아래 번호 재작성 사이트는 다루지 않았다.
- `set_info`는 [텍스트 문자열 인코딩](../invariant/text-string-encoding.md) 문제를 [문서 빌더](document-builder.md)와 공유한다.
- **암호화 저장**: `set_encryption(EncryptionConfig)` 후의 `build`는 [파일 직렬화](file-serialization.md)의 `serialize_writer_encrypted`로 암호화된 파일을 쓴다. `/Info`는 그대로 옮기고, `/ID`는 [객체 암호화](object-encryption.md)의 파일 ID 규칙(원본 첫 원소 유지·둘째 새 난수, 원본 `/ID`가 없으면 두 원소 같은 새 난수)을 따른다. `from_document`가 원본 `/ID` 첫 원소를 `extract_file_id`로 받아 둔다.
- `build`의 암호화는 세 상태(`Encryption`) 중 하나다: 없음(기본), 새 설정(`set_encryption`), 원본 유지(`preserve_encryption`). 두 메서드는 나중에 부른 쪽이 이긴다(메인테이너 확인, 2026-09-28, #102). 아무것도 부르지 않은 `build`는 인증된 암호화 원본에서 만든 modifier도 평문으로 쓰며, CLI `decrypt`가 이 동작에 기댄다. 기본값을 유지로 바꾸지 않은 것은 메인테이너 판단이다(#102 — 보여 준 것: pikepdf `save()` 기본값도 암호화를 푼다는 측정, 대안: 기본 유지).
- **원본 유지**: `from_document`가 원본 `SecurityState`(파일 키 포함)와 trailer의 `/Encrypt` 사전을 쓰인 그대로(`source_encrypt_dict`) 잡아 둔다. `build`는 그 사전을 새 번호의 객체로 추가하고 `encrypt_obj_num`을 그 번호로 바꿔(암호화 제외가 번호로 판정되므로) `serialize_pdf_encrypted`로 쓴다. 새 키를 유도하지 않으므로 비밀번호가 필요 없고, 원본 사용자·소유자 비밀번호가 둘 다 그대로 연다 — qpdf의 암호화 유지와 같은 방식이다(pikepdf `save(encryption=True)`, 2026-09-28 측정: 사용자 비밀번호로만 연 R5 파일의 출력이 두 비밀번호로 열림). `/ID`는 [키를 유도한 값인 `state.file_id`, 새 난수]다 — R2–R4 키 유도가 첫 원소를 쓴다. 원본이 암호화되지 않았으면 평문으로 쓴다. 원본에서 Identity crypt 필터로 평문이던 스트림은 읽을 때 `/Crypt`가 빠지므로 원본 유지 출력에서는 기본 방식으로 암호화된다(추론).
- [증분 저장](incremental-save.md)은 원본 `PdfDocument`를 받아 그 키를 쓰므로(#25) 원본 유지 상태를 허용한다.
- `set_encryption` 후 `build_with_xref_stream`과 `incremental_save`, 그리고 암호화 원본에 `preserve_encryption` 후 `build_with_xref_stream`은 `UnsupportedEncryption` 에러다 — 앞의 것은 xref 스트림 경로에 암호화 구현이 없고, 뒤의 것은 원본의 암호화(또는 평문)를 따라가므로, 둘 다 설정을 조용히 무시하고 쓰지 않도록 막는다(#75에서 정한 기술적 판단).
- `from_document`는 trailer의 `/Encrypt` 사전 객체를 복사하지 않는다. 복사하면 다시 쓴 파일에 참조되지 않는 객체로 남아, 평문 재작성(CLI `decrypt`)에서는 옛 `/O`·`/U`(`/OE`·`/UE`)가 평문으로 나가 옛 비밀번호를 오프라인으로 공격할 재료가 된다. 증분 저장은 원본 바이트와 trailer의 `/Encrypt` 참조를 그대로 두므로 영향이 없다.
- 병합(`merge_documents`)은 `graft_page`/`deep_copy_object`로 페이지와 의존 객체를 복사하고 리소스 이름 충돌을 처리한다.

## Code
- `justpdf-core/src/writer/modify.rs` — `DocumentModifier`, `from_document`, `delete_page`, `insert_page`, `reorder_pages`, `set_info`, `garbage_collect`, `set_encryption`, `preserve_encryption`, `Encryption`, `source_encrypt_dict`, `build`, `build_with_source_encryption`, `build_with_xref_stream`, `merge_documents`, `graft_page`, `deep_copy_object`, `test_build_with_encryption_roundtrip`, `test_build_with_encryption_keeps_permanent_id_and_changes_the_other`, `test_build_with_encryption_treats_empty_id_as_absent`, `test_build_without_encryption_decrypts_an_authenticated_source`, `test_build_with_xref_stream_refuses_encryption`, `test_incremental_save_refuses_encryption`, `test_build_with_encryption_reencrypts_an_authenticated_source`, `test_build_with_encryption_encrypts_an_object_set_at_a_new_number`, `test_from_document_requires_authentication`, `test_rewrite_drops_the_source_encrypt_dictionary`, `test_preserve_encryption_on_an_unencrypted_source_writes_it_plain`, `test_set_encryption_after_preserve_encryption_wins`, `test_preserve_encryption_after_set_encryption_wins`, `test_preserve_encryption_encrypts_an_object_set_at_the_source_encrypt_number`, `test_build_with_xref_stream_refuses_preserved_encryption`, `test_incremental_save_accepts_preserved_encryption`, `with_info_at_generation`, `assert_info_at_generation`, `test_generation_one_source_reads_its_info`, `test_build_keeps_a_source_generation`, `test_build_keeps_the_generation_of_a_replaced_object`, `test_build_with_encryption_keeps_a_source_generation`, `test_build_with_xref_stream_keeps_a_source_generation`, `test_build_with_xref_stream_keeps_a_generation_wider_than_a_byte`, `test_incremental_save_keeps_a_source_generation`, `test_set_object_at_a_number_no_longer_held_is_generation_zero`, `test_add_object_after_from_document_is_generation_zero`
- `justpdf-core/src/writer/mod.rs` — `PdfWriter`, `generation`, `generation_of`, `set_object`
- `justpdf-core/tests/integration.rs` — `test_preserve_encryption_keeps_the_source_encryption_in_third_party_files`, `test_preserve_encryption_leaves_document_metadata_as_the_source_does`

## Reference behaviour
**None.**

## Cross-cutting invariants
- [텍스트 문자열 인코딩](../invariant/text-string-encoding.md) — `set_info`.
- [원본 세대](../invariant/source-generation.md) — `from_document`가 세대를 담고, `build`·`build_with_xref_stream`이 그 세대로 쓴다.

## Blast radius
- [파일 직렬화](file-serialization.md) — `build`의 출력 경로.
- [객체 암호화](object-encryption.md) — `set_encryption` 후 `build`의 파일 ID 규칙과 암호화.
- [페이지 트리](page-tree.md) — 페이지 조작이 트리를 다시 쓴다.
- 이 수정기 위에 선 기능들 — [주석](annotations.md), [리댁션](redaction.md), [폼 채우기](form-fill.md), [폼 평탄화](form-flatten.md), [optional content](optional-content.md), [아웃라인](outlines.md), [페이지 레이블](page-labels.md), [첨부파일](embedded-files.md), [압축 파이프라인](compress-pipeline.md). `from_document`/`build` 의미를 바꾸면 모두 확인한다.
- [CLI](cli.md) — split/encrypt/decrypt/clean/merge. [파사드](facade.md) — `Modifier` 래퍼.

## Known holes / open
- resolve 실패 객체를 경고 없이 버린다.
- Tracked: #33 (텍스트 문자열 인코딩)
