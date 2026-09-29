# 폰트 서브세팅

## What it is
임베드된 TrueType 폰트(FontFile2)에서 문서가 실제로 쓰는 글리프만 남기고 테이블을 다시 조립한다. core의 `subset_font`가 폰트 바이너리를 다루고, 압축 쪽 `subset_embedded_fonts`가 문서에서 쓰인 코드를 모아 남길 GID를 정하고 FontFile2 스트림만 바꾼다. 폰트 사전(`/Widths`·`/CIDToGIDMap`·`/W`·`/Encoding`)은 건드리지 않는다. 유일한 소비처는 압축이다.

## Governing decisions
**None.**

## Design model
- **GID를 유지한다.** `subset_font`는 `numGlyphs`를 그대로 두고, 남기지 않는 글리프를 길이 0으로 만든다(`loca`는 항상 long 형식). 합성 글리프의 성분 참조는 원본 그대로다. `hmtx`는 원본 배치를 유지하고 남기지 않는 글리프의 값을 0으로 만든다 — 단, 마지막 long metric의 advance는 그 뒤 글리프들이 공유하므로 남긴다. `gid_map`은 남긴 글리프의 항등 매핑이다. CFF/`OTTO` 폰트는 거부한다.
  - 그래서 폰트 사전을 고칠 필요가 없고, 뷰어가 어느 경로(어느 `cmap` 서브테이블, `post` 이름, 코드=GID)로 글리프를 찾든 남긴 GID는 그대로다.
  - 비용(측정, 2026-09-23, Noto Sans로 "Hello"를 `preset_medium` 압축): 서브셋 폰트 프로그램 Flate(9) 크기 28,278 B(번호 재부여, #51 전) → 29,201 B(GID 유지), +923 B. 출력 PDF 전체는 30,888 B → 32,185 B. 추가분은 거의 같은 값이 반복되는 `loca`와 대부분 0인 `hmtx`, 그리고 합집합으로 더 남는 글리프다. 당시 크기의 대부분은 원본 그대로 복사되던 `cmap`(12,760 B)·`post`(42,975 B)였다 — 아래 `cmap`·`post` 줄이기가 그 몫이다.
- **`cmap`은 남긴 글리프로 가는 항목만 남긴다**(`subset_cmap`). 인코딩 레코드는 순서·platform/encoding ID·서브테이블 공유(Noto는 (0,3)/(3,1), (0,4)/(3,10)이 각각 한 서브테이블)를 그대로 둔다. format 0·6은 배치를 유지하고 버린 글리프를 0으로, format 4·12는 남긴 글리프(0 제외)로 가는 매핑을 연속 구간(코드와 GID가 함께 1씩 증가)마다 한 segment/group으로 다시 쓴다(format 4는 `idDelta`만, `idRangeOffset`은 모두 0). 그 밖의 format(2·8·10·13·14)은 원본 그대로 복사한다 — 버린 글리프를 가리키는 항목이 남아도 그 글리프는 비어 있을 뿐 GID는 맞다. 파싱할 수 없는 `cmap`, 16비트 길이를 넘는 format 4(8,189 segment 초과), 매핑이 유니코드 코드 포인트 수(0x110000)를 넘는 format 12도 원본으로 돌아간다.
  - 읽는 비용은 입력이 아니라 남긴 글리프에 묶는다: format 4는 코드 0..0xFFFE를 한 번씩 조회하고(ttf-parser), format 12는 group을 직접 읽어 각 group이 덮는 남긴 GID만 코드로 되돌린다(`format_12_kept_mappings`). 서브테이블의 모든 코드를 도는 방식(`codepoints`)은 쓰지 않는다 — 악의적인 폰트의 group 하나 `[0, 0xFFFFFFFF]`가 약 43억 번 조회가 되고, glyph 0으로 가는 겹친 format 4 segment가 수십 GB를 모은다(리뷰에서 코드 읽기로 추정, 2026-09-29). 테스트는 이 group을 1초 안에 처리하는지 본다.
  - 함정: format 4의 `searchRange`를 테이블 디렉터리용 `calc_table_search_params`(16바이트 단위, u16 곱셈)로 구하면 segment 4,096개부터 overflow한다 — CJK 문서 몇천 자면 닿는다. format 4는 자기 식(`2 × 2^floor(log2 segCount)`)으로 따로 구한다.
  - 모든 서브테이블을 남기는 것은 #51의 합집합 방식과 짝이다: 서브세팅이 모든 조회 경로의 글리프를 남기므로, 어느 서브테이블로 찾는 뷰어든 남긴 글리프에 닿아야 한다.
- **`post`는 남긴 글리프의 이름만 남긴다**(`subset_post`). format 2면 `glyphNameIndex`는 `numGlyphs`개 그대로 두고, 버린 글리프는 0(`.notdef`), 남긴 글리프는 표준 Macintosh 이름(<258)은 그 인덱스, 나머지는 새 이름 목록으로 다시 번호를 매긴다. 다른 format(1·2.5·3·4), 깨진 테이블, 새 인덱스가 예약 영역(32768 이상)에 닿을 만큼 사용자 이름이 많은 경우는 원본 그대로다.
  - `/Differences` 여부와 무관하게 항상 남긴 이름을 둔다(도출, 2026-09-29, #69): 남긴 글리프의 이름은 원본과 같으므로 `post`로 이름을 찾는 뷰어도 깨지지 않고, format 3(이름 없음)으로 가서 아낄 수 있는 것은 대부분 0인 `numGlyphs`×2 B(Noto 7,768 B, Flate 뒤 거의 0)뿐이다. 이슈가 제시한 "`/Differences`를 쓰면 이름을 남긴다" 조건 분기는 이 도출로 필요 없어졌다.
- 비용(측정, 2026-09-29, 위와 같은 "Hello" 입력, `cmap`·`post`를 원본으로 되돌린 같은 트리와 비교): 서브셋 폰트 프로그램 89,260 B → 41,572 B(해제), Flate(9) 29,201 B → 1,822 B. 출력 PDF 전체 32,184 B → 4,803 B. 해제 크기의 남은 대부분은 `loca` 15,540 B·`hmtx` 15,536 B·`post` 인덱스 7,810 B로 거의 0 또는 같은 값이라 Flate 뒤에는 작다. `name`은 1,510 B.
  - **메인테이너 판단(2026-09-29, #69)**: `name`은 원본 그대로 둔다. 제시된 사실: 원문을 읽은 MuPDF `subset_name_table`은 레코드를 하나도 버리지 않고(`filter_name_tables`가 `/* FIXME: For now, we keep everything. */ return 1;`) 같은 문자열만 합친다 — 이슈 본문의 "MuPDF는 줄인다"는 이만큼만 참이다. 크기 1,510 B. 대안은 "중복 문자열만 합침"(MuPDF 수준)과 "필수 name ID만 남김"(참조 구현 선례 없음). 이 판단은 `OS/2`·`cvt `·`fpgm`·`prep`과 CID 폰트에서 `cmap`·`post`를 아예 빼는 것(아래 Known holes)은 다루지 않았다.
  - **메인테이너 판단(2026-09-23, #51)**: GID 유지를 택했다. 제시된 대안은 MuPDF의 단순 폰트 방식(번호 재부여 + `cmap` 서브테이블 하나와 `post` 재생성, CID는 `/CIDToGIDMap` 재작성) — 크기가 조금 더 작지만 다른 서브테이블·`post` 이름으로 찾는 뷰어에서 깨지고 구현량이 크다. 판단 근거로 제시된 사실: Ghostscript는 GID를 유지하고 MuPDF도 CID 폰트에서는 유지한다(아래 Reference behaviour).
- **The unit of subsetting is the FontFile2 stream.** Candidate fonts are grouped by the FontFile2 object they reach; each stream is cut once, keeping the union of the glyph IDs every font in the group picks from the **original** program. `fonts_subsetted` counts replaced streams.
  - The stream is left whole when any font in the group trips the safety guard below or cannot resolve its glyphs (`simple_font_glyph_ids`/`cid_font_glyph_ids` return `None`), or when **the chain is not closed**: something other than the grouped fonts and the objects between them and the stream (the FontDescriptor, and for CID the descendant CIDFont) references the stream or one of those objects. A non-candidate font sharing the program (one drawn only inside a form XObject, or never drawn on a page) and a font reaching it through an inline FontDescriptor are caught here. A group whose fonts all showed only empty strings is left alone.
  - Why (measured, 2026-09-30, #117): subsetting once per font dictionary let the next font read back the stream `set_object` had just replaced and cut it again. With two fonts sharing a program and drawing "H" and "o", **both** lost their outlines — the second font could not find its character in the `cmap` #69 had already cut and picked only the code-as-GID glyph. Ghostscript rendered "Hello" + "world" as `llo  o l`; after the change the render is pixel-identical to the input. A font left whole on purpose (shown with `"`) lost its glyphs when a sibling cut the shared stream. The guard was per font while the cut is on the shared stream, so the decision moved up to the stream.
  - Because glyph IDs are kept (#51), a simple font and a CID font sharing one stream are served by the plain union of their glyph IDs (inferred).
- **남길 GID 고르기 — 단순 TrueType**(`simple_font_glyph_ids`): 쓰인 코드마다 모든 조회 경로가 닿는 글리프의 **합집합**을 남긴다 — 모든 `cmap` 서브테이블(symbolic 접두 `0xF000`/`0xF100`/`0xF200` 포함), `/Differences` 이름의 `post` 조회와 이름이 적은 유니코드(`uniXXXX`, `uXXXX`, 한 글자), WinAnsi로 읽은 유니코드(+ StandardEncoding의 `'`→U+2019, `` ` ``→U+2018), 코드 자체를 GID로 본 값. GID가 유지되므로 합집합이면 경로 선택과 무관하게 맞다.
  - 폰트를 **통째로 남기는** 경우: `/Differences` 이름을 `post`로도 유니코드로도 찾지 못함(AGL 표가 core에 없다), 또는 non-symbolic 폰트에서 기본 인코딩이 WinAnsi가 아닌데 `0x80` 이상 코드가 쓰임(core의 MacRoman·Standard 표가 근사다 — #45).
- **남길 GID 고르기 — CID**(`cid_font_glyph_ids`): `Identity-H`/`V`만 서브셋하고, CID → GID는 `/CIDToGIDMap`(없거나 `Identity`면 CID = GID, 스트림이면 표; 표 밖 CID는 GID 0). 다른 CMap이나 읽을 수 없는 표면 통째로 남긴다.
- **수집 구멍에 대한 안전장치**: 쓰인 코드는 페이지 콘텐츠의 `Tj`/`'`/`TJ`에서만 모은다. 그래서 수집이 못 보는 곳에서도 쓰일 수 있는 폰트는 서브셋하지 않는다(`fonts_reachable_outside_page_resources`) — 폰트를 참조하는 객체가 페이지 자신(인라인 `/Resources`·`/Font`), 페이지의 간접 `/Resources`, 그 `/Font` 사전 말고도 있으면(Form XObject, 주석 appearance, 페이지 트리에서 상속한 `/Resources`, AcroForm `/DR`). `"` 연산자로 그려진 폰트도 서브셋하지 않는다.
  - **메인테이너 판단(2026-09-23, #51)**: 수집 자체를 고치는 일(content walker)은 별도 이슈로 두고, 그동안 이 안전장치를 둔다. 대안이었던 "#51에 포함"과 "별도 이슈만(안전장치 없음)"이 함께 제시되었다.
  - `q`/`Q`로 폰트 상태를 되돌리는 것은 **추적한다**(글꼴 스택). 위 결정 문구에 없던 항목이라 작업자가 스킵 대신 추적을 제안했고, 메인테이너가 확인했다(2026-09-23, 대안은 "q/Q 안에서 Tf가 바뀌면 그 폰트는 스킵").
- **재현 기록**(2026-09-23): #51 전 코드에서 단순 TrueType은 서브셋 폰트가 3884 → 6 글리프이고 복사된 `cmap`이 옛 GID 43/72/79/82를 가리켜 "Hello" 다섯 글자가 모두 아웃라인을 잃었다(텍스트 추출은 그대로). master의 번호 재부여 `subset_font`로 되돌리면 CID 테스트 두 개(`Identity`, `/CIDToGIDMap` 스트림)도 빨간불이 된다 — CID 경로도 실행으로 재현됐다.
- 압축 테스트의 판정 기준인 `extract_page_text_string`은 ToUnicode만 보므로 글리프 오류를 잡지 못한다. 글리프를 보는 테스트는 각 문자를 폰트 `cmap`(단순) 또는 GID(CID)로 찾아 원본과 아웃라인을 비교한다 — 판정자는 ttf-parser로, 서브셋 코드와 독립이다.
  - `cmap`·`post` 테스트는 원본의 모든 (서브테이블, 코드)와 모든 GID의 이름을 ttf-parser로 서브셋과 대조한다. `subset_cmap`도 format 4는 ttf-parser로 **읽으므로** 그 읽기는 판정자와 모델을 공유한다(format 12는 원시 바이트로 읽고, 쓰기 쪽은 모두 독립). 그래서 2026-09-29에 서브셋 출력을 fontTools 4.62로 한 번 따로 읽었다: 네 서브테이블 모두 남긴 8개 글리프(`.notdef`, H·e·l·o, 코드=GID 합집합으로 남은 hyphen·sterling·ordfeminine·uni00AD)에만 닿고, 남긴 글리프의 이름은 원본과 같았다.
  - 함정: "Hello"의 글리프 이름은 모두 표준 Macintosh 이름(<258)이라 `post`의 새 이름 목록 번호를 틀려도 드러나지 않는다 — 이름 테스트는 사용자 이름을 가진 Amacron(267)·eng(331)을 함께 남기고, 그런 글리프가 2개 이상 남았는지부터 확인한다.
  - 함정: ttf-parser는 글리프 0으로 가는 코드를 format 0에서는 `None`, format 6에서는 `Some(GlyphId(0))`으로 돌려준다.
- **픽스처** `justpdf-core/tests/fixtures/NotoSans-Regular.ttf`: notofonts/notofonts.github.io 커밋 `28b15b4b43b7bed62b5cf6e6b0b5ff5846270535`의 `fonts/NotoSans/unhinted/ttf/NotoSans-Regular.ttf`를 수정 없이 가져왔다(431,364 B, sha256 `f3961a9cde016d41a4879aecda1474d3a36d6bf54fa0e4643de029cc2248b0e8`, `test_truetype_fixture_is_pinned`가 고정). 라이선스는 같은 커밋의 `fonts/LICENSE`(SIL OFL 1.1)를 `LICENSE-NotoSans`로 동봉. `glyf` 아웃라인이라 `subset_font`가 받는다.
  - **메인테이너 판단(2026-09-23)**: 수정 없는 원본(431KB)을 택했다. 검토한 대안은 fonttools로 ASCII만 남긴 사전 서브셋(수십 KB, 수정본이라 재현 절차 기록 필요)과 DejaVu Sans(~750KB, OFL 아님). 판단 근거로 제시된 사실: 기존 픽스처 전체가 36KB, `justpdf-core/Cargo.toml`에 `include`/`exclude`가 없어 이 파일이 crates.io 배포본에 실린다. 이 판단은 배포본 포함 여부 자체는 다루지 않았다(Tracked: #66).

## Code
- `justpdf-core/src/font/subset.rs` — `subset_font`, `SubsetResult`, `KEPT_TABLES`, `build_subset_hmtx`, `subset_cmap`, `cmap_subtable_bytes`, `subset_cmap_subtable`, `format_12_kept_mappings`, `cmap_runs`, `encode_cmap_format_4`, `encode_cmap_format_12`, `subset_post`, `test_subset_keeps_glyph_ids`, `test_subset_real_font_cmap_still_reaches_kept_glyph`, `test_subset_real_font_cmap_keeps_only_kept_glyph_entries`, `test_subset_real_font_post_keeps_only_kept_glyph_names`, `test_subset_cmap_formats_0_and_6_drop_entries_and_unknown_formats_are_copied`, `test_subset_post_leaves_other_formats_alone`, `test_subset_cmap_format_4_with_many_segments`, `test_subset_cmap_format_12_reads_only_kept_glyphs_of_a_huge_group`, `test_subset_post_refuses_more_custom_names_than_indices_allow`
- `justpdf-core/src/writer/compress.rs` — `subset_embedded_fonts`, `ProgramUsers`, `simple_font_glyph_ids`, `glyph_name_to_char`, `cid_font_glyph_ids`, `fonts_reachable_outside_page_resources`, `object_referrers`, `collect_reference_targets`, `extract_font_map`, `extract_char_codes`, `is_cid_font`, `find_fontfile2`, `create_pdf_with_truetype_font`, `create_pdf_with_cid_font`, `test_subset_truetype_font_actually_subsets`, `test_subset_truetype_font_keeps_glyph_outlines`, `test_subset_cid_identity_keeps_glyph_outlines`, `add_font_sharing_program`, `add_form_xobject`, `test_subset_shared_program_keeps_glyphs_of_every_font`, `test_subset_skips_shared_program_also_reached_by_a_non_candidate_font`, `test_subset_skips_shared_program_reached_through_an_inline_font_descriptor`
- `justpdf-core/src/font/type3.rs` — `parse_encoding_differences`(서브세팅이 `/Differences`를 읽는 데 쓴다)

## Reference behaviour
2026-09-23, 원본 소스를 직접 읽음(#51):
- **Ghostscript** `devices/vector/gdevpsft.c` — TrueType 쓰기에서 GID를 유지한다. `loca`를 원래 `numGlyphs + 1`개까지 채우고 쓰지 않는 글리프는 길이 0; `glyf`가 작으면 short `loca`.
- **MuPDF** `source/fitz/subset-ttf.c` — CID 폰트는 GID 유지(`new_num_glyphs = orig_num_glyphs`). 단순 폰트는 번호를 다시 매기고, `cmap` 서브테이블 하나((3,1)→(1,0)→(0,1)→(0,3), symbolic이면 (3,0)→(1,0))와 `post`를 다시 만든다. `source/pdf/pdf-subset.c` — 글리프는 폰트 해석 전체(`pdf_font_cid_to_gid`)로 모으고, content processor로 페이지와 주석·위젯을 훑는다. 단순 폰트는 `/FirstChar`·`/LastChar`·`/Widths`를 쓰인 코드로 좁힌다.
- **ISO 32000-2** 9.6.6.4(TrueType 글리프 선택)는 원문과 대조하지 않았다 — 합집합 방식이 경로 선택에 의존하지 않도록 한 이유 중 하나다.

2026-09-29, 원본 소스를 직접 읽음(#69):
- **MuPDF** `source/fitz/subset-ttf.c` — `subset_name_table`은 레코드를 모두 남기고(`filter_name_tables`가 항상 1) 같은 문자열만 합친다. `subset_post`는 format 2만 줄이고(남긴 글리프 이름만, 표준 Macintosh 이름이면 그 인덱스로) 다른 format이면 `post`를 **버린다**. CID 폰트는 `cmap`·`post`를 만들지도 복사하지도 않는다. `OS/2`·`cvt `·`fpgm`·`prep`은 복사.
- **Ghostscript** `devices/vector/gdevpsft.c` — CID로 쓸 때는 `cmap`·`name`·`OS/2`와 모르는 테이블을 뺀다. 원본에 `post`가 있으면 그대로 복사한다. 실행 결과(gswin64c 10.06.0 `pdfwrite`, 위 "Hello" 입력 PDF): 출력 PDF 29,112 B, 폰트 프로그램 Flate 25,948 B(해제 73,208 B) — `numGlyphs` 3884 유지, 원본 `post` 42,975 B·`GSUB` 11,960 B를 그대로 싣고 `cmap`은 (1,0) format 0과 (3,0) format 4를 4항목으로 새로 만든다.

2026-09-30, read from the raw source (#117, `ArtifexSoftware/mupdf` master `a7876256`):
- **MuPDF** `source/pdf/pdf-subset.c` — one `font_usage_t` per font file ("We have one of these records for each fontfile", found by `num`/`gen`), holding the list of top-level font dictionaries that use it and heaps of GIDs and CIDs. `font_analysis_Tf` resolves each font dictionary down to its FontFile/FontFile2/FontFile3 and attaches it to that record; after every page is examined, `subset_ttf`/`subset_cff` runs once per record — the same shape as the per-stream subsetting here. It also merges font files held as different objects but with the same `fz_font_digest`, by pointing the FontDescriptor at the first one. It has no "leave whole" guard, because `examine_page` processes the page contents, every annotation and every widget.

## Cross-cutting invariants
- [폰트 해석 경로](../invariant/font-resolution.md) — 서브셋 결과를 렌더러와 텍스트 추출이 각자 다른 방식으로 해석한다.

## Blast radius
- [폰트 로딩](font-loading.md) — `/Widths` 인덱싱 규칙(`FontWidths::Simple`)을 공유한다.
- [CID 폰트](cid-fonts.md) — `/W`·`/CIDToGIDMap` 해석.
- [글리프 렌더링](glyph-rendering.md) — `char_code_to_glyph_id`가 서브셋이 다시 쓴 `cmap`으로 GID를 찾는다.
- [텍스트 추출](text-extraction.md) — 압축 테스트의 판정자.
- [압축 파이프라인](compress-pipeline.md), [프리셋](compress-presets.md) — `font_subsetting` 노브.

## Known holes / open
- 쓰인 코드 수집은 페이지 콘텐츠의 `Tj`/`'`/`TJ`만 본다(XObject·주석·상속 `/Resources`·`"`는 위 안전장치가 폰트를 통째로 남겨서 막는다). 그만큼 서브셋되지 않는 폰트가 생긴다. Tracked: #68.
- A shared FontFile2 that a non-candidate font also reaches (one drawn only inside a form XObject, or never drawn on a page) is left whole, since that font's glyphs are not collected. Once collection reaches XObjects this case can be subsetted too. Tracked: #68.
- A font program embedded several times as different objects with the same content is subsetted once per copy. MuPDF merges such files by font digest (Reference behaviour above). `dedup_streams` runs after subsetting, so copies cut differently no longer merge (inferred). Tracked: #140.
- A page whose `/Contents` is a reference to an indirect array contributes no codes and does not mark its fonts unsafe, so a font it shares with another page is cut without its glyphs (measured, 2026-09-30). Tracked: #138.
- Subsetted fonts get no subset tag (`ABCDEF+`) on `/BaseFont`/`/FontName`, and a CID font keeps its original `/CIDSet` (inferred; ISO 32000 not yet checked). Tracked: #141.
- 단순 TrueType 글리프 선택은 (1,0) 서브테이블의 MacRoman 경로와 기본 인코딩 이름 → `post` 경로를 보지 않는다(추론) — 그 글리프는 남지 않고, `cmap`·`post`에서도 항목이 빠진다. Tracked: #118.
- 서브셋 폰트는 원본 `name`(메인테이너 판단)과 `OS/2`·`cvt `·`fpgm`·`prep`을 그대로 복사한다.
- CID 폰트(`/CIDToGIDMap`으로 GID를 찾는다)에도 `cmap`·`post`를 줄여서 남긴다. MuPDF와 Ghostscript는 CID 폰트에서 이 둘을 뺀다.
- `cmap` format 2·8·10·13·14는 원본 그대로 복사되어 버린 글리프 항목이 남는다. format 4가 16비트 길이를 넘을 때와 format 12 매핑 상한을 넘을 때 원본으로 돌아가는 경로는 실행하는 테스트가 없다.
- `/Differences` 이름을 AGL로 풀지 못한다(core에 표가 없다). `post`에 없는 이름이면 폰트를 통째로 남긴다.
- symbolic 폰트 경로(`(3,0)` 서브테이블, `0xF000` 접두)를 실행하는 테스트가 없다 — symbolic TrueType 픽스처가 없다. `Identity-V`도 테스트되지 않았다(`Identity-H`만).
- CFF(FontFile3)는 서브세팅 대상이 아니다.
