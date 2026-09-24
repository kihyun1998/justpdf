# 증분 trailer

## The fact
증분 업데이트로 덧붙인 구간의 trailer는 **원본 trailer의 문서 수준 키(`/Root`, `/Info`, `/Encrypt`, `/ID` 등)를 다시 적어야** 한다. justpdf의 xref 리더는 최신 trailer 하나만 남기고 이전 trailer의 키를 병합하지 않으므로, 새 trailer에 없는 키는 읽기 쪽에서 사라진다. pdf.js·MuPDF도 최신 trailer만 읽는다 — 쓰기 쪽이 옮겨 적지 않으면 다른 리더에서도 사라진다.

두 쓰기 사이트는 `incremental_trailer`(`writer/modify.rs`) 한 함수로 새 trailer를 만든다: 이전 trailer의 키를 모두 옮기고 xref 스트림 전용 키(`/Type /W /Index /Filter /DecodeParms /Length /F /FFilter /FDecodeParms /DL`)와 `/XRefStm`은 뺀 뒤, `/Size`(이전 값 이상), `/Root`, 필요하면 `/Info`, `/Prev`를 쓴다.

**메인테이너 판단(2026-09-23, #26)**: 쓰기 쪽만 고친다. 리더는 병합하지 않는다. 제시된 대안: 리더 병합(자기 파일은 읽지만 pdf.js·MuPDF에서는 여전히 깨지고, 쓰기 누락이 가려진다), 둘 다. 판단 근거로 제시된 사실: pdf.js `xref.js`(`topDict ||= dict`)와 MuPDF `pdf_trailer`(최신 섹션)가 둘 다 병합하지 않는다. ISO 32000-2 §7.5.6 문구는 원문과 대조하지 않았다.

## Why it is cross-cutting
증분 구간을 쓰는 코드가 두 벌(일반 증분 저장, 서명)이고 서로 호출하지 않는다. 둘 다 최소 키만 쓴다. 그리고 그 결과를 읽는 [xref](../territory/xref.md)는 또 다른 모듈이다. 쓰기 쪽 두 사이트와 읽기 쪽 한 사이트가 호출 없이 같은 가정을 공유한다.

## Territories it holds in
- [xref](../territory/xref.md) — `load_xref_at`: 최신 trailer만 남긴다("First trailer wins for main keys", 병합 없음). 의도된 동작이다(위 판단).
- [증분 저장](../territory/incremental-save.md) — `incremental_trailer`로 쓴다.
- [서명](../territory/signing.md) — `incremental_trailer`로 쓴다.

## What a violation looks like
- (#26 전) 암호화 PDF를 인증 후 `incremental_save`하면 결과 파일이 `is_encrypted=false`로 열리고 trailer에 `/Encrypt`가 없었다. 덧붙인 객체는 복호화된 평문으로 기록됐다(2026-09-23 재현). 서명 쪽은 같은 형태(코드 근거).
- 서명 후 `/Info`가 사라진다(서명 쪽).
- 다른 리더(이전 trailer를 따라가는)와 justpdf가 같은 파일을 다르게 읽는다.

## Discovery history
기록된 사고는 없다. 2026-09-23 맵 작성 중 읽기 쪽 연구 에이전트가 코드에서 보고했고, 같은 날 임시 프로브로 `incremental_save` 쪽을 재현했다(위). `test_incremental_update`는 최신 trailer의 `/Info`가 이긴다는 것만 확인한다.


## Where it will recur
**`/Prev`를 가진 trailer를 쓰는 함수는 이 불변식의 대상이다.** 새 trailer는 `incremental_trailer`로 만든다. 손으로 `<< /Size … /Prev … >>`를 쓰지 않는다. 문서가 암호화되어 있으면 덧붙이는 객체도 원래 파일 키로 암호화해야 한다(`incremental_save`는 그렇게 하고, `sign_pdf`는 키가 없어 암호화 입력을 거부한다).
