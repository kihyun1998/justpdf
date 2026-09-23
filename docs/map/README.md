# justpdf 맵

<!-- grill-map build stamp: eb0cd67 -->

이 맵은 두 질문에 답한다.

1. **수평 — "이걸 건드리면 또 무엇이 움직이나?"** 건드리는 코드가 속한 territory 노트를 열고 `## Blast radius`를 **체크리스트로** 따라간다(열어 보고 할 일이 없으면 그것도 정상). 이어서 `## Cross-cutting invariants`의 불변식 노트를 열어 `## Where it will recur`를 자기 변경에 대어 본다.
2. **수직 — "이 코드는 어떤 설계·결정에서 나왔나?"** 같은 노트의 `## Governing decisions`(왜)와 `## Design model`(규칙)을 본다. `**None.**`이면 그 공백 자체가 답이다: 아무도 정하지 않았다.

## 읽기 규약

- **작업 전에 읽는다.** 설계를 정하기 전에 해당 territory와 그 불변식을 연다. 변경을 다 만든 뒤에 여는 것은 이 맵의 목적을 뒤집는다.
- **작업 후에 쓴다.** 변경이 끝나면 두 가지를 확인한다.
  1. *Coverage* — 건드린 territory의 노트가 있고, `## Blast radius`가 여전히 맞는가? `## Code`의 심볼이 여전히 그 파일에 있는가?
  2. *Promotion* — **이번 수정이 드러낸 사실이 이 territory 밖에서도 참인가?** 예: "이 함수가 PDF 구문 바이트를 `PdfObject` Display 밖에서 만드는가?", "이 함수가 사람이 읽을 텍스트를 `PdfObject::String`에 넣는가?" — grep으로 답할 수 있다. 참이면 불변식 노트가 생기기(또는 사이트가 추가되기) 전에는 수정이 끝난 것이 아니다.
- 노트에 이슈를 인용할 때는 **사실을 먼저 쓰고 이슈는 추적 포인터로** 붙인다("X는 Y를 못 한다. Tracked: #N"). 이슈가 닫히는 순간 거짓이 되는 문장("#N이 열려 있다")은 쓰지 않는다.

## 이 층이 필요한 이유

같은 사실 — **"justpdf가 쓰는 PDF 구문은 justpdf 파서로 되읽어 같은 값이어야 한다"** — 이 세 번 따로 발견되고 세 번 모두 중심 사이트 한 곳만 고쳐졌다: 0be6cc7(Name 공백 → 한글 텍스트 소실), 0be6cc7(String 괄호), 7be23c0/#20(String 고바이트 → 특정 비밀번호로 암호화한 파일이 열리지 않음). 같은 규칙을 적용받아야 할 손으로 쓰는 사이트가 여덟 군데 더 있었고, 이 맵을 쓰는 중에야 목록이 됐다 — [객체 구문 왕복](invariant/object-syntax-roundtrip.md). 네 번째 발견을 막는 것이 이 맵의 합격 기준이다.

## 측정 (2026-09-23)

