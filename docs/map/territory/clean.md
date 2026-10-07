# Clean (clean/dedup)

## What it is
Removes duplicate objects with equal values, deletes unreferenced Nulls, and renumbers objects in order.

## Governing decisions
**None.**

## Design model
- Duplicates are decided by `same_value` (#27): a value comparison (`PdfObject: PartialEq`), and between streams it compares the dictionary without `/Length` plus the data (the write side drops and recomputes `/Length`). Display text cannot stand in for identity — a stream's Display has no data (before #27: streams `AAAA` and `BBBB` were merged), and `Real(1.0)` and `Integer(1)` are both `"1"` (before #27: the two were merged and `Integer(1)` disappeared). NaN is not equal even to itself, so it is never merged.
  - **Maintainer decision (2026-09-24, #27 triage)**: identity = `PartialEq` value comparison. Reproductions offered: the two cases above. No separate alternatives were offered (default approved).
- `merge_duplicates` does the merging — it finds with `find_duplicates`, deletes, and rewrites references, **repeating until nothing changes**. One merge can make the objects that pointed at that object equal (two images each with their own copy of the same `/SMask`). It uses the same function as [Compress dedup](compress-dedup.md).
- `merge_duplicates` gives rewritten references the generation of the kept object (the `generations` it receives, or 0 without them). `clean_objects` has no generation map, so it writes merged references at generation 0 — the public serialization function that writes the result is also generation 0. Renumbering in `compact_object_numbers` rewrites every reference to an object in the list at generation 0 — even references whose number did not change. This is because the public serialization function that writes the result writes every object at generation 0 (MuPDF `renumberobj` and qpdf also write references at generation 0 when they renumber). References pointing outside the list are left as they are (#106, technical decision).
- Bucket key `bucket_key`: for a stream, the dictionary entries without `/Length` + a hash of the data (`DefaultHasher`); for anything else, the Display text. An equal key does not merge — comparison with `same_value` happens only within the same bucket. `0.0` and `-0.0`, equal in value but different in Display, land in different buckets and are not merged (a miss, so harmless).
  - Without data in the key, streams with the same dictionary gather in one bucket and comparison grows quadratically — measured (release, 4096-byte streams differing only in the last byte, an intermediate implementation keyed on Display alone): 1,000 at 212 ms, 4,000 at 5.5 s, 16,000 at 88 s. With the current key the same inputs take 4 ms, 13 ms, 34 ms (2026-09-25).
- Renumbering does not update the catalog/info references the caller holds. This is why the [Compress pipeline](compress-pipeline.md) uses only GC instead of `clean_objects` (stated in a comment).
- Equal values are merged regardless of kind — two page dictionaries with the same content, or two OCGs with the same name, also become one (`/Kids [3 0 R 3 0 R]`). This was the same before #27 (lens probe, 2026-09-24). Decided in #82: `clean_objects` merges streams only (an allowlist, the predicate `dedup_streams` already uses), because many dictionaries carry identity (pages, OCGs, annotations, form fields, structure elements, outline items, threads, signature fields) and a denylist would miss one. MuPDF `removeduplicateobjs` excludes only `/Type /Page` ("Never common up pages!"), so it would still merge two identical OCGs (inferred).

## Code
- `justpdf-core/src/writer/clean.rs` — `clean_objects`, `same_value`, `bucket_key`, `find_duplicates`, `merge_duplicates`, `dedup_objects`, `test_no_dedup_of_streams_with_different_data`, `test_no_dedup_of_real_and_integer_with_the_same_text`, `test_clean_merges_equal_streams`, `rewrite_references`, `remove_null_objects`, `compact_object_numbers`, `CleanStats`, `test_compaction_writes_references_to_held_objects_at_generation_0`

## Reference behaviour
**None.**

## Cross-cutting invariants
- [Object syntax roundtrip](../invariant/object-syntax-roundtrip.md) — uses Display text as the bucket key. The decision is a value comparison, so even an ambiguous Display form (`Real(1.0)`=`"1"` before #28) does not change the merge result.
- [Source generation](../invariant/source-generation.md) — `merge_duplicates` and `compact_object_numbers` rewrite reference numbers.

## Blast radius
- [Compress dedup](compress-dedup.md) — shares `merge_duplicates`. Changing the identity rule changes both paths together.
- [Compress pipeline](compress-pipeline.md) — turning clean on starts with the catalog_ref invalidation problem.
- [CLI](cli.md) — the `clean` subcommand does not use this module; it only rebuilds (same name only).

## Known holes / open
- There are no callers in product code.
