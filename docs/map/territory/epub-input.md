# EPUB 입력

## What it is
EPUB의 OPF·XHTML에서 텍스트를 읽어 PDF로 바꾼다. DRM 파일은 거부한다.

## Governing decisions
- [ADR-0001](../../adr/0001-crates-split-by-dependency-layer.md) — `epub`은 render 없이 빌드된다고 간주되는 기능이다.

## Design model
- **`epub` 기능 단독으로는 컴파일되지 않는다**: `render_page`·`render_page_png`가 `crate::plaintext`를 부르는데 `plaintext` 모듈은 `plaintext` 기능 뒤에 있다. `cargo check -p justpdf-formats --no-default-features --features epub`가 `E0433: cannot find plaintext in crate`로 실패한다(2026-09-23 재현). 워크스페이스 빌드에서는 CLI가 켜는 `all`이 기능 통합으로 이를 가린다.
- 텍스트만 옮긴다.

## Code
- `justpdf-formats/src/epub/mod.rs` — `EpubDocument`, `to_pdf`, `render_page`, `render_page_png`

## Reference behaviour
**None.**

## Cross-cutting invariants
- [콘텐츠 텍스트 인코딩](../invariant/content-text-encoding.md)

## Blast radius
- [plaintext 입력](plaintext-input.md) — 미리보기를 빌려 쓰는 대상. 숨은 의존이다.
- [크레이트 레이어링](crate-layering.md) — 격리 스크립트는 epub을 `cargo tree`로만 확인하고 단독 빌드는 하지 않는다.
- [포맷 변환 계약](format-document.md).

## Known holes / open
- 단독 빌드 실패(위). #2의 수락 기준은 기능별 단독 빌드를 요구했지만 스크립트가 그것을 검사하지 않는다.
- Tracked: #35 (epub·office 단독 빌드)
