# 증분 trailer

## The fact
증분 업데이트로 덧붙인 구간의 trailer는 **원본 trailer의 문서 수준 키(`/Root`, `/Info`, `/Encrypt`, `/ID`)를 다시 적어야** 한다. justpdf의 xref 리더는 최신 trailer 하나만 남기고 이전 trailer의 키를 병합하지 않으므로, 새 trailer에 없는 키는 읽기 쪽에서 사라진다.

## Why it is cross-cutting
증분 구간을 쓰는 코드가 두 벌(일반 증분 저장, 서명)이고 서로 호출하지 않는다. 둘 다 최소 키만 쓴다. 그리고 그 결과를 읽는 [xref](../territory/xref.md)는 또 다른 모듈이다. 쓰기 쪽 두 사이트와 읽기 쪽 한 사이트가 호출 없이 같은 가정을 공유한다.

## Territories it holds in
- [xref](../territory/xref.md) — `load_xref_at`: 최신 trailer만 남긴다("First trailer wins for main keys", 병합 없음).
- [증분 저장](../territory/incremental-save.md) — `/Size /Root /Info /Prev`만 쓴다.
- [서명](../territory/signing.md) — `/Size /Root /Prev`만 쓴다.

## What a violation looks like
- 암호화 PDF를 인증 후 `incremental_save`하면 결과 파일이 `is_encrypted=false`로 열리고 trailer에 `/Encrypt`가 없다. 덧붙인 객체는 복호화된 평문으로 기록된다(2026-09-23 재현). 서명 쪽은 같은 형태로 추론.
- 서명 후 `/Info`가 사라진다(서명 쪽).
- 다른 리더(이전 trailer를 따라가는)와 justpdf가 같은 파일을 다르게 읽는다.

## Discovery history
기록된 사고는 없다. 2026-09-23 맵 작성 중 읽기 쪽 연구 에이전트가 코드에서 보고했고, 같은 날 임시 프로브로 `incremental_save` 쪽을 재현했다(위). `test_incremental_update`는 최신 trailer의 `/Info`가 이긴다는 것만 확인한다.

- Tracked: #26 (증분 trailer 키 소실)

## Where it will recur
**`/Prev`를 가진 trailer를 쓰는 함수는 이 불변식의 대상이다.** 확인할 것: 원본 trailer의 `/Encrypt`·`/ID`·`/Info`를 옮겨 적는가? 리더 쪽에서 이전 trailer 키를 병합하도록 바꾸면 쓰기 쪽 누락이 가려지므로, 어느 쪽을 고칠지는 함께 결정한다.
