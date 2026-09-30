# Compress dedup

## What it is
Merges streams whose dict and data are both equal into one, and rewrites the references.

## Governing decisions
**None.**

## Design model
- Applies the same `merge_duplicates` as [Clean](clean.md), to streams only (#27). The test is an exact match of the dict (excluding `/Length`) plus the data; merging repeats until nothing changes. Before #27 it looked only at the SHA-256 of the data, so two streams with the same 16 bytes but `/Width 4 /Height 4` and `/Width 2 /Height 8` were merged (reproduced). In the real file `testpdf.pdf` (low preset), two form XObjects that differed only in `/BBox` were being merged — after #27 the removed count went 21 → 20, output +220 bytes.
- References that pointed at a deleted stream are rewritten to the kept stream's number and **generation** (the writer's `generations`). Changing only the number leaves the reference `K 0 R` pointing at no object when the kept stream is written at its source generation g ≠ 0 (found in the #72 check step, `test_dedup_points_references_at_the_generation_of_the_kept_stream`). [Source generation](../invariant/source-generation.md).
- The dict comparison is an exact match, so if references differ (`/SMask 12 0 R` and so on) the streams are not merged in that round. Once the reference targets are merged and the references become equal, they merge in the next round.
  - **Maintainer decision (2026-09-24, #27 triage)**: exact dict match. A more aggressive dedup that accepts semantic equivalence (a direct value and a reference to the same value, and so on) was left out of scope. No alternative was offered separately (default approved).
  - **Maintainer decision (2026-09-24, during #27 implementation)**: leave `/Length` out of the comparison, and repeat until nothing changes. The decision above was made without seeing these two cases — lens reproduced them: two identical images differing only in an indirect `/Length N 0 R` (master removed 1, exact match 0), and two identical images each with its own identical `/SMask` (master removed 2, a single-pass exact match 1). Alternative offered: keep exact match and a single pass. Facts offered as grounds for the decision: the writing side drops `/Length` and recomputes it (`serialize.rs`); MuPDF `pdf-write.c` turns indirect lengths into values before dedup and repeats until nothing changes.

## Code
- `justpdf-core/src/writer/compress.rs` — `dedup_streams`, `test_dedup_merges_streams_with_equal_dict_and_data`, `test_dedup_keeps_streams_whose_dicts_differ`, `test_dedup_ignores_the_length_entry`, `test_dedup_merges_streams_that_become_equal_after_a_merge`, `test_dedup_points_references_at_the_generation_of_the_kept_stream`, `test_dedup_identical_images`
- `justpdf-core/src/writer/clean.rs` — `merge_duplicates`, `find_duplicates`, `same_value`, `bucket_key`, `rewrite_references`

## Reference behaviour
MuPDF `pdf-write.c` — `removeduplicateobjs`: compares a stream's dict and data together, repeats until nothing changes, and never merges page objects ("Never common up pages!").

## Cross-cutting invariants
- [Source generation](../invariant/source-generation.md) — a merged reference takes the kept stream's generation.

## Blast radius
- [Clean](clean.md) — shares `merge_duplicates`. Changing the equality rule changes both paths together.
- [Compress images](compress-images.md), [Compress stream recompression](compress-stream-recompression.md) — the earlier stages' output is dedup's input.

## Known holes / open
**None.**
