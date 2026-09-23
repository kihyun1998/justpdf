# 압축 파이프라인

## What it is
`compress_pdf`가 문서를 열어 `DocumentModifier`로 풀고, 기법들을 정해진 순서로 적용한 뒤 GC하고 다시 쓴다. `analyze_pdf`는 압축 전에 페이지 수·이미지 수·이미지 바이트·암호화 여부를 보고한다. 단계 순서와 각 단계를 켜는 노브의 연결이 이 노트의 대상이다 — 개별 기법은 [압축 aggregate](compress.md)의 멤버 노트가 소유한다.

## Governing decisions
**None.**

## Design model
- **암호화 입력은 거부한다**("decrypt first"). 에러는 `StreamDecode` 변형을 빌려 쓴다(추론: 범주가 틀린 에러).
- **GC만 하고 `clean_objects`는 쓰지 않는다**: 번호 재매김이 catalog_ref를 무효화하기 때문(단계 4 주석).
- **object stream 단계(5)는 꺼져 있다**: "xref stream implementation has compatibility issues with some PDF viewers". [object streams](object-streams.md)의 쓰기 쪽이 여기서 멈춘다.
- 단계 순서의 소유자는 `compress_pdf` 본문이다. 단계를 추가·이동할 때는 앞 단계 출력이 뒤 단계 입력이라는 점을 본다([aggregate](compress.md#why-they-sit-together)).

## Code
- `justpdf-core/src/writer/compress.rs` — `compress_pdf`, `analyze_pdf`, `AnalyzeResult`, `is_image_xobject`, `pack_into_object_streams`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- 멤버 기법 전부 — [이미지](compress-images.md), [그레이스케일](compress-grayscale.md), [폰트 서브세팅](font-subsetting.md), [스트림 재압축](compress-stream-recompression.md), [dedup](compress-dedup.md), [미사용 리소스](compress-unused-resources.md), [제거](compress-stripping.md).
- [프리셋](compress-presets.md) — 노브 → 단계 연결.
- [문서 수정기](document-modifier.md) — `from_document`/`garbage_collect`/`build`.
- [문서 접근](document-access.md) — `is_encrypted` 판단.
- [compress-wasm](compress-wasm.md), [CLI](cli.md) — `CompressStats`·`AnalyzeResult` 필드를 그대로 노출한다. 필드 추가·삭제는 두 표면의 getter/출력에 닿는다.

## Known holes / open
- core는 여전히 암호화 입력을 거부한다. CLI `--password`는 인증 후 `DocumentModifier`로 재직렬화한 바이트를 넘기므로, 보고되는 원본 크기가 파일 크기가 아니라 복호화·재직렬화된 크기다(추론). R5 파일은 틀린 비밀번호도 통과한다(#30).
- `justpdf-core/tests/`에는 `compress_pdf` 통합 테스트가 없다(단위 테스트는 `compress.rs` 안에만).
