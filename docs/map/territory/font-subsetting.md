# 폰트 서브세팅

## What it is
임베드된 TrueType 폰트(FontFile2)에서 문서가 실제로 쓰는 글리프만 남기고 테이블을 다시 조립한다. core의 `subset_font`가 폰트 바이너리를 다루고, 압축 쪽 `subset_embedded_fonts`가 문서에서 쓰인 코드를 모아 서브셋을 만들고 `/Widths`·`/CIDToGIDMap`을 고친다. 유일한 소비처는 압축이다.

## Governing decisions
**None.**

## Design model
- `subset_font`는 CFF/`OTTO` 폰트를 거부하고 `.notdef`를 남기며, **GID를 압축 번호로 다시 매긴다**(`gid_map`: 옛 GID → 새 GID). `cmap` 테이블은 그대로 복사한다.
- 단순 TrueType은 "char code ≈ glyph ID"를 가정하고 `/Widths`를 문자 코드 기준으로 유지한다.
- CID 폰트의 `/CIDToGIDMap`이 `Identity`면 조기 반환한다("GIDs already remapped in font") — 하지만 콘텐츠 스트림의 CID는 다시 매겨지지 않는다.
- CID 폰트의 `/W`는 건드리지 않는다.
- 위 세 가지를 합치면, 서브셋 뒤 문자가 엉뚱한 글리프를 가리킨다. **단순 TrueType에서 재현됨**(2026-09-23, Noto Sans로 "Hello"를 `preset_medium` 압축): `fonts_subsetted = 1`, 텍스트 추출은 "Hello" 그대로인데, 서브셋 폰트는 3884 → 6 글리프이고 복사된 `cmap`이 여전히 옛 GID 43/72/79/82를 가리켜 다섯 글자 모두 아웃라인이 없다. 원인은 "char code ≈ glyph ID" 가정 — 코드 72('H')의 GID는 43이다. CID 경로는 여전히 추론이다(CID 픽스처 없음).
- 압축 테스트의 판정 기준인 `extract_page_text_string`은 ToUnicode만 보므로 이 오류를 잡지 못한다. 그래서 글리프를 보는 테스트는 따로 있다: `test_subset_truetype_font_keeps_glyph_outlines`는 각 문자를 폰트 `cmap`으로 찾아 원본과 아웃라인을 비교한다(ttf-parser가 판정자 — 서브셋 코드와 독립). #51이 고쳐지기 전까지 `#[ignore]`이며, `--ignored`로 돌리면 빨간불이다.
- **픽스처** `justpdf-core/tests/fixtures/NotoSans-Regular.ttf`: notofonts/notofonts.github.io 커밋 `28b15b4b43b7bed62b5cf6e6b0b5ff5846270535`의 `fonts/NotoSans/unhinted/ttf/NotoSans-Regular.ttf`를 수정 없이 가져왔다(431,364 B, sha256 `f3961a9cde016d41a4879aecda1474d3a36d6bf54fa0e4643de029cc2248b0e8`, `test_truetype_fixture_is_pinned`가 고정). 라이선스는 같은 커밋의 `fonts/LICENSE`(SIL OFL 1.1)를 `LICENSE-NotoSans`로 동봉. `glyf` 아웃라인이라 `subset_font`가 받는다.
  - **메인테이너 판단(2026-09-23)**: 수정 없는 원본(431KB)을 택했다. 검토한 대안은 fonttools로 ASCII만 남긴 사전 서브셋(수십 KB, 수정본이라 재현 절차 기록 필요)과 DejaVu Sans(~750KB, OFL 아님). 판단 근거로 제시된 사실: 기존 픽스처 전체가 36KB, `justpdf-core/Cargo.toml`에 `include`/`exclude`가 없어 이 파일이 crates.io 배포본에 실린다. 이 판단은 배포본 포함 여부 자체는 다루지 않았다(Tracked: #66).

## Code
- `justpdf-core/src/font/subset.rs` — `subset_font`, `SubsetResult`, `KEPT_TABLES`, `build_subset_hmtx`, `rewrite_composite_glyph_ids`, `test_gid_mapping_correctness`
- `justpdf-core/src/writer/compress.rs` — `subset_embedded_fonts`, `update_font_widths`, `update_cid_to_gid_map`, `extract_font_map`, `extract_char_codes`, `load_cid_to_gid_map`, `is_cid_font`, `find_fontfile2`, `create_pdf_with_truetype_font`, `test_subset_truetype_font_actually_subsets`, `test_subset_truetype_font_keeps_glyph_outlines`

## Reference behaviour
**None.**

## Cross-cutting invariants
- [폰트 해석 경로](../invariant/font-resolution.md) — 서브셋 결과를 렌더러와 텍스트 추출이 각자 다른 방식으로 해석한다.

## Blast radius
- [폰트 로딩](font-loading.md) — `/Widths` 인덱싱 규칙(`FontWidths::Simple`)을 공유한다.
- [CID 폰트](cid-fonts.md) — `/W`·`/CIDToGIDMap` 해석.
- [글리프 렌더링](glyph-rendering.md) — `char_code_to_glyph_id`가 복사된 `cmap`으로 옛 GID를 찾는다.
- [텍스트 추출](text-extraction.md) — 압축 테스트의 판정자.
- [압축 파이프라인](compress-pipeline.md), [프리셋](compress-presets.md) — `font_subsetting` 노브.

## Known holes / open
- 단순 TrueType 서브셋이 그려지는 글리프를 잃는다(재현됨, 위). Tracked: #51.
- 임베드 CID(Type0/CIDFontType2) 폰트 픽스처가 없다 — CID 경로의 서브세팅은 어떤 테스트에서도 실행되지 않는다.
- CFF(FontFile3)는 서브세팅 대상이 아니다.
- Tracked: #51 (서브세팅 GID 매핑)
