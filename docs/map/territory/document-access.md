# 문서 열기·resolve·캐시

## What it is
`PdfDocument`가 원본 바이트(`Vec` 또는 mmap), 병합된 xref, 객체 LRU 캐시, 암호화 상태를 소유하고 `resolve(&self)`로 참조를 객체로 바꾼다. 모든 읽기 기능과 모든 바인딩이 이 한 타입을 지난다.

## Governing decisions
결정 기록(ADR)은 없다. 유지보수자의 판단(2026-09-29, #98): 참조의 세대가 xref 항목이 정의하는 세대와 다르면 `resolve`는 `Null`을 돌려준다 — ISO 32000-1 §7.3.10 "An indirect reference to an undefined object shall not be considered an error... it shall be treated as a reference to the null object". 보여 준 것: 세 선택지의 결과 열거(원문 코드 대조: MuPDF `pdf-xref.c`는 번호로만 찾고 키를 xref 세대로 만든다, pdf.js `xref.js`는 불일치를 에러로 던지고 복구 모드에서 재색인한다, qpdf `QPDF_objects.cc`는 (번호, 세대)로 찾고 없으면 조용히 null), 이 컴퓨터의 PDF 121개(참조 420,709개)에서 세대 불일치 0건·세대 ≠ 0 객체 0건이라는 측정(선택지를 가르지 못함), 그리고 #72에서 이 관용이 쓰기 쪽 세대 결함을 테스트에서 가렸다는 사실. 대안: (b) MuPDF처럼 xref 세대로 키를 만들고 참조 세대는 무시, (c) pdf.js처럼 에러. 헤더의 **객체 번호** 검사는 이 판단이 다루지 않았다(따로 등록).

## Design model
- **내부 가변성**: `resolve`가 `&self`만 요구하도록 `RwLock`을 쓴다(`Sync`). I/O 중에는 락을 잡지 않는다.
- 캐시 적중 경로도 LRU 순서 갱신 때문에 `write()` 락을 잡는다.
- **암호화 훅 세 지점**: `/Encrypt` 사전은 `load_object_raw`(복호화 안 함), 일반 객체는 인증 후 `load_object`에서 자기 번호로 복호화(object stream 안 객체는 ObjStm과 함께 `load_compressed_object`에서 복호화되고 여기서 건너뛴다 — [객체 복호화](object-decryption.md)), 미인증이면 `EncryptedDocument` 에러. 열 때 빈 비밀번호를 자동 시도하고, `authenticate`는 두 캐시를 비운다. 보안 핸들러는 `Standard`만 받는다.
- **세대 검사**: `resolve`는 캐시에 없고 인증을 통과한 참조의 세대를 xref 항목이 정의하는 세대(`XrefEntry::defined_generation` — 사용 중 항목은 그 세대, object stream 안 객체는 0)와 비교하고, 다르면 `Null`을 돌려준다(캐시하지 않는다). 그래서 성공하는 `resolve`의 복호화 키는 언제나 xref 세대로 만든다 — #98 전에는 참조의 세대로 만들어, 세대가 틀린 참조가 RC4에서는 쓰레기, AES-128에서는 패딩 에러가 됐다(R6은 키가 세대와 무관). free 엔트리는 `Null`, 없는 번호는 `ObjectNotFound` 에러다(아래 Known holes).
- 검사는 `resolve`에만 있다. `load_object_raw`에는 없다 — `/Encrypt` 사전(trailer 참조로 `detect_encryption`이 직접 읽음)과 object stream 컨테이너(세대 0으로 읽음)는 이 검사를 거치지 않는다.
- 파싱한 객체 헤더(`N g obj`)의 번호·세대는 xref와 대조하지 않는다 — `load_object_raw`가 버린다.
- 순환 참조는 `resolve` 호출 단위의 `visited`로 끊는다.

## Code
- `justpdf-core/src/parser.rs` — `PdfDocument`, `open`, `from_bytes`, `open_mmap`, `resolve`, `authenticate`, `is_encrypted`, `load_object`, `load_object_raw`, `detect_encryption`, `LruCache`, `DEFAULT_CACHE_CAPACITY`, `generation_matches`, `object_refs`
- `justpdf-core/src/xref/table.rs` — `XrefEntry`, `defined_generation`
- `justpdf-core/tests/integration.rs` — `test_a_reference_at_another_generation_resolves_to_null`, `test_an_object_in_an_object_stream_resolves_only_at_generation_zero`, `test_an_object_at_generation_one_resolves_only_at_generation_one`
- `justpdf-core/src/error.rs` — `JustPdfError`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [xref](xref.md), [객체 모델](object-model.md), [object streams](object-streams.md) — 로드 경로의 하위 단계.
- [객체 복호화](object-decryption.md), [비밀번호 인증](password-authentication.md) — 암호화 훅 세 지점. 훅 위치를 옮기면 두 노트를 함께 본다.
- [repair](repair.md) — `from_raw_parts`로 문서를 만들며 암호화 감지를 건너뛴다.
- [파사드](facade.md), [CLI](cli.md), [언어 바인딩](language-bindings.md) — `open`/`from_bytes`/`authenticate`의 시그니처 변경이 모두 닿는다.
- [압축 파이프라인](compress-pipeline.md) — 암호화 문서를 거부하는 판단이 `is_encrypted`에 기댄다.

## Known holes / open
- 없는 번호를 가리키는 참조는 `ObjectNotFound` 에러다 — §7.3.10은 정의되지 않은 객체를 null로 다룬다. 이 컴퓨터의 PDF 121개 중 32개에 그런 참조가 3,354개 있었다(2026-09-29, #98 lens). Tracked: #108
- 객체 헤더의 번호가 xref와 달라도(오프셋이 다른 객체를 가리켜도) 그 객체를 돌려준다. object stream의 인덱스 번호도 같다. Tracked: #109
- 캐시 적중도 쓰기 락을 잡으므로 여러 스레드의 `resolve`가 직렬화된다(추론: 병렬 렌더의 병목 후보).