- **공개 표면 vs 결정 기록.** 워크스페이스 크레이트의 `pub fn`은 수백 개(core가 대부분)이고, 결정 기록 3개(ADR-0001~0003)는 모두 **크레이트 배치**를 다룬다. API의 동작을 정한 기록은 0개다. territory 중 결정 기록이 있는 것은 크레이트 경계에 걸친 노트뿐이다(명령은 아래).
- **언급 vs 주제.** "compress"는 ADR 본문 3개에 언급되고 어느 것의 주제도 아니다 — grep상으로는 "다뤄진" 것처럼 보이는 최악의 상태다.
- **파일 크기.** `writer/compress.rs`(저장소 최대 파일)와 `justpdf-render/src/interpreter.rs`(두 번째)는 각각 코드 트리보다 잘게 나눴다: 압축 9개 + [aggregate](territory/compress.md), 렌더 인터프리터는 이미지·클리핑·투명도·셰이딩·패턴·글리프·주석·OCG로.
- **앞을 보는 문장 vs 실제.** 옛 `roadmap.md`는 체크박스가 전부 완료였지만 코드에는 인라인 이미지 렌더 no-op, Type 0 함수 부재, 타일 렌더링 미연결, `/Properties` OCG 조회 TODO가 있었다. CHANGELOG와 압축 설계 문서는 꺼진 object stream 패킹을 완료로 적었다. **낡은 예측**(설계대로 되어 있다고 말하는 문서)이 낡은 포인터보다 위험하다 — 그 문서를 보고 계획하면 없는 것을 있다고 가정한다. 2026-09-23에 roadmap은 삭제했고 나머지는 사실에 맞췄다.
- **백로그.** 맵 작성 전 열린 이슈는 모두 압축 라인이었다. 맵 작성 중 드러난 결함은 이슈로 등록했고(#25–#60), 노트의 `## Known holes / open`에 추적 포인터로 붙였다.
- **기존 문서가 틀린 곳**(이 맵은 결정 기록을 고치지 않는다): [ADR-0002](../adr/0002-language-bindings-outside-workspace.md)는 네 바인딩이 모두 워크스페이스 밖이라고 적지만 `justpdf-ffi`는 루트 `members`에 있고, `justpdf-wasm`은 `[workspace]` 블록이 없어 cargo가 매니페스트를 해석하지 못한다([언어 바인딩](territory/language-bindings.md)). README·mdBook 예제는 같은 날 코드에 맞췄다([게시 문서](territory/published-docs.md)).

## 규약

- territory는 **겹친다**. 한 파일이 여러 territory에 나올 수 있고, 한 사실이 여러 곳에서 참이면 불변식 노드가 된다.
- **빈 섹션을 지우지 않는다.** 비어 있음은 `**None.**` 센티널로 쓰고, 이어서 왜 비었는지(인접한 무엇이 이 영역을 다스리지 않는지)를 적는다. 세 센티널은 서로 다른 구멍이다: *아무도 정하지 않음*(Governing decisions), *아무도 비교하지 않음*(Reference behaviour), *아무도 만들지 않음*(Code).
- `## Code`는 **심볼 이름**만 적는다. 줄 번호는 쓰지 않는다.
- 링크는 상대 경로 마크다운 링크(Obsidian 그래프와 GitHub 양쪽에서 동작).
- 헤딩은 영어 고정 문자열이고 본문은 한국어다. 헤딩 문자열은 아래 명령들이 grep하므로 한 글자도 바꾸지 않는다. (저장소에 문서 언어 규칙이 없어, 결정 기록·이슈가 쓰는 한국어를 본문에 따랐다.)
- 사이트 목록은 **도구가 볼 수 있는 절반**(grep 명령을 함께 적음)과 **도구가 볼 수 없는 절반**(호출이 아니라 가정인 사이트, 손으로 유지)을 나눠 적는다.
- `(추론)`은 코드 읽기에서 나온 결론이며 실행으로 확인하지 않았다는 표시다.
- 참조 소스: ISO 32000-2는 **binding**, MuPDF·Ghostscript는 **example**이다. `## Reference behaviour`의 조항 번호는 비교 대상 포인터일 뿐이며, 원문과 대조한 기록이 생기기 전까지 모든 노트가 `**None.**`이다.

## 이 맵이 답하지 못하는 것

- 노드는 `.md` 파일뿐이다. 이슈·PR·소스 파일은 노트 안의 텍스트로만 있고, 그래프 뷰에 나타나지 않는다. 가장 뜨거운 곳(열린 이슈)이 그래프에서 보이지 않는다.
- 외부 저장소 Just-pdf-web(compress-wasm 소비자)은 노드가 아니다. [compress-presets](territory/compress-presets.md)의 체크리스트에 텍스트로만 있다.
- 불변식 7개 중 6개는 기록된 사고 없이 한 번의 읽기에서 발견됐다. `## Discovery history`가 그렇게 적고 있다 — 재발견 이력이 쌓이기 전까지는 예측이다.

## 범위와 부재의 의미

**저장소 전체**를 덮는다: 워크스페이스 크레이트 전부, 워크스페이스 밖 바인딩 넷, CI·릴리스·배포·게시 문서.

- **코드가 있는데 노트가 없다 → 구멍이다.** 이 맵 이후에 생긴 모듈·크레이트·기능이다. 노트가 **빚진 상태**가 되는 조건: 새 모듈의 첫 조각이 master에 병합될 때.
- **계획만 있고 코드가 없다 → 노트가 없는 것이 정상이다.** 계획의 목록은 이슈 트래커가 소유한다.
- 한 파일이 여러 개념을 담으면(예: `compress.rs`, `interpreter.rs`) 노트가 파일보다 잘다. 파일 이름으로 노트를 찾지 말고 개념 이름으로 찾는다.

## 게이트

`python3 scripts/check-map.py .` (저장소 루트에서) — 노트 하나만 보려면 뒤에 경로를 붙인다. 검사 항목: 섹션 집합(territory·invariant·aggregate별), `## Code` 심볼이 적힌 파일에 있는지, 링크와 `#앵커`가 해석되는지(코드 스팬·펜스 블록 안은 무시), 불변식 ↔ territory 상호 링크.

범위는 `docs/map/` 아래뿐이다. CI에 연결되어 있지 않으므로 통과는 "누가 돌렸다"는 뜻일 뿐이다. 노트의 **주장**(설계 규칙, 추론, 블라스트 간선)은 검사하지 않는다 — 리팩터 뒤에는 심볼 검사로 주소를 고치고, 심볼이 **사라졌으면** 그 노트의 `## Design model`부터 다시 읽는다.

## 질문을 명령으로

```sh
cd docs/map
# 아무도 정하지 않은 territory (센티널은 섹션별로 한정해야 한다 — 같은 센티널이 여러 섹션에 있다)
rg -lU '## Governing decisions\r?\n\*\*None\.\*\*' territory/
# 결정 기록이 있는 territory (aggregate 노트는 이 섹션이 없으므로 기준 집합에서 뺀다)
comm -23 <(rg -l '^## Governing decisions' territory/ | sort) <(rg -lU '## Governing decisions\r?\n\*\*None\.\*\*' territory/ | sort)
# 참조와 비교한 적 없는 territory
rg -lU '## Reference behaviour\r?\n\*\*None\.\*\*' territory/
# 코드가 없는 territory (설계만 있음)
rg -lU '## Code\r?\n\*\*None\.\*\*' territory/
# 어떤 territory가 이 불변식을 주장하나 (## Cross-cutting invariants 섹션 안의 링크만 — 본문 언급은 세지 않는다)
rg -lUP '## Cross-cutting invariants\n(?:(?!## ).*\n)*?.*invariant/object-syntax-roundtrip\.md' territory/
# 노드 목록 — 폴더가 목록이다
ls territory/ invariant/
```

## 노드
- core — 읽기: [tokenizer](territory/tokenizer.md) · [object-model](territory/object-model.md) · [xref](territory/xref.md) · [object-streams](territory/object-streams.md) · [document-access](territory/document-access.md) · [repair](territory/repair.md) · [linearization](territory/linearization.md) · [page-tree](territory/page-tree.md)
- core — 쓰기: [object-serialization](territory/object-serialization.md) · [file-serialization](territory/file-serialization.md) · [document-builder](territory/document-builder.md) · [document-modifier](territory/document-modifier.md) · [incremental-save](territory/incremental-save.md) · [clean](territory/clean.md) · [journal](territory/journal.md)
- 압축: [compress](territory/compress.md) · [compress-presets](territory/compress-presets.md) · [compress-pipeline](territory/compress-pipeline.md) · [compress-images](territory/compress-images.md) · [compress-grayscale](territory/compress-grayscale.md) · [font-subsetting](territory/font-subsetting.md) · [compress-stream-recompression](territory/compress-stream-recompression.md) · [compress-dedup](territory/compress-dedup.md) · [compress-unused-resources](territory/compress-unused-resources.md) · [compress-stripping](territory/compress-stripping.md)
- 스트림·이미지·색: [stream-filters](territory/stream-filters.md) · [image-decoding](territory/image-decoding.md) · [color-spaces](territory/color-spaces.md) · [pdf-functions](territory/pdf-functions.md)
- 폰트·텍스트: [font-loading](territory/font-loading.md) · [font-encodings](territory/font-encodings.md) · [tounicode](territory/tounicode.md) · [cid-fonts](territory/cid-fonts.md) · [cjk-font-embedding](territory/cjk-font-embedding.md) · [cff](territory/cff.md) · [opentype-layout](territory/opentype-layout.md) · [type3-fonts](territory/type3-fonts.md) · [font-recovery](territory/font-recovery.md) · [content-stream-parsing](territory/content-stream-parsing.md) · [text-extraction](territory/text-extraction.md) · [reading-order](territory/reading-order.md) · [text-output-formats](territory/text-output-formats.md) · [text-search](territory/text-search.md) · [text-wrapping](territory/text-wrapping.md)
- 암호화·서명: [encryption-model](territory/encryption-model.md) · [key-derivation](territory/key-derivation.md) · [password-authentication](territory/password-authentication.md) · [object-decryption](territory/object-decryption.md) · [object-encryption](territory/object-encryption.md) · [permissions](territory/permissions.md) · [signature-detection](territory/signature-detection.md) · [signing](territory/signing.md) · [signature-verification](territory/signature-verification.md) · [timestamps](territory/timestamps.md) · [signature-appearance](territory/signature-appearance.md)
- 대화형 기능: [annotations](territory/annotations.md) · [annotation-appearance](territory/annotation-appearance.md) · [redaction](territory/redaction.md) · [acroform](territory/acroform.md) · [form-fill](territory/form-fill.md) · [form-flatten](territory/form-flatten.md) · [form-appearance](territory/form-appearance.md) · [actions](territory/actions.md) · [outlines](territory/outlines.md) · [optional-content](territory/optional-content.md) · [page-labels](territory/page-labels.md) · [embedded-files](territory/embedded-files.md)
- 렌더: [render-interpreter](territory/render-interpreter.md) · [raster-device](territory/raster-device.md) · [svg-renderer](territory/svg-renderer.md) · [bbox-device](territory/bbox-device.md) · [display-list](territory/display-list.md) · [glyph-rendering](territory/glyph-rendering.md) · [render-shading](territory/render-shading.md) · [render-tiling-patterns](territory/render-tiling-patterns.md) · [render-images](territory/render-images.md) · [render-clipping](territory/render-clipping.md) · [render-transparency](territory/render-transparency.md) · [render-annotations](territory/render-annotations.md) · [render-api](territory/render-api.md)
- 제품: [facade](territory/facade.md) · [cli](territory/cli.md) · [compress-wasm](territory/compress-wasm.md) · [format-document](territory/format-document.md) · [format-detection](territory/format-detection.md) · [xps-input](territory/xps-input.md) · [epub-input](territory/epub-input.md) · [office-input](territory/office-input.md) · [svg-input](territory/svg-input.md) · [cbz-input](territory/cbz-input.md) · [mobi-input](territory/mobi-input.md) · [fb2-input](territory/fb2-input.md) · [plaintext-input](territory/plaintext-input.md) · [ocr](territory/ocr.md) · [barcode](territory/barcode.md) · [zugferd](territory/zugferd.md) · [bidi](territory/bidi.md) · [deskew](territory/deskew.md) · [language-bindings](territory/language-bindings.md) · [ffi-binding](territory/ffi-binding.md) · [python-binding](territory/python-binding.md) · [wasm-binding](territory/wasm-binding.md) · [node-binding](territory/node-binding.md)
- 인프라: [crate-layering](territory/crate-layering.md) · [ci](territory/ci.md) · [release](territory/release.md) · [crate-publishing](territory/crate-publishing.md) · [published-docs](territory/published-docs.md)
- 불변식: [content-text-encoding](invariant/content-text-encoding.md) · [font-resolution](invariant/font-resolution.md) · [image-pixel-layout](invariant/image-pixel-layout.md) · [incremental-trailer](invariant/incremental-trailer.md) · [object-syntax-roundtrip](invariant/object-syntax-roundtrip.md) · [page-content-assembly](invariant/page-content-assembly.md) · [text-string-encoding](invariant/text-string-encoding.md)
