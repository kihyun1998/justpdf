# CLI (`justpdf` 바이너리)

## What it is
단 하나의 공식 PDF CLI. `info`·`text`·`render`·`merge`·`split`·`encrypt`·`decrypt`·`clean`·`compress`·`convert`·`sign` 서브커맨드가 core·render·formats를 직접 부른다. 태그 릴리스로 OS별 바이너리가 배포되는 유일한 제품이다.

## Governing decisions
- [ADR-0003](../../adr/0003-cli-is-end-product-not-a-dependency-layer.md) — CLI는 완제품이다. 새 기능은 별도 도구가 아니라 서브커맨드로 더하고, core+render+formats를 통째로 의존하며, 바이너리 크기 최소화를 추구하지 않는다.

## Design model
- 암호화 입력은 `open_doc`이 `--password`로 인증한다.
- `encrypt`는 항상 AES-128이고 `--no-print`·`--no-copy`만 노출한다. `serialize_pdf_encrypted`를 직접 부르며 고정 파일 ID를 쓴다([객체 암호화](object-encryption.md)).
- `clean`은 재빌드만 한다([정리(clean)](clean.md) 모듈을 부르지 않는다).
- `convert`에는 MOBI·FB2 분기가 없다 — CLI가 formats `all`을 켜지만 두 형식은 "unsupported input format"이 된다.
- `sign`은 "not yet fully implemented"를 출력하고 **성공 코드로 끝난다**.
- `compress`는 `--preset` 위에 노브별 오버라이드(`resolve_options`: 플래그가 이기고 미지정은 프리셋 유지, `--x`/`--no-x` 동시 지정은 에러), `--analyze`(아무것도 쓰지 않음), `--verbose`(stderr 상세), `--password`(복호화 후 재직렬화해서 압축, 출력은 **암호화 없음**)를 가진다. `remove_unused_resources`만 플래그가 없다.

## Code
- `justpdf-cli/src/main.rs` — `Commands`, `CompressArgs`, `resolve_options`, `open_doc`, `cmd_info`, `cmd_text`, `cmd_render`, `cmd_merge`, `cmd_split`, `cmd_encrypt`, `cmd_decrypt`, `cmd_clean`, `cmd_compress`, `cmd_convert`
- `justpdf-cli/tests/compress.rs` — `every_preset_produces_a_valid_smaller_pdf`, `conflicting_on_off_pair_is_rejected`, `analyze_needs_no_output_flag_and_writes_nothing`, `verbose_prints_breakdown_on_stderr`
- `justpdf-cli/tests/compress_encrypted.rs` — `compresses_encrypted_pdf_with_password_and_drops_encryption`, `wrong_password_is_rejected`

## Reference behaviour
**None.** 기능 범위의 example 참조는 MuPDF `mutool`이다(`docs/mupdf-feature-analysis.md`). 명령별 동작을 `mutool`과 비교한 기록은 없다.

## Cross-cutting invariants
**None.**

## Blast radius
- [compress-presets](compress-presets.md), [압축 파이프라인](compress-pipeline.md) — `compress`의 이름·출력.
- [텍스트 출력 형식](text-output-formats.md), [렌더 API](render-api.md), [SVG 렌더러](svg-renderer.md) — `text`·`render`.
- [포맷 감지](format-detection.md), [포맷 변환 계약](format-document.md) — `convert`.
- [문서 수정기](document-modifier.md), [파일 직렬화](file-serialization.md), [객체 암호화](object-encryption.md), [권한](permissions.md) — split/encrypt/decrypt/merge.
- [서명](signing.md) — `sign`이 연결되어야 할 곳.
- [릴리스](release.md) — 이 크레이트만 바이너리로 배포된다.
- [게시 문서](published-docs.md) — README·mdBook(`docs/src/cli.md`)·크레이트 README의 CLI 예제. 플래그를 바꾸면 세 곳을 본다.

## Known holes / open
- `--structural` 도움말이 "GC + dedup + object streams"라고 하지만 object stream 패킹은 꺼져 있다.
- "비암호화 PDF에 `--password`" 경로에 테스트가 없다(수동 실행으로는 정상).
- Tracked: #37 (서명 연결·CLI sign), #49 (convert MOBI·FB2)
