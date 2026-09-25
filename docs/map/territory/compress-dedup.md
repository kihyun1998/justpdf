# 압축 — 스트림 중복 제거

## What it is
사전과 데이터가 모두 같은 스트림을 하나로 합치고, 참조를 다시 쓴다.

## Governing decisions
**None.**

## Design model
- [정리(clean)](clean.md)와 같은 `merge_duplicates`를 스트림에만 적용한다(#27). 판정은 사전(`/Length` 제외) 정확 일치 + 데이터, 합치기는 변화가 없을 때까지 반복. #27 전에는 데이터의 SHA-256만 봐서, 같은 16바이트에 `/Width 4 /Height 4`와 `/Width 2 /Height 8`인 두 스트림이 합쳐졌다(재현). 실제 파일 `testpdf.pdf`(low 프리셋)에서는 `/BBox`만 다른 두 폼 XObject가 합쳐지고 있었다 — #27 뒤 제거 수 21 → 20, 출력 +220바이트.
- 사전 비교는 정확 일치라 참조(`/SMask 12 0 R` 등)가 다르면 그 차례에는 합치지 않는다. 참조 대상이 합쳐져 참조가 같아지면 다음 차례에 합친다.
  - **메인테이너 판단(2026-09-24, #27 triage)**: 사전 정확 일치. 의미상 동치(직접 값과 같은 값을 가리키는 참조 등)를 인정하는 더 공격적인 dedup은 범위 밖으로 두었다. 대안은 따로 제시되지 않았다(기본값 승인).
  - **메인테이너 판단(2026-09-24, #27 구현 중)**: `/Length`는 비교에서 빼고, 변화가 없을 때까지 반복한다. 위 판단은 이 두 경우를 보지 못한 채 내려졌다 — lens가 재현했다: 간접 `/Length N 0 R`만 다른 같은 이미지 두 개(master 1개 제거, 정확 일치 0개), 같은 `/SMask`를 따로 가진 같은 이미지 두 개(master 2개, 한 번만 도는 정확 일치 1개). 제시된 대안: 지금대로 정확 일치·한 번. 판단 근거로 제시된 사실: 쓰기 쪽이 `/Length`를 버리고 다시 계산한다(`serialize.rs`), MuPDF `pdf-write.c`는 dedup 전에 간접 길이를 값으로 바꾸고 변화가 없을 때까지 반복한다.

## Code
- `justpdf-core/src/writer/compress.rs` — `dedup_streams`, `test_dedup_merges_streams_with_equal_dict_and_data`, `test_dedup_keeps_streams_whose_dicts_differ`, `test_dedup_ignores_the_length_entry`, `test_dedup_merges_streams_that_become_equal_after_a_merge`, `test_dedup_identical_images`
- `justpdf-core/src/writer/clean.rs` — `merge_duplicates`, `find_duplicates`, `same_value`, `bucket_key`, `rewrite_references`

## Reference behaviour
MuPDF `pdf-write.c` — `removeduplicateobjs`: 스트림은 사전과 데이터를 함께 비교하고, 변화가 없을 때까지 반복하며, 페이지 객체는 합치지 않는다("Never common up pages!").

## Cross-cutting invariants
**None.**

## Blast radius
- [정리(clean)](clean.md) — `merge_duplicates` 공유. 동일성 규칙을 바꾸면 두 경로가 함께 바뀐다.
- [compress-images](compress-images.md), [스트림 재압축](compress-stream-recompression.md) — 앞 단계 출력이 dedup 입력이다.

## Known holes / open
**None.**
