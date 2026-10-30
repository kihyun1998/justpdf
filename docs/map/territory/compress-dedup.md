# Compress dedup

## What it is
Merges streams whose dictionary and data are both equal into one, and points the references of the removed copies at the kept one. Gated by the `structural` knob, after every rewriting stage and before GC.

## Governing decisions
**None.** The equality rule was a maintainer decision on #27 (below), recorded on the issue, not in an ADR.

## Design model
- `dedup_streams` calls the same `merge_duplicates` as [Clean](clean.md), restricted to streams (#27). Two streams are duplicates when their data are equal and their dictionaries are equal without `/Length` (`same_value`, `entries_without_length`). `PdfDict` is a `BTreeMap`, so key order does not matter. Merging repeats until a round finds nothing (`merge_duplicates`, `loop`).
- Before #27 dedup looked only at the SHA-256 of the data, so two streams with the same 16 bytes but `/Width 4 /Height 4` and `/Width 2 /Height 8` were merged (reproduced on #27). In `testpdf.pdf` (repo root, `low` preset) two form XObjects that differed only in `/BBox` were merged; after #27 the removed count went from 21 to 20 and the output grew by 220 bytes (#27, 2026-09-25). At 88b2a08 the count is still 20 at every preset (measured 2026-10-30, `justpdf compress -v`).
- Candidates are bucketed by `bucket_key` (the dictionary without `/Length` plus a hash of the data) and compared only within a bucket (`find_duplicates`). The first stream in object-table order is kept.
- A reference that pointed at a removed stream is rewritten to the kept stream's number **and generation** (the writer's `generations`, through `rewrite_references` and `generation_of`). Changing only the number would leave `K 0 R` pointing at nothing when the kept stream keeps a source generation g ≠ 0 (found while checking #72, `test_dedup_points_references_at_the_generation_of_the_kept_stream`) — [Source generation](../invariant/source-generation.md).
- The dictionary comparison is exact, so streams whose references differ (`/SMask 12 0 R` against `/SMask 13 0 R`) are not merged in that round. Once the referenced objects are merged and the references become equal, they merge in the next round (`test_dedup_merges_streams_that_become_equal_after_a_merge`).
  - **Maintainer decision (2026-09-24, #27 triage)**: exact dictionary match. A dedup that accepts semantic equivalence (a direct value against a reference to the same value, and so on) was left out of scope. No alternative was offered separately (default approved).
  - **Maintainer decision (2026-09-24, during #27 implementation)**: leave `/Length` out of the comparison, and repeat until nothing changes. The decision above was made without seeing these two cases, which lens reproduced: two identical images differing only in an indirect `/Length N 0 R` (master removed 1, exact match 0), and two identical images each with its own identical `/SMask` (master removed 2, a single-pass exact match 1). Alternative offered: keep exact match and a single pass. Grounds offered: the writing side drops `/Length` and recomputes it (`serialize.rs`); MuPDF `pdf-write.c` turns indirect lengths into values before dedup and repeats until nothing changes.
- Merging duplicate image streams lowers `analyze_pdf`'s image count on a correct output ([Compress pipeline](compress-pipeline.md)).
- **Input bounds**: every stream in the file is a candidate. Each round is one pass over the objects; each round that continues removes at least one object, so the number of rounds is at most the number of streams (inferred). Nothing else bounds it.

## Code
- `justpdf-core/src/writer/compress.rs` — `dedup_streams`, `dedup_two_streams`, `image_dict`, `create_pdf_with_duplicate_images`, `test_dedup_identical_images`, `test_dedup_points_references_at_the_generation_of_the_kept_stream`, `test_dedup_merges_streams_with_equal_dict_and_data`, `test_dedup_ignores_the_length_entry`, `test_dedup_merges_streams_that_become_equal_after_a_merge`, `test_dedup_keeps_streams_whose_dicts_differ`, `test_dedup_different_streams_no_false_positive`, `test_dedup_roundtrip_valid`
- `justpdf-core/src/writer/clean.rs` — `merge_duplicates`, `find_duplicates`, `same_value`, `entries_without_length`, `bucket_key`, `rewrite_references`, `rewrite_references_in_dict`
- `justpdf-core/src/writer/mod.rs` — `generation_of`

## Reference behaviour
MuPDF `pdf-write.c` — `removeduplicateobjs` compares a stream's dictionary and data together, repeats until nothing changes, and never merges page objects ("Never common up pages!") (read for #27, 2026-09-24). Compress dedup merges only streams, so pages are never candidates here.

## Cross-cutting invariants
- [Source generation](../invariant/source-generation.md) — a merged reference takes the kept stream's generation.

## Blast radius
- [Clean](clean.md) — shares `merge_duplicates`, `same_value` and `bucket_key`; changing the equality rule changes both paths together.
- [Compress images](compress-images.md), [Compress grayscale](compress-grayscale.md), [Font subsetting](font-subsetting.md), [Compress stream recompression](compress-stream-recompression.md) — dedup compares what these stages wrote; two copies one stage rewrote differently no longer merge.
- [Compress pipeline](compress-pipeline.md) — `analyze_pdf` counts image streams, so each merged image lowers the count the web's sanity check compares.

## Known holes / open
- Font programs embedded as separate objects with the same content are subsetted per copy before dedup runs, so the two subsets no longer merge. Tracked: #140 ([Font subsetting](font-subsetting.md)).
