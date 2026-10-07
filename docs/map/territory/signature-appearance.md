# Signature appearance (visible signature)

## What it is
Builds the Form XObject of a visible signature: writes the signer, reason and date in Helvetica Type1 (`/F1`) and draws a border.

## Governing decisions
**None.**

## Design model
- `build_pdf_with_placeholder` calls it with `date=None` — no date is shown.
- Puts `encode_winansi` of each line into `Tj` with `string_syntax` (#29), in `/F1`, declared in its `/Resources` as a WinAnsi Helvetica (measured, #34: `Müller` is written `M\xFCller`) — [Content text encoding](../invariant/content-text-encoding.md).

## Code
- `justpdf-core/src/sign/appearance.rs` — `generate_signature_appearance`

## Reference behaviour
**None.**

## Cross-cutting invariants
- [Content text encoding](../invariant/content-text-encoding.md)
- [Object syntax roundtrip](../invariant/object-syntax-roundtrip.md) — `string_syntax`.

## Blast radius
- [Signing](signing.md) — the caller.
- [Form appearance](form-appearance.md), [Annotation appearance](annotation-appearance.md) — two other generators doing the same job. All three escape strings with `string_syntax`. When fixing the font rules, look at all three together.

## Known holes / open
- Tracked: #173 (text beyond WinAnsi)
