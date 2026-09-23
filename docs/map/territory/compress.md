# Aggregate — 압축

**This note owns no detail.** `writer/compress.rs` 한 파일(저장소 최대 파일)에 들어 있는, 서로 다른 여러 기법과 그 기법들을 제품으로 노출하는 표면들의 관계.

| Concept | Note |
|---|---|
| 네 프리셋과 10개 노브가 무엇을 약속하는가, 표면 간 일치 | [compress-presets](compress-presets.md) |
| `compress_pdf` 단계 순서, `analyze_pdf`, 암호화 거부, 꺼진 단계 | [compress-pipeline](compress-pipeline.md) |
| JPEG 재인코딩, CTM 기반 DPI 다운스케일 | [compress-images](compress-images.md) |
| 그레이스케일 변환과 색 연산자 재작성 | [compress-grayscale](compress-grayscale.md) |
| TrueType 서브세팅과 Widths/CIDToGIDMap 갱신 | [font-subsetting](font-subsetting.md) |
| Flate 재압축, 무압축 스트림 압축 | [compress-stream-recompression](compress-stream-recompression.md) |
| 스트림 SHA-256 중복 제거 | [compress-dedup](compress-dedup.md) |
| 미사용 리소스 제거 | [compress-unused-resources](compress-unused-resources.md) |
| 메타데이터·구조·부가 데이터 제거 | [compress-stripping](compress-stripping.md) |
| 브라우저 제품 / CLI 서브커맨드 | [compress-wasm](compress-wasm.md), [CLI](cli.md) |

## Why they sit together
모든 기법이 한 번의 `compress_pdf` 호출 안에서, 하나의 `DocumentModifier` 상태를 순서대로 고쳐 쓴다. 기법끼리 서로 호출하지는 않지만 **앞 단계의 출력이 뒤 단계의 입력**이다: 이미지 교체 뒤에 dedup이 돌고, 리소스 제거 뒤에 GC가 돈다. 그래서 한 기법의 출력 형태(예: 교체된 이미지 사전의 모양)를 바꾸면 뒤 단계를 본다 — 순서는 [compress-pipeline](compress-pipeline.md)이 소유한다.

## 왜 파일이 아니라 개념으로 나눴나
`compress.rs`는 모듈 트리상 한 줄이지만, 블라스트 반경이 기법마다 다르다. 폰트 서브세팅은 [폰트 로딩](font-loading.md)과 렌더링 글리프 매핑에 닿고, 그레이스케일은 [콘텐츠 스트림 파싱](content-stream-parsing.md)과 [객체 구문 왕복](../invariant/object-syntax-roundtrip.md)에 닿고, 프리셋은 브라우저 제품·CLI·외부 웹 저장소에 닿는다. 한 노트로 묶으면 모든 간선이 "압축 전체를 확인"으로 굵어진다.
