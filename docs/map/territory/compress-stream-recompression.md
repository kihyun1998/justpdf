# 압축 — 스트림 재압축

## What it is
두 단계: 필터가 없는 128바이트 초과 스트림을 Flate로 압축하고(`compress_raw_streams`), 이미 FlateDecode 하나만 걸린 스트림을 최고 레벨로 다시 압축한다(`recompress_flate_streams`). 결과가 엄격히 작을 때만 교체한다.

## Governing decisions
**None.**

## Design model
- DecodeParms가 있는 스트림(predictor 등)과 이미지 XObject는 재압축하지 않는다.
- 필터 체인이 Flate 단일일 때만(`is_single_flate`).
- **테스트 입력의 함정**: `PageBuilder`가 만드는 콘텐츠 스트림은 이미 zlib 기본 레벨(6)이고, 스트림이 수만 바이트 미만이면 최고 레벨(9)과 크기가 같다 → "엄격히 작을 때만 교체" 규칙에 걸려 재압축 0건. 2026-09-23 실측: 스트림 1개 기준 1,000자 347↔347, 5,000자 1,227↔1,227, 50,000자 10,180↔10,007. 레벨 1(`fast`)로 저장해도 짧은 스트림("Test page N", 100자)은 0건이고, ~1000자여야 5/5건. 그래서 재압축을 검증하는 테스트(Phase B 네 개)는 `create_fast_flate_text_pdf`(~1000자 + 레벨 1)를 입력으로 쓰고, 재압축이 실제로 일어났는지(스트림 수 일치)를 먼저 단언한다.
- "크기가 같으면 교체하지 않는다"는 `test_recompress_flate_already_best_no_growth`의 2차 패스(이미 best인 입력 → 0건)가 고정한다.

## Code
- `justpdf-core/src/writer/compress.rs` — `compress_raw_streams`, `recompress_flate_streams`, `is_single_flate`
- `justpdf-core/src/writer/encode.rs` — `encode_flate`, `encode_flate_best`
- `justpdf-core/src/writer/compress.rs` (테스트) — `create_fast_flate_text_pdf`, `test_recompress_flate_output_not_larger`, `test_recompress_flate_roundtrip_identical`, `test_recompress_flate_already_best_no_growth`, `test_recompress_flate_text_pdf_improvement`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [스트림 필터](stream-filters.md) — 디코드 쪽. Flate 디코더 동작이 바뀌면 재압축 결과 검증이 달라진다.
- [compress-dedup](compress-dedup.md) — 재압축 뒤 바이트가 같아진 스트림이 dedup 대상이 된다.
- [프리셋](compress-presets.md) — `compress_streams` 노브.

## Known holes / open
**None.**
