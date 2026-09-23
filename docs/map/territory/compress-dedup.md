# 압축 — 스트림 중복 제거

## What it is
스트림 데이터를 SHA-256으로 해시해 같은 스트림을 하나로 합치고, 참조를 다시 쓴다.

## Governing decisions
**None.**

## Design model
- 해시 대상은 **데이터만**이고 사전은 아니다. 바이트는 같지만 Filter·크기·색공간이 다른 스트림도 합쳐질 수 있다(추론 위험).
- 참조 재작성은 [정리(clean)](clean.md)의 `rewrite_references`를 재사용한다.

## Code
- `justpdf-core/src/writer/compress.rs` — `dedup_streams`
- `justpdf-core/src/writer/clean.rs` — `rewrite_references`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [정리(clean)](clean.md) — 참조 재작성 공유. clean의 동일성 판정(Display 텍스트)과 여기(데이터 해시)는 서로 다른 규칙이다.
- [compress-images](compress-images.md), [스트림 재압축](compress-stream-recompression.md) — 앞 단계 출력이 dedup 입력이다.

## Known holes / open
- `test_dedup_identical_font_streams`는 `%PDF`만 확인하며, 입력에 폰트 스트림이 없다. Tracked: #8.
- Tracked: #27 (스트림 동일성 판정)
