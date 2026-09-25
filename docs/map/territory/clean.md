# 정리 (clean/dedup)

## What it is
값이 같은 객체를 중복 제거하고, 참조되지 않는 Null을 지우고, 객체 번호를 순서대로 다시 매긴다.

## Governing decisions
**None.**

## Design model
- 중복 판정은 `same_value`다(#27): 값 비교(`PdfObject: PartialEq`)이고, 스트림끼리는 `/Length`를 빼고 사전·데이터를 비교한다(`/Length`는 쓰기 쪽이 버리고 다시 계산한다). Display 텍스트는 동일성의 대리가 될 수 없다 — 스트림 Display에는 데이터가 없고(#27 전: `AAAA`·`BBBB` 스트림이 합쳐졌다), `Real(1.0)`과 `Integer(1)`이 둘 다 `"1"`이다(#27 전: 둘이 합쳐져 `Integer(1)`이 사라졌다). NaN은 자기 자신과도 같지 않아 합쳐지지 않는다.
  - **메인테이너 판단(2026-09-24, #27 triage)**: 동일성 = `PartialEq` 값 비교. 제시된 재현: 위 두 사례. 대안은 따로 제시되지 않았다(기본값 승인).
- 합치기는 `merge_duplicates`가 한다 — `find_duplicates`로 찾고, 지우고, 참조를 다시 쓰기를 **변화가 없을 때까지 반복**한다. 한 번의 병합이 그 객체를 가리키던 객체들을 같게 만들 수 있기 때문이다(같은 `/SMask`를 따로 가진 두 이미지). [압축 dedup](compress-dedup.md)과 같은 함수를 쓴다.
- 버킷 키 `bucket_key`: 스트림은 `/Length`를 뺀 사전 항목 + 데이터 해시(`DefaultHasher`), 그 밖은 Display 텍스트. 키가 같다고 합치지 않는다 — 같은 버킷 안에서만 `same_value`로 비교한다. 값은 같아도 Display가 다른 `0.0`과 `-0.0`은 버킷이 달라 합쳐지지 않는다(놓치는 쪽이라 무해).
  - 키에 데이터가 없으면 사전이 같은 스트림이 한 버킷에 모여 비교가 제곱으로 는다 — 측정(release, 4096바이트 스트림이 끝 바이트만 다름, Display만 키로 쓴 중간 구현): 1,000개 212 ms, 4,000개 5.5 s, 16,000개 88 s. 지금 키로 같은 입력이 4 ms, 13 ms, 34 ms(2026-09-25).
- 번호 재매김은 호출자가 들고 있는 catalog/info 참조를 갱신하지 않는다. [압축 파이프라인](compress-pipeline.md)이 `clean_objects` 대신 GC만 쓰는 이유가 이것이다(주석에 명시).
- 값이 같으면 종류를 가리지 않고 합친다 — 같은 내용의 두 페이지 사전이나 이름이 같은 두 OCG도 하나가 된다(`/Kids [3 0 R 3 0 R]`). #27 전에도 같았다(lens 프로브, 2026-09-24).

## Code
- `justpdf-core/src/writer/clean.rs` — `clean_objects`, `same_value`, `bucket_key`, `find_duplicates`, `merge_duplicates`, `dedup_objects`, `test_no_dedup_of_streams_with_different_data`, `test_no_dedup_of_real_and_integer_with_the_same_text`, `test_clean_merges_equal_streams`, `rewrite_references`, `remove_null_objects`, `compact_object_numbers`, `CleanStats`

## Reference behaviour
**None.**

## Cross-cutting invariants
- [객체 구문 왕복](../invariant/object-syntax-roundtrip.md) — Display 텍스트를 버킷 키로 쓴다. 판정은 값 비교라 Display 왕복 구멍(#28)이 결과를 바꾸지 않는다.

## Blast radius
- [압축 dedup](compress-dedup.md) — `merge_duplicates` 공유. 동일성 규칙을 바꾸면 두 경로가 함께 바뀐다.
- [압축 파이프라인](compress-pipeline.md) — clean을 켜려면 catalog_ref 무효화 문제부터.
- [CLI](cli.md) — `clean` 서브커맨드는 이 모듈이 아니라 재빌드만 한다(이름만 같다).

## Known holes / open
- 제품 코드 호출자가 없다.
