# 문서 빌더 (새 PDF 생성)

## What it is
빈 상태에서 PDF를 만든다. `DocumentBuilder`가 폰트·페이지·이미지·메타데이터·암호화를 모아 Pages/Catalog/Info(/Encrypt)를 만들고 직렬화한다. `PageBuilder`는 한 페이지의 콘텐츠 스트림(텍스트, 경로, 이미지 배치)을 연산자 텍스트로 조립한다. formats·special 크레이트의 모든 PDF 출력이 이 경로를 쓴다.

## Governing decisions
**None.**

## Design model
- 표준 폰트는 `/Encoding`도 임베드도 없는 Type1 사전이다(`add_standard_font`).
- `show_text`는 Rust 문자열의 UTF-8 바이트를 `(…) Tj`에 그대로 넣고 `( ) \`만 이스케이프한다 — [콘텐츠 텍스트 인코딩](../invariant/content-text-encoding.md).
- `set_title` 등 Info 값은 `as_bytes()`(BOM 없는 UTF-8)로 저장된다 — [텍스트 문자열 인코딩](../invariant/text-string-encoding.md).
- `set_font`/`draw_image`는 리소스 이름을 `/{}`로 이스케이프 없이 쓴다.
- `draw_inline_image`는 `BI … ID … EI`만 쓰고 `cm`을 쓰지 않는다. 이미지는 단위 정사각형에 매핑되므로 호출자가 `cm`을 앞에 써야 한다(추론: cbz/svg 입력은 이걸 하지 않는다).
- XMP 값은 XML 이스케이프 없이 들어간다(`set_xmp_metadata`).
- `embed_truetype_font`는 리소스 이름만 돌려준다. 폰트의 `IndirectRef`는 테스트 빌드 전용 `font_ref`(`#[cfg(test)]`)로만 얻을 수 있고, `PageBuilder`는 임베드 폰트를 `add_font_ref`(ref 필요)로만 리소스에 넣는다(`add_font`는 표준 Type1 인라인 사전을 만든다). 그래서 공개 API만으로는 임베드한 TrueType 폰트를 페이지에서 쓸 수 없다.
- 암호화 시 파일 ID는 `random_file_id`로 만든 16바이트이고 `/ID` 두 원소가 같다 — [객체 암호화](object-encryption.md). 암호화하지 않으면 `/ID`를 쓰지 않는다.

## Code
- `justpdf-core/src/writer/document.rs` — `DocumentBuilder`, `add_standard_font`, `embed_truetype_font`, `font_ref`, `set_title`, `set_xmp_metadata`, `set_encryption`, `build`, `save`, `embed_jpeg`, `embed_png`, `generate_tounicode_cmap`
- `justpdf-core/src/writer/page.rs` — `PageBuilder`, `show_text`, `set_font`, `draw_image`, `draw_inline_image`
- `justpdf-core/src/writer/encode.rs` — `make_stream`, `encode_flate`, `encode_flate_best`

## Reference behaviour
**None.**

## Cross-cutting invariants
- [콘텐츠 텍스트 인코딩](../invariant/content-text-encoding.md) — `show_text`가 대표 사이트.
- [텍스트 문자열 인코딩](../invariant/text-string-encoding.md) — Info 값 쓰기.
- [객체 구문 왕복](../invariant/object-syntax-roundtrip.md) — `PageBuilder`가 연산자 텍스트를 손으로 쓴다(이름 미이스케이프).

## Blast radius
- [파일 직렬화](file-serialization.md), [객체 암호화](object-encryption.md) — `build`의 끝.
- [포맷 변환 계약](format-document.md) 및 각 입력 포맷, [OCR](ocr.md) — 모든 `to_pdf`가 이 API를 쓴다. `show_text`·`draw_inline_image` 시그니처나 의미를 바꾸면 전부 확인한다.
- [CJK 폰트 임베딩](cjk-font-embedding.md), [ToUnicode](tounicode.md) — 임베드 폰트 쓰기 경로.
- [파사드](facade.md) — `DocumentBuilder`/`PageBuilder`를 재수출한다.

## Known holes / open
- 공개 API로는 임베드 TrueType 폰트를 페이지에 연결할 수 없다(위 Design model). Tracked: #65.
- 비 ASCII 텍스트를 올바르게 쓸 경로가 없다: 표준 폰트는 WinAnsi 계열이고, UTF-16BE 텍스트 문자열 인코더도 없다.
- 암호화하지 않으면 `/ID`를 쓰지 않는다. ISO 32000-1 §14.4는 "optional but should be used"이고, PDF 2.0에서 필수인지는 확인하지 않았다.
- Tracked: #29 (손으로 쓰는 구문 이스케이프), #33 (텍스트 문자열 인코딩), #34 (비 ASCII 콘텐츠 텍스트), #43 (인라인 이미지 cm 누락), #77 (비암호화 `/ID` 부재)
