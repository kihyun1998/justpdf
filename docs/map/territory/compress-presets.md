# 압축 프리셋과 노브

## What it is
`CompressOptions`의 10개 노브와, 그 조합에 이름을 붙인 네 프리셋(low/medium/high/extreme). 프리셋 이름은 core API, 브라우저 WASM, CLI, 그리고 외부 저장소 Just-pdf-web이 공유하는 **제품 약속**이다. 프리셋마다의 실제 값은 코드가 소유한다 — `CompressOptions::preset_*`를 읽는다(여기 복사하지 않는다).

## Governing decisions
**None.** [ADR-0002](../../adr/0002-language-bindings-outside-workspace.md)는 compress-wasm이 워크스페이스 안의 1급 제품이라는 것, [ADR-0003](../../adr/0003-cli-is-end-product-not-a-dependency-layer.md)은 압축이 CLI 서브커맨드라는 것을 정할 뿐, 프리셋 의미·노브 집합·표면 간 일치는 정하지 않는다.

## Design model
- 프리셋의 정체성은 테스트가 고정한다: `test_preset_low_identity` … `test_preset_extreme_identity`(#6에서 추가). 프리셋 값을 바꾸면 이 테스트가 먼저 깨져야 한다.
- **네 프리셋 모두 grayscale이 꺼져 있다.** grayscale은 `compress_advanced`(WASM)나 커스텀 옵션으로만 켜진다.
- 표면마다 노출하는 노브가 다르다:
  - WASM `compress`는 프리셋 이름, `compress_custom`은 품질·DPI만(나머지는 어떤 프리셋과도 다른 고정 조합), `compress_advanced`는 일부 노브만.
  - CLI는 `--preset` + 노브별 오버라이드(`resolve_options`). `remove_unused_resources`만 플래그가 없다.
- 기본 프리셋이 표면마다 다르다: CLI는 `"medium"`, `examples/compress_pdf.rs`는 `"high"`, Just-pdf-web은 high에 매핑된 Strong.
- `jpeg_quality`가 `None`이어도 `max_image_dpi`가 있으면 이미지는 q75로 재인코딩된다([compress-images](compress-images.md)). WASM README가 이 동작을 적고 있다.

## Code
- `justpdf-core/src/writer/compress.rs` — `CompressOptions`, `preset_low`, `preset_medium`, `preset_high`, `preset_extreme`, `from_preset`, `CompressStats`, `test_preset_low_identity`, `test_preset_extreme_identity`
- `justpdf-compress-wasm/src/lib.rs` — `compress`, `compress_custom`, `compress_advanced`
- `justpdf-cli/src/main.rs` — `cmd_compress`
- `justpdf-core/examples/compress_pdf.rs` — `main`

## Reference behaviour
**None.** `dev/pdf-compress-wasm-design.md` §3이 Ghostscript·qpdf 기법 표를 싣지만, 프리셋 결과를 Ghostscript 출력과 비교한 기록은 없다.

## Cross-cutting invariants
**None.** 표면 간 일치는 아래 블라스트 반경 체크리스트로 관리한다.

## Blast radius
프리셋 이름·의미·기본값을 바꿀 때의 체크리스트:
- [compress-wasm](compress-wasm.md) — `compress` 문서 주석, 크레이트 README, npm 재배포.
- [CLI](cli.md) — `--preset` 도움말, `cmd_compress`의 에러 문자열, `resolve_options`와 그 단위 테스트(프리셋 값을 직접 비교), `justpdf-cli/tests/compress.rs`의 프리셋 목록.
- 외부 저장소 Just-pdf-web(이 맵의 노드가 아님) — `STRENGTH_TO_WASM` 매핑, 프리셋 타입, 로케일 카피(현재 medium/extreme 설명이 실제 값과 다르다), 그 저장소의 `CONTEXT.md`.
- [compress-pipeline](compress-pipeline.md) — 노브가 어느 단계를 켜는지.
- `dev/pdf-compress-wasm-design.md` §4 표.

## Known holes / open
- `remove_unused_resources`에 CLI 플래그가 없다(#11은 병합으로 닫혔고, 이 누락을 정한 기록은 없다).
- 기본 프리셋 불일치(CLI medium vs 예제·웹 high)를 정한 기록이 없다.
- Tracked: #57 (jpeg_quality 절단), #58 (extreme의 첨부파일 제거)
