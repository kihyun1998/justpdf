# 스트림 필터 디코딩

## What it is
스트림의 `/Filter`(이름 또는 배열)를 순서대로 적용하고 `/DecodeParms`를 인덱스로 짝지어 넘긴다. Flate·LZW(+predictor)·ASCII85·ASCIIHex·RunLength·CCITT를 여기서 푼다. DCT·JPX·JBIG2·Crypt는 원바이트를 그대로 통과시킨다 — 픽셀 디코드는 [이미지 디코딩](image-decoding.md)의 몫이고, Crypt는 문서 수준에서 이미 풀려 있다.

## Governing decisions
**None.**

## Design model
- 진입점 셋: 엄격(`decode_stream`), 관용(`decode_stream_tolerant` — 모르는 필터는 건너뛰고 Flate만 부분 복구), 무복사(`decode_stream_cow`). **제품 코드는 모두 엄격 경로만 쓴다**. 관용·무복사는 테스트와 통합 테스트에서만 호출된다.
- predictor는 Flate/LZW 뒤에만 돈다. TIFF predictor는 8비트/성분만 받는다.
- LZW는 early change를 1로 하드코딩한다(`/EarlyChange`를 읽지 않음).
- JPX/JBIG2 통과는 DCT 분기와 같은 의도적 분리다: 픽셀 디코드는 이미지 계층(53799fc)이 맡는다.
- CCITT 출력은 픽셀당 1바이트이며 기본값에서 검정 0x00·흰색 0xFF(`/BlackIs1`이면 반대).

## Code
- `justpdf-core/src/stream/mod.rs` — `decode_stream`, `decode_stream_tolerant`, `decode_single_tolerant`, `decode_stream_cow`, `is_passthrough_filter`, `decode_single`, `get_filters`, `get_decode_params`, `lzw_decode`
- `justpdf-core/src/stream/flate.rs` — `decode`, `decode_partial`
- `justpdf-core/src/stream/predictor.rs` — `apply`, `apply_tiff_predictor`, `apply_png_predictor`
- `justpdf-core/src/stream/ccitt.rs` — `CcittParams`, `decode`, `test_group4_all_white_line`
- `justpdf-core/src/stream/dct.rs` — `decode`, `jpeg_dimensions`
- `justpdf-core/src/stream/ascii85.rs` — `decode`
- `justpdf-core/src/stream/ascii_hex.rs` — `decode`
- `justpdf-core/src/stream/run_length.rs` — `decode`

## Reference behaviour
**None.** 코드는 CCITT에 대해 ITU-T T.4/T.6 표를 인용한다. 비교 대상 조항: ISO 32000-2 §7.4.

## Cross-cutting invariants
**None.**

## Blast radius
- [이미지 디코딩](image-decoding.md) — 통과된 DCT/JPX/JBIG2 바이트를 받는다. 통과 규칙을 바꾸면 이미지 쪽 분기가 이중 디코드된다(렌더러 마스크 경로가 이미 그렇다 — [렌더 이미지](render-images.md)).
- [object streams](object-streams.md), [xref](xref.md) — ObjStm·xref 스트림 디코드.
- [텍스트 추출](text-extraction.md), [렌더 인터프리터](render-interpreter.md), [리댁션](redaction.md), [첨부파일](embedded-files.md) — 콘텐츠·파일 스트림 디코드.
- [스트림 재압축](compress-stream-recompression.md) — 디코드 후 재인코드.

## Known holes / open
- LZW와 TIFF predictor에 테스트가 없다.
- 관용 디코드(깨진 스트림 복구)는 API로만 존재하고 어떤 기능도 쓰지 않는다. 렌더러의 관용성은 에러를 삼키는 것(`unwrap_or_default`)뿐이다.
