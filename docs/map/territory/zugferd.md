# ZUGFeRD (e-invoice reading)

## What it is
Finds ZUGFeRD/Factur-X invoice XML among a PDF's attachments, guesses the profile, and reads a handful of invoice fields from the XML. Read-only: it does not generate invoices.

## Governing decisions
- [ADR-0001](../../adr/0001-crates-split-by-dependency-layer.md) — places it in `justpdf-special` (`zugferd` feature, `roxmltree`); it decides nothing about invoices.

## Design model
Status: read from the source only.
- Lookup (`detect_zugferd_xml`): `read_embedded_files` from core [Embedded files](embedded-files.md), which walks only the catalog's `/Names/EmbeddedFiles` tree; the catalog `/AF` array and the XMP `fx:` declaration are not read. Any attachment whose name ends in `.xml` qualifies (the three named files — `factur-x.xml`, `zugferd-invoice.xml`, `xrechnung.xml` — all end in `.xml`, so the name list adds nothing), when `extract_file` succeeds, it is UTF-8, and it contains `CrossIndustryInvoice` or `rsm:`. The first match wins. An XRechnung in UBL syntax has neither and is not found (inferred).
- Profile (`detect_profile`): a substring search over the whole lower-cased XML in a fixed order — `xrechnung`, `extended`, `en16931` or `comfort`, `basic` and `wl`, `basic`, `minimum`. It does not read `GuidelineSpecifiedDocumentContextParameter/ID`, so a word such as "extended" or "bowl" anywhere in the invoice changes the result (inferred). Tests feed only the URN.
- Fields (`parse_zugferd_xml`): walks every element and keeps the last value seen for `invoice_number` (an `ID` under `ExchangedDocument`), `issue_date` (any `IssueDateTime` or `DateTimeString`, so a later delivery or due date overwrites it — inferred), the four amount totals, `currency`, `seller_name`, `buyer_name`.
- Input bound: none here; the attachment is decoded whole by `extract_file`, whose stream decoding has no size bound in core (Tracked there: #161).

## Code
- `justpdf-special/src/zugferd/mod.rs` — `ZugferdProfile`, `ZugferdInfo`, `is_zugferd`, `extract_zugferd`, `detect_zugferd_xml`, `detect_profile`, `parse_zugferd_xml`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [Embedded files](embedded-files.md) — the only input path; this module assumes every invoice attachment is in the `/Names/EmbeddedFiles` tree and that `extract_file` returns the decoded bytes.
- [Compress stripping](compress-stripping.md) — `high` removes the PDF/A + Factur-X XMP and `extreme` removes the EmbeddedFiles name tree while `/AF` keeps the XML in the file (measured on #58, 2026-10-07), so after `extreme` this module no longer finds an invoice that is still there.

## Known holes / open
- No test runs the lookup on a PDF with an invoice attachment; the two document tests use an empty PDF.
- Profile and issue-date detection are substring and last-wins guesses (Design model).
- Files reachable only through `/AF` are not found.
- Compression strips what an invoice needs; decided on #58 (maintainer, 2026-10-07): detect PDF/A, PDF/UA and `/AF`, keep what they depend on, and report it. Tracked: #58
