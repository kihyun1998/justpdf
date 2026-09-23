# 압축 — 미사용 리소스 제거

## What it is
페이지 콘텐츠에서 실제로 쓰이는 `Tf`(폰트)·`Do`(XObject)·`gs`(ExtGState) 이름을 모아, 리소스 사전에서 쓰이지 않는 항목을 지운다. Form XObject 안으로 재귀한다. 지워진 객체는 뒤이은 GC가 걷어낸다.

## Governing decisions
**None.**

## Design model
- 정리 대상은 `Font`·`XObject`·`ExtGState` 세 하위 사전뿐이다. `ColorSpace`·`Pattern`·`Shading`·`Properties`는 건드리지 않는다.
- 사용 여부는 콘텐츠 스트림 파싱 결과로만 판단한다. 주석 외관 스트림이 쓰는 리소스는 별도로 모으지 않는다(추론).

## Code
- `justpdf-core/src/writer/compress.rs` — `remove_unused_resources`, `clean_resource_subdict`, `count_removed`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [콘텐츠 스트림 파싱](content-stream-parsing.md) — 사용 판정의 입력.
- [optional content](optional-content.md) — `/Properties`를 남기는 이유가 기록된 곳은 없다.
- [압축 파이프라인](compress-pipeline.md) — 뒤이은 GC.
- [프리셋](compress-presets.md) — `remove_unused_resources` 노브(CLI 노출 계획에도 빠져 있음).

## Known holes / open
- 페이지가 아닌 곳(주석 외관, 패턴)에서 쓰는 리소스를 페이지 리소스로 공유하는 파일이면 지워질 수 있다(추론, 테스트 없음).
