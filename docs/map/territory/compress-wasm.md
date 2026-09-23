# compress-wasm (브라우저 압축 제품)

## What it is
브라우저에서 서버 없이 PDF를 압축하는 WASM 모듈. core의 압축 API만 노출하고 렌더러를 끌어오지 않는다. npm 패키지로 배포되며, 외부 저장소 Just-pdf-web이 워커에서 `analyze`와 `compress(bytes, preset)`를 부른다.

## Governing decisions
- [ADR-0002](../../adr/0002-language-bindings-outside-workspace.md) — 일반 바인딩과 달리 "완성된 제품"이므로 워크스페이스 **안**에 두고 core와 CI·테스트·버전을 함께 끈다.
- [ADR-0001](../../adr/0001-crates-split-by-dependency-layer.md) — render를 빼서 번들을 작게 한다.

## Design model
- 표면: `compress`(프리셋), `compress_custom`(품질·DPI, 나머지 고정), `compress_advanced`(일부 노브), `analyze`. 결과 getter가 `CompressStats`·`AnalyzeResult` 필드를 1:1로 노출한다.
- `compress_advanced`의 `jpeg_quality: i32`는 `as u8`로 잘린다.
- `getrandom`의 `js` 기능은 코드가 쓰지 않는다. `rsa` → `rand_core`의 전이 의존이 wasm32에서 빌드되게 하려는 것이다(설계 문서 I-3).
- `pkg/`(빌드 산출물)는 git에 추적되지 않는다.

## Code
- `justpdf-compress-wasm/src/lib.rs` — `compress`, `compress_custom`, `compress_advanced`, `analyze`, `CompressResult`, `AnalyzeResult`
- `justpdf-compress-wasm/Cargo.toml` — `getrandom`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [compress-presets](compress-presets.md) — 프리셋 이름·의미.
- [압축 파이프라인](compress-pipeline.md) — 통계 필드 추가·삭제가 getter에 닿는다.
- [크레이트 배포](crate-publishing.md) — core에 `path`만 있고 `version`이 없는 의존성이라 crates.io 게시가 막힌다(추론, `cargo publish` 미실행). npm 게시 경로는 저장소 워크플로에 없다.
- 외부 저장소 Just-pdf-web — getter 이름·프리셋 이름 변경이 그쪽 워커·타입을 깬다.

## Known holes / open
- `compress_advanced`에서 `jpeg_quality`를 0으로 줘도 `max_dpi > 0`이면 q75로 재인코딩된다(README에 적혀 있음). 노브로 끌 수 있는 것은 DPI 쪽뿐이다.
- Tracked: #57 (jpeg_quality 절단), #58 (extreme의 첨부파일 제거)
