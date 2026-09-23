# 렌더 — 이미지

## What it is
이미지 XObject·이미지 마스크·SMask·명시적 마스크·인라인 이미지를 픽스맵에 그린다. XObject 해석(`resolve_xobject`)과 `Do` 분기가 여기서 이미지와 Form으로 갈린다.

## Governing decisions
**None.**

## Design model
- 디코드는 core `decode_image` 하나를 쓰고, 그 뒤 자체 `image_to_rgba`로 해석한다: 성분 1/3/4만, 항상 8비트, 단순 CMYK — [이미지 픽셀 레이아웃](../invariant/image-pixel-layout.md).
- 이미지 마스크는 결과를 1비트 패킹으로 풀어 읽는데, CCITT·JBIG2 경로는 픽셀당 1바이트를 돌려준다(추론: 불일치).
- **SMask·명시적 마스크는 두 번 디코드된다**: `self.doc.decode_stream`으로 푼 뒤 그 결과를 다시 `decode_image`에 같은 사전과 함께 넘긴다. Flate 마스크는 두 번째 디코드가 실패해 마스크가 조용히 빠진다(추론).
- **인라인 이미지는 그리지 않는다**: `render_inline_image` 본문이 `// TODO`와 `Ok(())`뿐이다.
- `resolve_xobject`는 DCTDecode를 원바이트로, 그 외는 `decode_stream`으로 넘긴다.

## Code
- `justpdf-render/src/interpreter.rs` — `do_xobject`, `resolve_xobject`, `render_image`, `render_image_mask`, `apply_image_smask`, `apply_image_explicit_mask`, `render_inline_image`, `image_to_rgba`

## Reference behaviour
**None.** 비교 대상 조항: ISO 32000-2 §8.9, §8.9.6(마스크), §11.6.5.3.

## Cross-cutting invariants
- [이미지 픽셀 레이아웃](../invariant/image-pixel-layout.md)

## Blast radius
- [이미지 디코딩](image-decoding.md) — 입력. 출력 형태가 바뀌면 `image_to_rgba`와 마스크 풀이를 본다.
- [스트림 필터](stream-filters.md) — 이중 디코드의 원인(통과 규칙).
- [투명도](render-transparency.md) — SMask 적용.
- [SVG 렌더러](svg-renderer.md), [compress-images](compress-images.md) — 같은 디코드 출력의 다른 해석자.
- [문서 빌더](document-builder.md) — `draw_inline_image`로 만든 PDF(cbz/svg 입력)가 여기서 그려지지 않는다.

## Known holes / open
- 인라인 이미지 미렌더(위). 인라인 이미지는 파싱만 된다.
- Tracked: #42 (렌더 인라인 이미지·마스크)
