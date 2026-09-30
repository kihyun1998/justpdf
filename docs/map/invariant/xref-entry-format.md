# Xref entry format

## The fact
Each entry in the classic xref table justpdf writes is **exactly 20 bytes**: the 18 bytes of `nnnnnnnnnn ggggg n` (or `f`) plus a 2-byte end of line. ISO 32000-1:2008 §7.5.4 verbatim (checked 2026-09-28): "a 2-character end-of-line sequence consisting of one of the following: SP CR, SP LF, or CR LF. Thus, the overall length of the entry shall always be exactly 20 bytes." justpdf uses SP LF (`n \n`), as qpdf does — entries in fixtures qpdf 12.4.1 produced (`aes256_r5_user_owner.pdf` and others) read `0000000000 65535 f \n`.

## Why it is cross-cutting
Four separate pieces of code write a classic xref table, and none calls another. Each writes the entry string by hand with `write!`/`extend_from_slice`. Fixing one leaves the rest as they were — the first fix list did miss the free entries for unused numbers in `serialize.rs` and the signature writer, and grep found them.

## Territories it holds in
- [File serialization](../territory/file-serialization.md) — `serialize_pdf_impl`: entry 0, in-use entries, free entries for unused numbers.
- [Incremental save](../territory/incremental-save.md) — `incremental_save`: entries for appended objects and removed objects (`65535 f`).
- [Linearization](../territory/linearization.md) — the main xref in `write_linearized_inner`: entry 0, in-use entries, free entries for unused numbers.
- [Signing](../territory/signing.md) — `build_pdf_with_placeholder`: entries for appended objects.

## What a violation looks like
- (Before #102) all four writers wrote `n \r\n` (space + CRLF, 21 bytes). qpdf 12.4.1 (pikepdf `check_pdf_syntax`) reported every output, down to the simplest file `DocumentBuilder` makes, as "file is damaged / invalid xref entry (obj=1)" and opened it by reconstructing the xref (measured 2026-09-28). justpdf's own reader reads line by line and accepted it, so round-trip tests could not see it, and `test_xref_entry_format` pinned the 21-byte form as its expected value.
- A reader that does not reconstruct reads entries whose offsets drift by one byte each (inferred).

## Discovery history
2026-09-28, found while checking the output of #102 (keep the source encryption) independently with qpdf. Earlier checks only looked at whether qpdf **could open** the file and at its content, not at its warnings.

## Where it will recur
**Any code that writes classic xref table entries is subject to this invariant.** Check a new site with `check_xref_entries` in `integration.rs` (a check derived from the spec wording, covering unused numbers and removed objects). PDFs built by hand inside tests as reader input (tests in `parser.rs`, `repair.rs`, `ocg/parse.rs`) use 21-byte entries; they are input for reader tolerance and are not subject to it. External check: `uv run --with pikepdf`, then confirm `Pdf.check_pdf_syntax()` returns an empty list.
