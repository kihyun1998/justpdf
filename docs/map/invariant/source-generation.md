# 원본 세대

## The fact
원본 문서에서 가져온 객체는 **원본 xref의 세대 번호로** 써야 한다 — 객체 헤더(`N g obj`), xref 항목, 객체 암호화 키(R2–R4) 셋 모두. 문서 안의 참조 `N g R`은 원본 그대로 복사되기 때문이다. 셋이 어긋나면 ISO 32000-1 §7.3.10에 따라 참조는 null(정의되지 않은 객체)이다. 참조의 **번호**를 다시 쓰는 코드는 세대도 새 대상의 세대로 함께 다시 써야 한다. object stream 안 객체는 세대 0이어야 한다(§7.5.7).

writer 쪽 규칙은 `PdfWriter`의 `generations`(0이 아닌 세대만)가 갖는다: [문서 수정기](../territory/document-modifier.md)의 `from_document`가 채우고, writer를 받는 직렬화 함수가 읽는다. 번호·객체 슬라이스만 받는 공개 함수는 세대를 모르므로 0으로 쓴다([파일 직렬화](../territory/file-serialization.md)).

## Why it is cross-cutting
세대는 참조(복사된 값)와 정의(쓰는 쪽이 새로 만드는 헤더·xref·키)에 따로 나타나고, 둘을 맞추는 코드가 여러 모듈에 흩어져 있다: 전체 재작성, 증분 저장, xref 스트림, 중복 병합, 번호 압축, 선형화, object stream 패킹. 서로 호출하지 않는다. #98 전에는 justpdf 리더가 참조의 세대를 보지 않아 어긋나도 자기 왕복 테스트가 통과했다 — 쓰기 결함이 읽기 관용에 가려졌다. 지금 리더는 세대가 틀린 참조를 `Null`로 읽는다([문서 접근](../territory/document-access.md)). 단 `object_refs()`로 얻은 참조는 xref 세대를 쓰므로, 문서 안에 적힌 참조를 따라가는 단언만 어긋남을 본다.

## Territories it holds in
- [문서 수정기](../territory/document-modifier.md) — `from_document`가 세대를 담고, `build`(평문·`set_encryption`·원본 유지)·`build_with_xref_stream`이 그 세대로 쓴다. 지킨다(#72).
- [문서 수정기](../territory/document-modifier.md)의 `merge_documents` — `deep_copy_object`가 다른 문서의 객체를 writer에서 새로 받은 번호에 세대 0으로 넣고, 참조도 새 번호·세대 0으로 쓴다. 지킨다(추론 — 코드 읽기, #106 check 단계).
- [증분 저장](../territory/incremental-save.md) — 덧붙이는 객체의 헤더·xref 항목·키. 지킨다(#72).
- [파일 직렬화](../territory/file-serialization.md) — writer를 받는 함수는 원본 세대로, 공개 슬라이스 함수는 0으로 쓴다.
- [압축 dedup](../territory/compress-dedup.md) — `merge_duplicates`가 합친 참조에 남긴 객체의 세대를 준다. 지킨다(#72).
- [정리(clean)](../territory/clean.md) — `compact_object_numbers`는 번호를 다시 매기며 목록 안 객체를 가리키는 참조를 모두 세대 0으로 쓴다. 결과를 쓰는 공개 슬라이스 직렬화 함수의 세대다. 지킨다(#106).
- [선형화](../territory/linearization.md) — 원본 객체를 xref 세대로 헤더·main xref 항목에 쓰고, trailer `/Root`·`/Info`는 원본 그대로 복사한다. 지킨다(#106).
- [object streams](../territory/object-streams.md) — `DocumentModifier::pack_object_streams`는 세대 ≠ 0 객체를 빼고 컨테이너 번호를 writer에서 새로 받는다. 공개 슬라이스 `pack_object_streams`는 모든 객체를 세대 0으로 본다. writer 쪽 xref 스트림 직렬화가 세대 ≠ 0인 압축 객체·컨테이너를 에러로 거부한다. 지킨다(#106).

## What a violation looks like
- (#72 전) `DocumentModifier`가 세대를 버리고 모든 객체를 `N 0 obj`·세대 0 xref·세대 0 키로 썼다. 원본 `/Info`가 `6 1 obj`인 파일(세대 1·300 픽스처, 평문·RC4·AES-128)을 다시 쓰면 qpdf 12.4.1이 trailer `/Info 6 1 R`을 null로 읽었다(전체 재작성·xref 스트림). 증분 저장에서는 새 구간의 `6 0`이 원본 구간의 `6 1`을 가리지 못해 qpdf가 **바뀌기 전** 객체를 읽었다 — 수정이 조용히 사라진다(2026-09-29 측정). RC4·AES-128 파일은 justpdf 자신도 틀린 키로 복호화해 쓰레기·패딩 에러를 낸다.
- 중복 병합이 번호만 바꾸면 남긴 객체가 `K 1 obj`인데 합친 참조가 `K 0 R`이다(#72 check 단계에서 코드로 발견, 테스트로 재현).

## Discovery history
2026-09-24 #26(증분 trailer) 작업 중 `DocumentModifier`가 세대를 버리는 것을 코드로 발견해 #72로 등록. 2026-09-29 #72 수정 중 테스트와 qpdf 판정으로 재현했고, check 단계의 assay·lens가 중복 병합 사이트를 찾았다. 나머지 어기는 사이트(번호 압축, 선형화, 패킹)는 같은 날 grep과 코드 읽기로 모았고, #106에서 테스트로 재현해 고쳤다.

## Where it will recur
**원본 문서의 객체를 쓰거나, 참조의 번호를 다시 쓰는 코드는 이 불변식의 대상이다.** 원본에서 온 객체는 `PdfWriter`를 거쳐 writer 쪽 직렬화 함수로 쓴다. 참조 번호를 바꾸면 세대도 새 대상의 세대로 바꾼다(`merge_duplicates`의 `rewrite_references`). 쓰는 쪽이 따로 들고 있는 참조(modifier의 `catalog_ref`·`info_ref`)는 세대를 믿지 말고 writer에서 얻는다(`PdfWriter::reference_to`, #110). 검증: 세대 ≠ 0 픽스처(`with_info_at_generation`, `writer/modify.rs` 테스트)로 헤더·xref 항목을 직접 보고, 키는 RC4·AES-128 왕복으로 보고(R6은 증거가 아니다), 외부 판정은 pikepdf로 `pdf.trailer.Info.objgen`과 `check_pdf_syntax()`를 본다. justpdf 리더의 왕복은 문서 안에 적힌 참조(trailer·사전 값)를 resolve할 때만 증거가 된다 — `object_refs()`를 도는 왕복은 아니다. 도구가 볼 수 있는 절반: `rg '\{\} 0 obj|obj_num = new|r\.obj_num =' justpdf-core/src` — `clean.rs`의 `rewrite_references`를 찾고, 새 번호만 쓰는 선형화 사전·힌트 스트림, `sign_pdf`, xref 스트림 자신도 걸린다(대상 아님). 도구가 볼 수 없는 절반: 번호·객체 슬라이스만 받는 공개 함수(`pack_object_streams`, `clean_objects`, `serialize_pdf*`)가 모든 객체를 세대 0으로 본다는 가정 — writer의 객체를 이 함수들에 넘기는 호출자, 그리고 `writer().objects`의 번호를 직접 바꾸는 호출자.
