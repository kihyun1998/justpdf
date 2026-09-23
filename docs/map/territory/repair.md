# 복구 (repair)

## What it is
xref가 깨진 파일을 위해 파일 전체에서 `N M obj` 헤더를 스캔해 xref를 다시 만들고, trailer를 찾거나 `/Type /Catalog` 객체로 합성한다.

## Governing decisions
**None.**

## Design model
- 같은 번호가 여러 번 나오면 **마지막 것이 이긴다**(증분 업데이트 의미론을 흉내 냄).
- trailer 탐색은 마지막 4 KiB만 본다. 버전 헤더를 못 읽으면 1.4로 둔다.
- 만드는 엔트리는 `InUse`뿐이다 — object stream 안의 객체는 복구되지 않는다(추론).
- `from_raw_parts`는 `security: None`으로 문서를 만들고 암호화 감지를 하지 않는다.

## Code
- `justpdf-core/src/repair.rs` — `rebuild_xref`, `repair_document`, `from_bytes_with_repair`, `scan_object_headers`, `find_trailer_dict`, `synthesise_trailer`, `try_parse_dict_at`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [xref](xref.md) — 같은 `Xref` 구조를 만든다.
- [문서 접근](document-access.md) — `from_raw_parts`로 `PdfDocument`를 조립한다.
- [토크나이저](tokenizer.md), [객체 모델](object-model.md) — 헤더·사전 스캔에 쓴다.

## Known holes / open
- **어디서도 호출되지 않는다.** 소비처는 명령으로 확인한다: `rg -l 'from_bytes_with_repair|repair_document|rebuild_xref' --glob '*.rs' --glob '!target' .` — `repair.rs` 자신만 나온다. 일반 `open`은 실패 시 repair로 떨어지지 않는다.
- 복구된 암호화 문서는 복호화되지 않는다(위 Design model).
