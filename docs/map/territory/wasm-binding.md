# 범용 WASM 바인딩

## What it is
wasm-bindgen으로 `WasmDocument`(생성자·인증·텍스트·PNG 렌더·크기·Info)를 노출한다. 압축 전용 [compress-wasm](compress-wasm.md)과 다른 크레이트다.

## Governing decisions
- [ADR-0002](../../adr/0002-language-bindings-outside-workspace.md) — 워크스페이스 밖이어야 하지만 `[workspace]` 블록이 없어 현재 cargo가 매니페스트를 해석하지 못한다([aggregate](language-bindings.md#adr-0002와-저장소가-어긋나는-지점)).

## Design model
- `js_name` 변경이 없어 JS 쪽 클래스 이름이 `WasmDocument`다.

## Code
- `justpdf-wasm/src/lib.rs` — `WasmDocument`, `new`, `authenticate`, `text`, `page_text`, `render_page_png`, `page_width`, `page_height`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [문서 접근](document-access.md), [텍스트 추출](text-extraction.md), [렌더 API](render-api.md).
- [게시 문서](published-docs.md) — README·mdBook 예제가 `WasmDocument`와 snake_case 메서드 이름을 쓴다(`js_name` 변경 시 함께 고친다).
- [CI](ci.md) — 빌드되지 않는다.

## Known holes / open
- 단독 빌드 불가(위).
- Tracked: #36 (justpdf-wasm 매니페스트·ADR-0002)
