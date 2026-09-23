# 압축 — 스트림 재압축

## What it is
두 단계: 필터가 없는 128바이트 초과 스트림을 Flate로 압축하고(`compress_raw_streams`), 이미 FlateDecode 하나만 걸린 스트림을 최고 레벨로 다시 압축한다(`recompress_flate_streams`). 결과가 엄격히 작을 때만 교체한다.

## Governing decisions
**None.**

## Design model
- DecodeParms가 있는 스트림(predictor 등)과 이미지 XObject는 재압축하지 않는다.
- 필터 체인이 Flate 단일일 때만(`is_single_flate`).

## Code
- `justpdf-core/src/writer/compress.rs` — `compress_raw_streams`, `recompress_flate_streams`, `is_single_flate`
- `justpdf-core/src/writer/encode.rs` — `encode_flate`, `encode_flate_best`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [스트림 필터](stream-filters.md) — 디코드 쪽. Flate 디코더 동작이 바뀌면 재압축 결과 검증이 달라진다.
- [compress-dedup](compress-dedup.md) — 재압축 뒤 바이트가 같아진 스트림이 dedup 대상이 된다.
- [프리셋](compress-presets.md) — `compress_streams` 노브.

## Known holes / open
- `test_recompress_flate_output_not_larger`는 `%PDF`와 크기만 확인하고 `streams_recompressed` 통계를 버린다. Tracked: #7.
