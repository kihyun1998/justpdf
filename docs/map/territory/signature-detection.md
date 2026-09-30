# Signature detection

## What it is
Walks the AcroForm field tree recursively, collects fields that are `/FT /Sig` and have a `/V` as signatures, and pulls the name, reason, location, ByteRange and Contents out of the signature dictionary. It is the only signature feature the facade's `Document::signatures` calls.

## Governing decisions
**None.**

## Design model
- `/Name`, `/Reason`, `/T` and the like are decoded with `from_utf8_lossy` — it does not understand PDFDocEncoding or a UTF-16BE BOM ([Text string encoding](../invariant/text-string-encoding.md)).
- A field that has its own ancestor as a kid is `CircularReference`, and exceeding the visit budget through shared fields is `LimitExceeded` — [Tree traversal cycles](../invariant/tree-traversal-cycles.md).
- It returns early when there are `/Kids`, so a field whose only children are widgets is not checked (inferred).
- In an encrypted document `/Contents` is decrypted on its way through `resolve` (inferred: by the spec, the signature value is not subject to encryption).

## Code
- `justpdf-core/src/sign/detect.rs` — `detect_signatures`, `collect_sig_fields`

## Reference behaviour
**None.** Clause to compare against: ISO 32000-2 §12.8.

## Cross-cutting invariants
- [Text string encoding](../invariant/text-string-encoding.md)
- [Tree traversal cycles](../invariant/tree-traversal-cycles.md)

## Blast radius
- [AcroForm](acroform.md) — each walks the same field tree on its own.
- [Signing](signing.md) — the side that makes the signatures this should be able to find (it currently cannot — the signing side does not register the field in AcroForm).
- [Signature verification](signature-verification.md) — consumer of the detection result.
- [Object decryption](object-decryption.md) — decryption of `/Contents`.
- [Facade](facade.md) — `signatures`.

## Known holes / open
- It does not find the signature in a file justpdf has just signed (inferred; no sign → detect test).
- There is no depth limit — extreme depth without a cycle can overflow the stack (inferred).
- For every field with a shared `/V`, it clones the whole `/V` and copies `contents_raw` — the visit budget does not reach this.
- Tracked: #33 (text string encoding), #37 (signature wiring, CLI sign), #122 (depth limit), #135 (cost outside the budget)
