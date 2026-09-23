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
- 위 세 가지를 합치면, 서브셋 뒤 문자가 엉뚱한 글리프를 가리킬 수 있다(추론, 테스트 없음). 압축 테스트의 판정 기준은 `extract_page_text_string`인데 이것은 ToUnicode만 보므로 글리프 재매핑 오류를 잡지 못한다.

## Code
- `justpdf-core/src/font/subset.rs` — `subset_font`, `SubsetResult`, `KEPT_TABLES`, `build_subset_hmtx`, `rewrite_composite_glyph_ids`, `test_gid_mapping_correctness`
- `justpdf-core/src/writer/compress.rs` — `subset_embedded_fonts`, `update_font_widths`, `update_cid_to_gid_map`, `extract_font_map`, `extract_char_codes`, `load_cid_to_gid_map`, `is_cid_font`, `find_fontfile2`

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
- 저장소 어디에도 임베드 TrueType 폰트를 만드는 테스트 픽스처가 없다. Tracked: #9.
- CFF(FontFile3)는 서브세팅 대상이 아니다.
- Tracked: #51 (서브세팅 GID 매핑)
