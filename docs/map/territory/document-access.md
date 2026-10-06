# Document opening, resolve and cache

## What it is
`PdfDocument` owns the source bytes (`Vec` or mmap), the merged xref, the object LRU cache and the encryption state, and turns references into objects with `resolve(&self)`. Every reading feature and every binding goes through this one type.

## Governing decisions
There is no decision record (ADR). Maintainer decision (2026-09-29, #98): when a reference's generation differs from the generation its xref entry defines, `resolve` returns `Null` — ISO 32000-1 §7.3.10 "An indirect reference to an undefined object shall not be considered an error... it shall be treated as a reference to the null object". Shown: the outcomes of three options laid out (checked against the source code: MuPDF `pdf-xref.c` looks up by number only and builds the key from the xref generation, pdf.js `xref.js` throws the mismatch as an error and reindexes in recovery mode, qpdf `QPDF_objects.cc` looks up by (number, generation) and silently gives null when absent), a measurement over 121 PDFs on this computer (420,709 references) finding 0 generation mismatches and 0 objects at a generation ≠ 0 (which did not decide between the options), and the fact that in #72 this leniency hid a writing-side generation defect from the tests. Alternatives: (b) like MuPDF, build the key from the xref generation and ignore the reference's generation, (c) like pdf.js, an error. Checking the **object number** in the header was not covered by this decision (filed separately). That a number missing from the xref is also `Null` (#108) is not a maintainer decision but a derivation that follows from the same clause and this decision — a better derivation can overturn it.

## Design model
- **Interior mutability**: `RwLock` is used so `resolve` needs only `&self` (`Sync`). No lock is held during I/O.
- The cache hit path also takes the `write()` lock, because it updates the LRU order.
- **Three encryption hooks**: the `/Encrypt` dictionary goes through `load_object_raw` (no decryption); ordinary objects are decrypted with their own number in `load_object` after authentication (an object inside an object stream is decrypted together with the ObjStm in `load_compressed_object` and skipped here — [Object decryption](object-decryption.md)); unauthenticated, it is an `EncryptedDocument` error. Opening tries the empty password automatically, and `authenticate` clears both caches. Only the `Standard` security handler is accepted.
- **Definition check**: for a reference not in the cache that passed authentication, `resolve` checks that the xref defines it (`defines` — the entry is in use, and the reference's generation equals the generation the entry defines, `XrefEntry::defined_generation`: that generation for an in-use entry, 0 for an object inside an object stream), and otherwise returns `Null` (not cached). Free entries, numbers missing from the xref and other generations are all `Null` by this one rule. So the decryption key of a `resolve` that succeeds is always built from the xref generation — before #98 it was built from the reference's generation, so a reference at the wrong generation became garbage under RC4 and a padding error under AES-128 (for R6 the key does not depend on the generation). Before #108 a missing number was an `ObjectNotFound` error.
- The check is only in `resolve`, not in `load_object_raw` — the `/Encrypt` dictionary (read directly by `detect_encryption` through the trailer reference) and object stream containers (read at generation 0) do not go through it.
- The number and generation in a parsed object header (`N g obj`) are not compared against the xref — `load_object_raw` discards them.
- Reference cycles are cut with a `visited` set per `resolve` call.

## Code
- `justpdf-core/src/parser.rs` — `PdfDocument`, `open`, `from_bytes`, `open_mmap`, `resolve`, `authenticate`, `is_encrypted`, `load_object`, `load_object_raw`, `detect_encryption`, `LruCache`, `DEFAULT_CACHE_CAPACITY`, `defines`, `object_refs`
- `justpdf-core/src/xref/table.rs` — `XrefEntry`, `defined_generation`
- `justpdf-core/tests/integration.rs` — `test_a_reference_at_another_generation_resolves_to_null`, `test_an_object_in_an_object_stream_resolves_only_at_generation_zero`, `test_an_object_at_generation_one_resolves_only_at_generation_one`, `test_a_reference_to_a_number_missing_from_the_xref_resolves_to_null`
- `justpdf-core/src/error.rs` — `JustPdfError`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [xref](xref.md), [Object model](object-model.md), [object streams](object-streams.md) — the lower stages of the load path.
- [Object decryption](object-decryption.md), [Password authentication](password-authentication.md) — the three encryption hooks. When moving a hook, look at both notes.
- [repair](repair.md) — builds a document with `from_raw_parts` and skips encryption detection.
- [Facade](facade.md), [CLI](cli.md), [Language bindings](language-bindings.md) — a signature change to `open`/`from_bytes`/`authenticate` reaches all of them.
- [Compress pipeline](compress-pipeline.md) — the decision to refuse encrypted documents relies on `is_encrypted`.

## Known holes / open
- When the number in an object header differs from the xref (the offset points at a different object), that object is returned anyway. The same goes for an object stream's index numbers. Tracked: #109
- The `open_mmap` unit tests in `parser.rs` run only with `--features mmap`, and no CI job turns on `justpdf-core/mmap` (inferred from `.github/workflows/ci.yml`). They make their file in a `tempfile::tempdir()` declared before the document, so on Windows the mapping is dropped before the directory is removed.
- A cache hit also takes the write lock, so `resolve` calls from several threads are serialized (inferred: a candidate bottleneck for parallel rendering).
