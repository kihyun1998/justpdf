# 증분 저장 (incremental save)

## What it is
원본 바이트를 그대로 두고 변경분 객체·새 xref·`/Prev`가 달린 trailer를 파일 끝에 덧붙이는 저장 방식. 서명은 원본 바이트 보존이 필수라 같은 방식을 쓰지만, 서명 쪽은 자기 구현을 따로 가진다([서명](signing.md)).

## Governing decisions
**None.**

## Design model
- 덧붙이는 객체는 `serialize_object`로 쓴다(#26 전에는 Display라 스트림이 디버그 형식으로 깨졌다 — #25의 직렬화 부분).
- `incremental_save(doc, modifier)`는 변경분만 쓴다(#25). 원본 `doc`을 다시 resolve해 modifier의 객체와 값(`PdfObject: PartialEq`)으로 비교하고, 값이 다르거나 원본에 없던 번호만 덧붙인다. 변경을 `set_object`/`add_object` 호출로 추적하지 않는다 — compress 파이프라인 등이 `writer().objects`를 직접 고치기 때문이다(`test_incremental_save_appends_an_object_edited_in_place`).
- 삭제: `from_document`가 복사한 객체(`source_objects` — resolve 성공, `/Encrypt` 제외) 중 modifier에 없는 것만 새 xref에 `0000000000 65535 f`로 쓴다. resolve에 실패해 복사되지 않은 객체와 `/Encrypt` 객체는 free하지 않는다 — 같은 `source_objects`를 복사와 삭제 판정이 함께 쓴다.
  - 예외: 원본 xref에서 **안 바뀐** 객체가 `Compressed`로 가리키는 객체 스트림(ObjStm) 컨테이너는 modifier에 없어도 free하지 않는다. 컨테이너는 `N 0 R`로 참조되지 않고 xref 항목으로만 닿으므로 `garbage_collect`가 지운다. free하면 그 안의 안 바뀐 객체가 resolve되지 않는다(`test_incremental_save_keeps_object_streams_that_hold_unchanged_objects`). 멤버가 모두 바뀌거나 사라진 컨테이너는 free된다. 원본의 xref 스트림 객체도 GC로 빠지면 free되는데, 리더는 xref 구간을 바이트 오프셋으로 찾으므로 영향이 없다.
  - **메인테이너 판단(2026-09-24, #25 구현 중)**: 위 예외. 앞의 "복사됐다가 사라진 것" 판단은 이 경우를 보지 못한 채 내려졌다 — check 단계의 lens·assay가 `set_info`/`delete_page` + `garbage_collect` 뒤 `object stream 8 is not a stream`으로 재현했다. 제시된 대안: 살아 있는 멤버를 일반 객체로 다시 덧붙이기(압축 문서는 대부분이 ObjStm 안이라 변경분만 쓰는 목적이 무너진다), GC와 함께면 거부(MuPDF `pdf-write.c`의 "Can't do incremental writes with garbage collection").
  - free 항목은 ISO 32000-1 §7.5.4의 "두 번째 방식"(세대 65535, 0을 가리킴, 연결 리스트에 넣지 않음, 재사용 불가)이다. 기본 방식(세대 + 1, 리스트 연결 — 부록 H.7.3 예제는 증분 구간에서 객체 0까지 다시 쓴다)은 원본 세대와 기존 free 리스트를 따라가야 한다. MuPDF `pdf-write.c`도 증분 삭제를 65535로 쓴다(`FIXME: would be better to link to existing free list`). justpdf는 새 번호를 항상 원본 최대 번호 위에서 주므로 재사용 불가의 대가가 없다.
- 변경도 삭제도 없으면 원본 바이트를 그대로 돌려준다(개행도 붙이지 않는다). MuPDF도 같은 조기 반환을 한다(`pdf_has_unsaved_changes`).
- 비교 기준은 저장 시점의 원본 문서다. modifier는 원본 사본을 들지 않는다 — `from_document`는 compress·주석·폼 등 모든 수정 기능이 공유하므로, 사본을 들면 증분 저장을 쓰지 않는 호출자도 메모리를 두 배로 쓴다. `doc`이 modifier를 만든 문서인지는 검사하지 않는다(파일 ID가 없는 PDF가 흔해 믿을 식별 수단이 없다).
  - **메인테이너 판단(2026-09-24, #25 triage)**: 위 네 가지(기준 = 저장 시점의 원본 문서, 삭제 범위 = 복사된 것 중 사라진 것, 65535 free 항목, 변경 없음 = 원본 그대로)와 불일치 무검사. 제시된 대안: 변경 판정은 modifier 스냅샷·증분 전용 생성자·더티 플래그·해시 스냅샷, 삭제는 기록 안 함·원본 xref 전체 기준, free 형식은 세대 + 1 연결, 변경 없음은 빈 구간 덧붙임, 불일치는 catalog/`/ID` 비교로 거부. 판단 근거로 제시된 사실: compress의 `writer().objects` 직접 수정, `from_document`의 공유, ISO 32000-1:2008 §7.5.4·§7.5.6·부록 H.7 원문과 MuPDF `pdf-write.c`·`pdf-xref.c` 원문. ISO 32000-2 원문과는 대조하지 않았다.
- 새 trailer는 `incremental_trailer`로 만든다 — 원본 trailer의 키를 옮긴다([증분 trailer](../invariant/incremental-trailer.md)).
- 원본이 암호화되어 있으면 `doc`의 `SecurityState` 파일 키로 덧붙이는 객체를 `encrypt_object`한다(`/Encrypt` 객체는 복사도 비교도 하지 않는다). 키의 출처는 `doc` 하나다 — `DocumentModifier`는 `SecurityState`를 들지 않는다(#25, 메인테이너 판단). 인증되지 않은 암호화 문서로는 modifier를 만들 수 없다([문서 수정기](document-modifier.md)의 `from_document`가 거부한다). `incremental_save` 자신의 검사는 `doc`을 보므로, 같은 바이트를 인증 없이 다시 연 `doc`을 넘기면 걸린다.
  - **메인테이너 판단(2026-09-23, #26)**: 증분 저장은 암호화 입력을 지원하고 서명은 거부한다. 대안이었던 "둘 다 거부"와 "둘 다 지원(서명에 비밀번호 인자 추가)"이 함께 제시되었다.
- `DocumentModifier`는 세대 번호를 버린다 — 바뀐 객체는 `N 0 obj`로 덧붙고, 암호화 키도 세대 0으로 유도한다. 원본에 세대가 0이 아닌 객체가 있으면 참조(`N g R`)와 어긋난다(추론). free 항목은 65535로 쓰므로 원본 세대에 기대지 않는다.
  - **메인테이너 판단(2026-09-24, #25)**: #25를 #72보다 먼저, 독립으로 한다. 처음(triage 1라운드)에는 "#72 먼저"였다 — free 항목에 원본 세대 + 1이 필요하다는 전제였다. 비교 기준을 저장 시점의 `doc`으로 정한 뒤(원본 세대는 `doc`의 xref가 안다), 그리고 65535 free 형식을 고른 뒤 그 전제가 없어져 다시 물었다.

## Code
- `justpdf-core/src/writer/modify.rs` — `incremental_save`, `incremental_trailer`, `source_objects`, `DocumentModifier`, `test_incremental_save_keeps_encryption`, `test_incremental_save_reopens_with_intact_objects`, `test_incremental_save_appends_only_changed_objects`, `test_incremental_save_without_changes_returns_the_original`, `test_incremental_save_frees_removed_objects`, `test_incremental_save_keeps_an_object_that_does_not_resolve`, `test_incremental_save_keeps_object_streams_that_hold_unchanged_objects`, `test_incremental_save_deletes_a_page_of_a_packed_document`, `test_incremental_save_of_an_encrypted_document`, `test_incremental_save_appends_an_object_edited_in_place`, `test_incremental_save_after_xref_stream_section`

## Reference behaviour
MuPDF `pdf-write.c`(`dowriteobject`의 `pdf_xref_is_incremental` 필터, 증분 삭제의 `gen_list[num] = 65535`, 변경 없음 조기 반환). 비교 대상 조항: ISO 32000-2 §7.5.6(원문 대조는 ISO 32000-1:2008 §7.5.4·§7.5.6으로 했다).

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
- 세대 번호가 0이 아닌 객체(위). Tracked: #72
