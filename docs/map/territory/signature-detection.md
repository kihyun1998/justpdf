# 서명 필드 감지

## What it is
AcroForm 필드 트리를 재귀로 걸어 `/FT /Sig`이고 `/V`가 있는 필드를 서명으로 수집하고, 서명 사전의 이름·사유·위치·ByteRange·Contents를 꺼낸다. 파사드 `Document::signatures`가 부르는 유일한 서명 기능이다.

## Governing decisions
**None.**

## Design model
- `/Name`·`/Reason`·`/T` 등은 `from_utf8_lossy`로 디코드한다 — PDFDocEncoding·UTF-16BE BOM을 이해하지 못한다([텍스트 문자열 인코딩](../invariant/text-string-encoding.md)).
- `/Kids`가 있으면 조기 반환하므로, 위젯만 자식으로 가진 필드는 검사되지 않는다(추론).
- 암호화 문서에서는 `/Contents`가 `resolve`를 지나며 복호화된다(추론: 스펙상 서명 값은 암호화 대상이 아님).

## Code
- `justpdf-core/src/sign/detect.rs` — `detect_signatures`, `collect_sig_fields`

## Reference behaviour
**None.** 비교 대상 조항: ISO 32000-2 §12.8.

## Cross-cutting invariants
- [텍스트 문자열 인코딩](../invariant/text-string-encoding.md)

## Blast radius
- [AcroForm](acroform.md) — 같은 필드 트리를 각자 걷는다.
- [서명](signing.md) — 여기서 찾을 수 있어야 할 서명을 만드는 쪽(현재는 못 찾는다 — 서명 쪽이 AcroForm에 필드를 등록하지 않는다).
- [서명 검증](signature-verification.md) — 감지 결과의 소비처.
- [객체 복호화](object-decryption.md) — `/Contents` 복호화.
- [파사드](facade.md) — `signatures`.

## Known holes / open
- justpdf가 방금 서명한 파일에서 서명을 찾지 못한다(추론; 서명 → 감지 테스트 없음).
- Tracked: #33 (텍스트 문자열 인코딩), #37 (서명 연결·CLI sign)
