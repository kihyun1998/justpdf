# 폰트 서브세팅

## What it is
임베드된 TrueType 폰트(FontFile2)에서 문서가 실제로 쓰는 글리프만 남기고 테이블을 다시 조립한다. core의 `subset_font`가 폰트 바이너리를 다루고, 압축 쪽 `subset_embedded_fonts`가 문서에서 쓰인 코드를 모아 남길 GID를 정하고 FontFile2 스트림만 바꾼다. 폰트 사전(`/Widths`·`/CIDToGIDMap`·`/W`·`/Encoding`)은 건드리지 않는다. 유일한 소비처는 압축이다.

## Governing decisions
**None.**

## Design model
- **GID를 유지한다.** `subset_font`는 `numGlyphs`를 그대로 두고, 남기지 않는 글리프를 길이 0으로 만든다(`loca`는 항상 long 형식). 합성 글리프의 성분 참조, `cmap`, `post`는 원본 그대로다. `hmtx`는 원본 배치를 유지하고 남기지 않는 글리프의 값을 0으로 만든다 — 단, 마지막 long metric의 advance는 그 뒤 글리프들이 공유하므로 남긴다. `gid_map`은 남긴 글리프의 항등 매핑이다. CFF/`OTTO` 폰트는 거부한다.
  - 그래서 폰트 사전을 고칠 필요가 없고, 뷰어가 어느 경로(어느 `cmap` 서브테이블, `post` 이름, 코드=GID)로 글리프를 찾든 남긴 GID는 그대로다.
  - 비용(측정, 2026-09-23, Noto Sans로 "Hello"를 `preset_medium` 압축): 서브셋 폰트 프로그램 Flate(9) 크기 28,278 B(번호 재부여, #51 전) → 29,201 B(GID 유지), +923 B. 출력 PDF 전체는 30,888 B → 32,185 B. 추가분은 거의 같은 값이 반복되는 `loca`와 대부분 0인 `hmtx`, 그리고 합집합으로 더 남는 글리프다. 서브셋 크기의 대부분은 원본 그대로 복사되는 `cmap`(원본 12,760 B)·`post`(42,975 B)다 — 이 둘을 줄이는 것은 별개 작업이다.
  - **메인테이너 판단(2026-09-23, #51)**: GID 유지를 택했다. 제시된 대안은 MuPDF의 단순 폰트 방식(번호 재부여 + `cmap` 서브테이블 하나와 `post` 재생성, CID는 `/CIDToGIDMap` 재작성) — 크기가 조금 더 작지만 다른 서브테이블·`post` 이름으로 찾는 뷰어에서 깨지고 구현량이 크다. 판단 근거로 제시된 사실: Ghostscript는 GID를 유지하고 MuPDF도 CID 폰트에서는 유지한다(아래 Reference behaviour).
- **남길 GID 고르기 — 단순 TrueType**(`simple_font_glyph_ids`): 쓰인 코드마다 모든 조회 경로가 닿는 글리프의 **합집합**을 남긴다 — 모든 `cmap` 서브테이블(symbolic 접두 `0xF000`/`0xF100`/`0xF200` 포함), `/Differences` 이름의 `post` 조회와 이름이 적은 유니코드(`uniXXXX`, `uXXXX`, 한 글자), WinAnsi로 읽은 유니코드(+ StandardEncoding의 `'`→U+2019, `` ` ``→U+2018), 코드 자체를 GID로 본 값. GID가 유지되므로 합집합이면 경로 선택과 무관하게 맞다.
  - 폰트를 **통째로 남기는** 경우: `/Differences` 이름을 `post`로도 유니코드로도 찾지 못함(AGL 표가 core에 없다), 또는 non-symbolic 폰트에서 기본 인코딩이 WinAnsi가 아닌데 `0x80` 이상 코드가 쓰임(core의 MacRoman·Standard 표가 근사다 — #45).
- **남길 GID 고르기 — CID**(`cid_font_glyph_ids`): `Identity-H`/`V`만 서브셋하고, CID → GID는 `/CIDToGIDMap`(없거나 `Identity`면 CID = GID, 스트림이면 표; 표 밖 CID는 GID 0). 다른 CMap이나 읽을 수 없는 표면 통째로 남긴다.
- **수집 구멍에 대한 안전장치**: 쓰인 코드는 페이지 콘텐츠의 `Tj`/`'`/`TJ`에서만 모은다. 그래서 수집이 못 보는 곳에서도 쓰일 수 있는 폰트는 서브셋하지 않는다(`fonts_reachable_outside_page_resources`) — 폰트를 참조하는 객체가 페이지 자신(인라인 `/Resources`·`/Font`), 페이지의 간접 `/Resources`, 그 `/Font` 사전 말고도 있으면(Form XObject, 주석 appearance, 페이지 트리에서 상속한 `/Resources`, AcroForm `/DR`). `"` 연산자로 그려진 폰트도 서브셋하지 않는다.
  - **메인테이너 판단(2026-09-23, #51)**: 수집 자체를 고치는 일(content walker)은 별도 이슈로 두고, 그동안 이 안전장치를 둔다. 대안이었던 "#51에 포함"과 "별도 이슈만(안전장치 없음)"이 함께 제시되었다.
  - `q`/`Q`로 폰트 상태를 되돌리는 것은 **추적한다**(글꼴 스택). 위 결정 문구에 없던 항목이라 작업자가 스킵 대신 추적을 제안했고, 메인테이너가 확인했다(2026-09-23, 대안은 "q/Q 안에서 Tf가 바뀌면 그 폰트는 스킵").
- **재현 기록**(2026-09-23): #51 전 코드에서 단순 TrueType은 서브셋 폰트가 3884 → 6 글리프이고 복사된 `cmap`이 옛 GID 43/72/79/82를 가리켜 "Hello" 다섯 글자가 모두 아웃라인을 잃었다(텍스트 추출은 그대로). master의 번호 재부여 `subset_font`로 되돌리면 CID 테스트 두 개(`Identity`, `/CIDToGIDMap` 스트림)도 빨간불이 된다 — CID 경로도 실행으로 재현됐다.
- 압축 테스트의 판정 기준인 `extract_page_text_string`은 ToUnicode만 보므로 글리프 오류를 잡지 못한다. 글리프를 보는 테스트는 각 문자를 폰트 `cmap`(단순) 또는 GID(CID)로 찾아 원본과 아웃라인을 비교한다 — 판정자는 ttf-parser로, 서브셋 코드와 독립이다.
- **픽스처** `justpdf-core/tests/fixtures/NotoSans-Regular.ttf`: notofonts/notofonts.github.io 커밋 `28b15b4b43b7bed62b5cf6e6b0b5ff5846270535`의 `fonts/NotoSans/unhinted/ttf/NotoSans-Regular.ttf`를 수정 없이 가져왔다(431,364 B, sha256 `f3961a9cde016d41a4879aecda1474d3a36d6bf54fa0e4643de029cc2248b0e8`, `test_truetype_fixture_is_pinned`가 고정). 라이선스는 같은 커밋의 `fonts/LICENSE`(SIL OFL 1.1)를 `LICENSE-NotoSans`로 동봉. `glyf` 아웃라인이라 `subset_font`가 받는다.
  - **메인테이너 판단(2026-09-23)**: 수정 없는 원본(431KB)을 택했다. 검토한 대안은 fonttools로 ASCII만 남긴 사전 서브셋(수십 KB, 수정본이라 재현 절차 기록 필요)과 DejaVu Sans(~750KB, OFL 아님). 판단 근거로 제시된 사실: 기존 픽스처 전체가 36KB, `justpdf-core/Cargo.toml`에 `include`/`exclude`가 없어 이 파일이 crates.io 배포본에 실린다. 이 판단은 배포본 포함 여부 자체는 다루지 않았다(Tracked: #66).

## Code
- `justpdf-core/src/font/subset.rs` — `subset_font`, `SubsetResult`, `KEPT_TABLES`, `build_subset_hmtx`, `test_subset_keeps_glyph_ids`, `test_subset_real_font_cmap_still_reaches_kept_glyph`
- `justpdf-core/src/writer/compress.rs` — `subset_embedded_fonts`, `simple_font_glyph_ids`, `glyph_name_to_char`, `cid_font_glyph_ids`, `fonts_reachable_outside_page_resources`, `collect_reference_targets`, `extract_font_map`, `extract_char_codes`, `is_cid_font`, `find_fontfile2`, `create_pdf_with_truetype_font`, `create_pdf_with_cid_font`, `test_subset_truetype_font_actually_subsets`, `test_subset_truetype_font_keeps_glyph_outlines`, `test_subset_cid_identity_keeps_glyph_outlines`
- `justpdf-core/src/font/type3.rs` — `parse_encoding_differences`(서브세팅이 `/Differences`를 읽는 데 쓴다)

## Reference behaviour
2026-09-23, 원본 소스를 직접 읽음(#51):
- **Ghostscript** `devices/vector/gdevpsft.c` — TrueType 쓰기에서 GID를 유지한다. `loca`를 원래 `numGlyphs + 1`개까지 채우고 쓰지 않는 글리프는 길이 0; `glyf`가 작으면 short `loca`.
- **MuPDF** `source/fitz/subset-ttf.c` — CID 폰트는 GID 유지(`new_num_glyphs = orig_num_glyphs`). 단순 폰트는 번호를 다시 매기고, `cmap` 서브테이블 하나((3,1)→(1,0)→(0,1)→(0,3), symbolic이면 (3,0)→(1,0))와 `post`를 다시 만든다. `source/pdf/pdf-subset.c` — 글리프는 폰트 해석 전체(`pdf_font_cid_to_gid`)로 모으고, content processor로 페이지와 주석·위젯을 훑는다. 단순 폰트는 `/FirstChar`·`/LastChar`·`/Widths`를 쓰인 코드로 좁힌다.
- **ISO 32000-2** 9.6.6.4(TrueType 글리프 선택)는 원문과 대조하지 않았다 — 합집합 방식이 경로 선택에 의존하지 않도록 한 이유 중 하나다.

## Cross-cutting invariants
- [폰트 해석 경로](../invariant/font-resolution.md) — 서브셋 결과를 렌더러와 텍스트 추출이 각자 다른 방식으로 해석한다.

## Blast radius
- [폰트 로딩](font-loading.md) — `/Widths` 인덱싱 규칙(`FontWidths::Simple`)을 공유한다.
- [CID 폰트](cid-fonts.md) — `/W`·`/CIDToGIDMap` 해석.
- [글리프 렌더링](glyph-rendering.md) — `char_code_to_glyph_id`가 복사된 `cmap`으로 옛 GID를 찾는다.
- [텍스트 추출](text-extraction.md) — 압축 테스트의 판정자.
- [압축 파이프라인](compress-pipeline.md), [프리셋](compress-presets.md) — `font_subsetting` 노브.

## Known holes / open
- 쓰인 코드 수집은 페이지 콘텐츠의 `Tj`/`'`/`TJ`만 본다(XObject·주석·상속 `/Resources`·`"`는 위 안전장치가 폰트를 통째로 남겨서 막는다). 그만큼 서브셋되지 않는 폰트가 생긴다. Tracked: #68.
- 서브셋 폰트는 원본 `cmap`·`post`·`name`을 그대로 복사한다 — 크기의 대부분이다. Tracked: #69.
- `/Differences` 이름을 AGL로 풀지 못한다(core에 표가 없다). `post`에 없는 이름이면 폰트를 통째로 남긴다.
- symbolic 폰트 경로(`(3,0)` 서브테이블, `0xF000` 접두)를 실행하는 테스트가 없다 — symbolic TrueType 픽스처가 없다. `Identity-V`도 테스트되지 않았다(`Identity-H`만).
- CFF(FontFile3)는 서브세팅 대상이 아니다.
