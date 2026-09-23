# 문서 열기·resolve·캐시

## What it is
`PdfDocument`가 원본 바이트(`Vec` 또는 mmap), 병합된 xref, 객체 LRU 캐시, 암호화 상태를 소유하고 `resolve(&self)`로 참조를 객체로 바꾼다. 모든 읽기 기능과 모든 바인딩이 이 한 타입을 지난다.

## Governing decisions
**None.**

## Design model
- **내부 가변성**: `resolve`가 `&self`만 요구하도록 `RwLock`을 쓴다(`Sync`). I/O 중에는 락을 잡지 않는다.
- 캐시 적중 경로도 LRU 순서 갱신 때문에 `write()` 락을 잡는다.
- **암호화 훅 세 지점**: `/Encrypt` 사전은 `load_object_raw`(복호화 안 함), 일반 객체는 인증 후 `load_object`에서 복호화, 미인증이면 `EncryptedDocument` 에러. 열 때 빈 비밀번호를 자동 시도하고, `authenticate`는 두 캐시를 비운다. 보안 핸들러는 `Standard`만 받는다.
- xref 조회는 세대 번호를 무시한다. free 엔트리는 `Null`.
- 순환 참조는 `resolve` 호출 단위의 `visited`로 끊는다.

## Code
- `justpdf-core/src/parser.rs` — `PdfDocument`, `open`, `from_bytes`, `open_mmap`, `resolve`, `authenticate`, `is_encrypted`, `load_object`, `load_object_raw`, `detect_encryption`, `LruCache`, `DEFAULT_CACHE_CAPACITY`
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
- 캐시 적중도 쓰기 락을 잡으므로 여러 스레드의 `resolve`가 직렬화된다(추론: 병렬 렌더의 병목 후보).
