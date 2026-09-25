# 주석 외관 스트림

## What it is
하이라이트·밑줄·취소선·물결·사각형·원·메모·스탬프·Redact 주석의 `/AP /N` Form XObject를 생성한다.

## Governing decisions
**None.**

## Design model
- XObject는 `BBox [0 0 w h]`, `Matrix [1 0 0 1 -llx -lly]`이고 `/Resources`가 없다.
- **좌표계가 섞여 있다**: 하이라이트·밑줄·취소선·물결·사각형·Redact는 페이지 절대 좌표(`rect.llx`…)로, 메모·스탬프는 로컬 좌표(`0 0 w h`)로 그린다. 위 BBox·Matrix와 함께면 절대 좌표 도형은 BBox 밖으로 나간다(추론).
- 스탬프는 `/Helvetica 14 Tf`를 쓰지만 폰트 리소스를 선언하지 않는다. 문자열은 `string_syntax`로 쓴다(#29) — [객체 구문 왕복](../invariant/object-syntax-roundtrip.md).
- 원 베지어 상수가 [폼 외관](form-appearance.md)의 라디오 버튼과 중복이다.

## Code
- `justpdf-core/src/annot/appearance.rs` — `generate_appearance`, `highlight_appearance`, `underline_appearance`, `strikeout_appearance`, `squiggly_appearance`, `square_appearance`, `circle_appearance`, `text_note_appearance`, `stamp_appearance`, `redact_appearance`

## Reference behaviour
**None.** 비교 대상 조항: ISO 32000-2 §12.5.5, §8.10(Form XObject BBox/Matrix).

## Cross-cutting invariants
- [객체 구문 왕복](../invariant/object-syntax-roundtrip.md)
- [콘텐츠 텍스트 인코딩](../invariant/content-text-encoding.md) — 스탬프 텍스트.

## Blast radius
- [주석](annotations.md) — 호출자.
- [렌더 주석](render-annotations.md) — 이 XObject를 그리는 쪽. 좌표 규칙을 고치면 렌더 쪽 Rect 매핑과 함께 본다.
- [폼 외관](form-appearance.md), [서명 외관](signature-appearance.md) — 같은 일을 하는 다른 두 생성기.

## Known holes / open
- 좌표계 혼용(위). 외관 스트림을 렌더해 검증하는 테스트가 없다.
- Tracked: #34 (비 ASCII 콘텐츠 텍스트), #41 (주석 외관 좌표)
