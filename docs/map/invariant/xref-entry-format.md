# xref 항목 형식

## The fact
justpdf가 쓰는 고전 xref 테이블의 항목은 **정확히 20바이트**다: `nnnnnnnnnn ggggg n`(또는 `f`) 18바이트와 2바이트 줄끝. ISO 32000-1:2008 §7.5.4 원문(2026-09-28 대조): "a 2-character end-of-line sequence consisting of one of the following: SP CR, SP LF, or CR LF. Thus, the overall length of the entry shall always be exactly 20 bytes." justpdf는 qpdf와 같은 SP LF(`n \n`)를 쓴다 — qpdf 12.4.1이 만든 픽스처(`aes256_r5_user_owner.pdf` 등)의 항목이 `0000000000 65535 f \n`이다.

## Why it is cross-cutting
고전 xref 테이블을 쓰는 코드가 네 벌이고 서로 호출하지 않는다. 모두 항목 문자열을 손으로 `write!`/`extend_from_slice`한다. 한 곳만 고치면 나머지는 그대로 남는다 — 실제로 첫 수정 목록이 `serialize.rs`의 빈 번호용 free 항목과 서명 writer를 빠뜨렸고 grep으로 찾았다.

## Territories it holds in
- [파일 직렬화](../territory/file-serialization.md) — `serialize_pdf_impl`: 항목 0, 사용 중 항목, 빈 번호의 free 항목.
- [증분 저장](../territory/incremental-save.md) — `incremental_save`: 덧붙인 객체와 제거한 객체(`65535 f`)의 항목.
- [선형화](../territory/linearization.md) — `write_linearized_inner`의 main xref: 항목 0, 사용 중 항목, 빈 번호의 free 항목.
- [서명](../territory/signing.md) — `build_pdf_with_placeholder`: 덧붙인 객체의 항목.

## What a violation looks like
- (#102 전) 네 writer 모두 `n \r\n`(공백 + CRLF, 21바이트)으로 썼다. qpdf 12.4.1(pikepdf `check_pdf_syntax`)이 `DocumentBuilder`로 만든 가장 단순한 파일까지 모든 출력을 "file is damaged / invalid xref entry (obj=1)"로 보고 xref를 재구성해서 열었다(2026-09-28 측정). justpdf 자신의 리더는 줄 단위로 읽어 통과했으므로 왕복 테스트로는 보이지 않았고, `test_xref_entry_format`은 21바이트 형태를 기대값으로 고정하고 있었다.
- 재구성하지 않는 리더는 오프셋이 한 바이트씩 밀린 항목을 읽는다(추론).

## Discovery history
2026-09-28, #102(원본 암호화 유지) 출력을 qpdf로 독립 판정하다가 발견. 이전 판정들은 qpdf가 파일을 **열 수 있는지**와 내용만 봤고, 경고는 보지 않았다.

## Where it will recur
**고전 xref 테이블 항목을 쓰는 코드는 이 불변식의 대상이다.** 새 사이트는 `integration.rs`의 `check_xref_entries`(스펙 문구에서 끌어낸 검사, 빈 번호·제거 객체까지 포함)로 검증한다. 테스트 안에서 리더 입력용으로 손으로 만든 PDF(`parser.rs`, `repair.rs`, `ocg/parse.rs`의 테스트)는 21바이트 항목을 쓰며, 리더 관용성을 위한 입력이라 대상이 아니다. 외부 판정: `uv run --with pikepdf`로 `Pdf.check_pdf_syntax()`가 빈 목록인지 본다.
