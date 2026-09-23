# 정리 (clean/dedup)

## What it is
Display 텍스트가 같은 객체를 중복 제거하고, 참조되지 않는 Null을 지우고, 객체 번호를 순서대로 다시 매긴다.

## Governing decisions
**None.**

## Design model
- 중복 판정 `hash_object`가 `format!("{}", obj)`를 쓴다. 스트림의 Display에는 데이터가 없으므로 **데이터가 다른 스트림이 합쳐진다**(연구 에이전트 프로브: `AAAA`와 `BBBB` 스트림 → `duplicate_objects_removed: 1`).
- 번호 재매김은 호출자가 들고 있는 catalog/info 참조를 갱신하지 않는다. [압축 파이프라인](compress-pipeline.md)이 `clean_objects` 대신 GC만 쓰는 이유가 이것이다(주석에 명시).
- `rewrite_references`는 `pub(crate)`로 [압축 dedup](compress-dedup.md)이 재사용한다.

## Code
- `justpdf-core/src/writer/clean.rs` — `clean_objects`, `hash_object`, `dedup_objects`, `rewrite_references`, `remove_null_objects`, `compact_object_numbers`, `CleanStats`

## Reference behaviour
**None.**

## Cross-cutting invariants
- [객체 구문 왕복](../invariant/object-syntax-roundtrip.md) — Display 텍스트를 객체 동일성의 대리로 쓰는 사이트.

## Blast radius
- [압축 dedup](compress-dedup.md) — `rewrite_references` 공유.
- [압축 파이프라인](compress-pipeline.md) — clean을 켜려면 catalog_ref 무효화 문제부터.
- [CLI](cli.md) — `clean` 서브커맨드는 이 모듈이 아니라 재빌드만 한다(이름만 같다).

## Known holes / open
- 제품 코드 호출자가 없다. 스트림 두 개로 된 테스트가 없다.
- Tracked: #27 (스트림 동일성 판정)
