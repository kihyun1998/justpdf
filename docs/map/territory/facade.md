# 고수준 파사드 (`justpdf` 크레이트)

## What it is
`justpdf-core`와 `justpdf-render`를 감싼 사용자용 API: `Document`(열기·인증·페이지·메타데이터·텍스트·검색·아웃라인·주석·폼·첨부·서명 감지·렌더), `Page`, `Modifier`, `merge`. crates.io에서 `justpdf`라는 이름으로 소비되는 입구다.

## Governing decisions
- [ADR-0001](../../adr/0001-crates-split-by-dependency-layer.md) — 이 크레이트는 core + render만 통합하고 formats/special을 **의식적으로 제외**한다.

## Design model
- 저장소 안의 어떤 크레이트도 `justpdf`에 의존하지 않는다. 네 바인딩과 CLI는 모두 core/render를 직접 쓴다.
- `Metadata` 타입은 없다 — 메타데이터는 문자열 getter와 `metadata() -> Vec<(String,String)>`.
- `Page::render_svg`·`render_raw`는 페이지를 인덱스로 다시 모으고, 다른 렌더 메서드는 캐시된 `PageInfo`를 넘긴다(불일치).
- `open`·`from_bytes`·`open_mmap`은 생성 중에 페이지를 모으므로, 사용자 비밀번호가 있는 암호화 파일에서는 `Core(EncryptedDocument)`로 실패한다. 그런 파일은 `*_with_password` 생성자로 연다 — core `authenticate` 뒤 페이지를 모은다. 빈 사용자 비밀번호로 열리는 문서는 core `authenticate`가 "이미 인증됨"으로 돌아오므로 어떤 비밀번호로도 열린다. 파사드 `authenticate`는 빈 사용자 비밀번호 파일에서만 닿는다(#99, 메인테이너가 비밀번호 생성자 추가안을 택함 — 지연 페이지 수집안은 인증 전 `page_count()`가 0이 되는 의미 변경이라 택하지 않음).
- `Document::modify`는 `&self.inner`를 `DocumentModifier::from_document`에 넘긴다 — 인증 상태가 그대로 쓰인다. `from_document`가 `encryption: None`으로 시작하므로, 암호화 문서를 `modify()`로 저장하면 `inner_mut().set_encryption`을 부르지 않는 한 `/Encrypt` 없는 파일이 나온다(2026-09-28 측정, `aes256_r5_empty_user.pdf`). 파사드 `Modifier` 자신에는 암호화 메서드가 없다.
- 기능 플래그: `mmap`(core), `parallel`(render), `async`(tokio — 읽기만 비동기, 파싱은 동기). `arena`는 core로 전달되지만 파사드 코드는 이것으로 아무것도 게이트하지 않는다.

## Code
- `justpdf/src/lib.rs` — `Document`, `open`, `from_bytes`, `open_mmap`, `open_with_password`, `from_bytes_with_password`, `open_mmap_with_password`, `authenticated`, `authenticate`, `pages`, `PageIter`, `metadata`, `text`, `search`, `outlines`, `annotations`, `form_fields`, `embedded_files`, `signatures`, `modify`, `Page`, `render_png`, `render_svg`, `render_raw`, `Modifier`, `merge`, `merge_bytes`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [문서 접근](document-access.md), [페이지 트리](page-tree.md), [텍스트 추출](text-extraction.md), [텍스트 검색](text-search.md), [렌더 API](render-api.md), [문서 수정기](document-modifier.md) — 래핑 대상. 하위 시그니처 변경이 여기서 멈추는지 새어 나가는지 본다.
- [아웃라인](outlines.md), [주석](annotations.md), [AcroForm](acroform.md), [첨부파일](embedded-files.md), [서명 감지](signature-detection.md), [페이지 레이블](page-labels.md), [선형화](linearization.md) — 읽기 전용 노출.
- [크레이트 레이어링](crate-layering.md) — 의존성 구성 원칙.
- [게시 문서](published-docs.md) — README·mdBook의 Rust 예제가 이 API를 부른다. 시그니처를 바꾸면 예제를 다시 컴파일해 본다(게이트 없음).

## Known holes / open
- mmap·parallel·async 기능 테스트가 없다. CI 전체 기능 실행에 `mmap`이 빠져 있다.
