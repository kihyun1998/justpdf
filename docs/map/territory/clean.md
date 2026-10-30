# Clean (clean/dedup)

## What it is
`clean_objects` removes duplicate objects with equal values, deletes unreferenced `null` objects, and renumbers the objects from 1 in list order (inferred, `writer/clean.rs — clean_objects`). The duplicate merge, `merge_duplicates`, is shared with [Compress dedup](compress-dedup.md).

## Governing decisions
**None.** No ADR covers it. The identity rule was decided on #27 and its scope on #82, attributed below.

## Design model
- **Identity.** Duplicates are decided by `same_value`: a value comparison (`PdfObject: PartialEq`), where two streams are compared on their dictionaries without `/Length` plus their data, because the write side drops and recomputes `/Length` (inferred, `same_value`; #27). Display text cannot stand in for identity: a stream's Display has no data (before #27, streams `AAAA` and `BBBB` were merged), and `Real(1.0)` and `Integer(1)` were both `"1"` (before #27 the two were merged and `Integer(1)` disappeared) (#27; `test_no_dedup_of_streams_with_different_data`, `test_no_dedup_of_real_and_integer_with_the_same_text`). NaN is not equal even to itself, so it is never merged (inferred, `PartialEq` on `f64`).
  - **Maintainer decision (2026-09-24, #27 triage)**: identity is the `PartialEq` value comparison. Reproductions offered: the two cases above. No separate alternatives were offered; the default was approved.
- **Repeated merging.** `merge_duplicates` finds duplicates with `find_duplicates`, deletes them and rewrites references, **repeating until nothing changes** (inferred). One merge can make the objects that pointed at the merged ones equal (two images each with their own copy of the same `/SMask`). Each round removes at least one object, so the loop ends.
- **Generations.** `merge_duplicates` gives rewritten references the generation of the kept object from the `generations` map it receives, 0 when absent (inferred, `rewrite_references`). `clean_objects` passes an empty map, so merged references are written at generation 0 (inferred). `compact_object_numbers` rewrites every reference to an object in the list at generation 0, even references whose number did not change (inferred). This matches the public serialization functions that write the result, which write every object at generation 0 ([File serialization](file-serialization.md)); MuPDF `renumberobj` and qpdf also write references at generation 0 when they renumber (#106). References pointing outside the list are left as they are (inferred; technical decision, #106).
- **Bucketing.** `bucket_key` is, for a stream, its dictionary entries without `/Length` plus a `DefaultHasher` hash of its data; for anything else, its Display text (inferred). An equal key does not merge: `same_value` is compared only within one bucket. `0.0` and `-0.0` are equal in value but differ in Display, so they land in different buckets and are not merged, a miss and so harmless (inferred, the `bucket_key` doc comment).
  - Without the data in the key, streams with the same dictionary gather in one bucket and comparison grows quadratically. Measured 2026-09-25 (#27; release build, 4096-byte streams differing only in the last byte, an intermediate implementation keyed on Display alone): 1,000 streams in 212 ms, 4,000 in 5.5 s, 16,000 in 88 s. With the current key the same inputs take 4 ms, 13 ms and 34 ms.
- **Caller-held references.** Renumbering does not update the catalog and Info references the caller holds: `clean_objects` takes only the object list (inferred). This is why the [Compress pipeline](compress-pipeline.md) runs only `garbage_collect` and not `clean_objects` (the comment at its structural step in `compress.rs`).
- **Scope.** `clean_objects` merges equal values of every kind (inferred, `dedup_objects` selects `|_| true`): two page dictionaries with the same content, or two OCGs with the same name, also become one (`/Kids [3 0 R 3 0 R]`). This was the same before #27 (lens probe, 2026-09-24).
- `remove_null_objects` removes a `null` object only when no object in the list references its number (inferred).
- **Input size**: nothing bounds the object count or a stream's size. Every stream's data is hashed once per merge round, and each round rebuilds the buckets (inferred, `merge_duplicates`).

## Code
- `justpdf-core/src/writer/clean.rs` — `CleanStats`, `clean_objects`, `entries_without_length`, `same_value`, `bucket_key`, `find_duplicates`, `merge_duplicates`, `dedup_objects`, `rewrite_references`, `rewrite_references_in_dict`, `remove_null_objects`, `collect_references`, `compact_object_numbers`, `test_no_dedup_of_streams_with_different_data`, `test_no_dedup_of_real_and_integer_with_the_same_text`, `test_clean_merges_equal_streams`, `test_compaction_writes_references_to_held_objects_at_generation_0`

## Reference behaviour
- MuPDF `removeduplicateobjs` excludes only `/Type /Page` from merging ("Never common up pages!"), so it would still merge two identical OCGs (inferred on #82, 2026-10-07).
- MuPDF `renumberobj` and qpdf write references at generation 0 when renumbering (#106).

## Cross-cutting invariants
- [Object syntax roundtrip](../invariant/object-syntax-roundtrip.md) — uses Display text as the bucket key. Merging is decided by value, so an ambiguous Display form (`Real(1.0)` as `"1"` before #28) cannot change the merge result.
- [Source generation](../invariant/source-generation.md) — `merge_duplicates` and `compact_object_numbers` rewrite reference numbers.

## Blast radius
- [Compress dedup](compress-dedup.md) — shares `merge_duplicates` and `same_value`. A change to the identity rule changes both paths at once.
- [Compress pipeline](compress-pipeline.md) — turning `clean_objects` on there would renumber objects behind the catalog and Info numbers `DocumentModifier` holds.
- [Object serialization](object-serialization.md) — `bucket_key` assumes Display text is stable for equal non-stream values; a Display change only moves objects between buckets.
- [CLI](cli.md) — the `clean` subcommand shares only the name: it runs `DocumentModifier::garbage_collect` and `build`, not this module (inferred, `cmd_clean`).

## Known holes / open
- There are no callers in product code; `clean_objects` is public through `writer` (inferred).
- `clean_objects` merges distinct objects that carry identity (pages, OCGs, annotations, form fields and others). The maintainer decided on 2026-10-07 that it merges streams only, an allowlist with the predicate `dedup_streams` already uses, because a denylist would miss one of the many dictionary kinds that carry identity (#82). Not built at HEAD (inferred, `dedup_objects`). Tracked: #82
