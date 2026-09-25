# 리댁션 (가림 적용)

## What it is
Redact 주석 영역의 페이지 콘텐츠를 걷어내고 채운 사각형을 그린 뒤 Redact 주석을 지운다. "보이는 것을 지운다"가 아니라 "파일에서 정보를 없앤다"가 약속인 기능이다.

## Governing decisions
**None.**

## Design model
콘텐츠 필터(`filter_content_ops`)의 실제 규칙:
- 텍스트: 텍스트 표시 연산자의 **시작 위치**가 영역 안이면 BT…ET 블록 전체를 지운다. 위치는 `Tm`/`Td`/`TD`만으로 계산하고, `T*`는 12pt 행간을 하드코딩한다("approximate leading"). 텍스트에 CTM을 적용하지 않는다.
- 이미지: 직전 `cm` 하나로 계산한 상자가 영역과 겹치는 `Do`(이미지·Form 구분 없음)와 인라인 이미지(`BI`)를 지운다. q/Q 스택도 행렬 누적도 없다.
  - 인라인 이미지는 #29에서 추가했다. #29 전에는 다시 쓰기가 페이지의 모든 인라인 이미지를 파괴해서, 영역 안 이미지도 우연히 사라졌다. 다시 쓰기가 이미지를 보존하자 영역 안 이미지가 덮개 밑에 남게 되어(`garbage_collect` 후에도) 함께 넣었다. **메인테이너 판단(2026-09-25, #29 check-it)**: `Do`와 같은 규칙으로 거른다. 제시된 대안: #39에 코멘트만 남기기. 정밀한 기하(CTM 누적)는 #39에 남는다.
- **벡터 경로는 지우지 않는다**. Form XObject 내부는 보지 않고 이미지 픽셀은 바꾸지 않는다.
- 걸러낸 연산자는 `write_content`로 다시 쓴다 — 영역 밖의 연산자(괄호 짝 안 맞는 문자열, 인라인 이미지 포함)는 되읽으면 같다(`test_redaction_keeps_content_outside_the_area_unchanged`, #29). 덮개 사각형과 색은 `writeln!("{}", f64)`로 쓴다 — [객체 구문 왕복](../invariant/object-syntax-roundtrip.md)의 생성기 실수 판단.
- **지운 바이트가 파일에 남는다**: 옛 콘텐츠 스트림 객체는 [문서 수정기](document-modifier.md)의 `build`가 GC하지 않아 출력에 남는다. [증분 저장](incremental-save.md)은 설계상 원본을 남긴다(추론: 지운 텍스트가 복구 가능).
- 페이지 콘텐츠를 자체 함수로 조립한다 — [페이지 콘텐츠 조립](../invariant/page-content-assembly.md).

## Code
- `justpdf-core/src/annot/redact.rs` — `apply_redactions`, `filter_content_ops`, `get_page_content_data`, `RedactInfo`

## Reference behaviour
**None.**

## Cross-cutting invariants
- [객체 구문 왕복](../invariant/object-syntax-roundtrip.md)
- [페이지 콘텐츠 조립](../invariant/page-content-assembly.md)

## Blast radius
- [콘텐츠 스트림 파싱](content-stream-parsing.md) — 파싱과 다시 쓰기.
- [문서 수정기](document-modifier.md) — 저장 시 GC 여부가 약속의 성패를 가른다.
- [주석](annotations.md) — Redact 주석 입력.
- [텍스트 추출](text-extraction.md) — 지웠는지 확인할 판정자(현재 테스트가 쓰지 않음).

## Known holes / open
- `test_redaction_apply`는 Redact 주석이 사라졌는지만 확인하고 텍스트가 지워졌는지 확인하지 않는다.
- 콘텐츠가 없는 페이지에서는 Redact 주석이 제거되지 않는다. `/Rect` 없는 Redact는 조용히 버려진다. `overlay_text`는 `dead_code`.
- Tracked: #39 (리댁션 잔존)
