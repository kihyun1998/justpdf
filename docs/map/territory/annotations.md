# 주석 (모델·읽기·생성)

## What it is
페이지 `/Annots`를 타입별 모델로 파싱하고, 빌더로 새 주석 사전을 만들어 페이지에 추가·삭제한다. 추가할 때마다 외관 스트림을 생성해 `/AP /N`에 붙인다.

## Governing decisions
**None.**

## Design model
- 링크 주석은 `/A /URI`를 직접 읽고 `/Dest`를 원시 `PdfObject`로 둔다 — [액션](actions.md) 파서를 쓰지 않는다.
- 텍스트 값: 읽기는 `from_utf8_lossy`, 쓰기는 UTF-8 바이트 — [텍스트 문자열 인코딩](../invariant/text-string-encoding.md).
- `add_annotation`: 페이지 `/Annots`가 간접 참조면 기존 주석을 버리고 새 배열로 바꾼다(추론).

## Code
- `justpdf-core/src/annot/types.rs` — `AnnotationType`, `AnnotationData`, `AnnotationFlags`, `AnnotColor`, `BorderStyle`
- `justpdf-core/src/annot/parse.rs` — `get_annotations`, `get_all_annotations`, `parse_annotation_dict`
- `justpdf-core/src/annot/builder.rs` — `AnnotationBuilder`, `build_dict`, `add_annotation`, `delete_annotation`

## Reference behaviour
**None.** 비교 대상 조항: ISO 32000-2 §12.5.

## Cross-cutting invariants
- [텍스트 문자열 인코딩](../invariant/text-string-encoding.md)

## Blast radius
- [주석 외관](annotation-appearance.md) — 추가 시 항상 호출.
- [렌더 주석](render-annotations.md) — 외관을 그리는 쪽.
- [리댁션](redaction.md) — Redact 주석을 적용한다.
- [문서 수정기](document-modifier.md) — 추가·삭제의 저장 경로.
- [파사드](facade.md) — `annotations`(읽기 전용).

## Known holes / open
- 빌더에 Caret 생성과 Popup 연결이 없다(Popup은 `popup_ref` 파싱만).
- Tracked: #33 (텍스트 문자열 인코딩), #41 (주석 외관 좌표)
