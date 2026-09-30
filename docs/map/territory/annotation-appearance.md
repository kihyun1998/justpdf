# Annotation appearance streams

## What it is
Generates the `/AP /N` Form XObject for highlight, underline, strikeout, squiggly, square, circle, text note, stamp and Redact annotations.

## Governing decisions
**None.**

## Design model
- The XObject has `BBox [0 0 w h]` and `Matrix [1 0 0 1 -llx -lly]`, and no `/Resources`.
- **The coordinate systems are mixed**: highlight, underline, strikeout, squiggly, square and Redact draw in absolute page coordinates (`rect.llx`…), while text note and stamp draw in local coordinates (`0 0 w h`). Together with the BBox and Matrix above, the shapes in absolute coordinates fall outside the BBox (inferred).
- The stamp uses `/Helvetica 14 Tf` but declares no font resource. Its string is written with `string_syntax` (#29) — [Object syntax roundtrip](../invariant/object-syntax-roundtrip.md).
- The circle Bézier constant duplicates the radio button in [Form appearance](form-appearance.md).

## Code
- `justpdf-core/src/annot/appearance.rs` — `generate_appearance`, `highlight_appearance`, `underline_appearance`, `strikeout_appearance`, `squiggly_appearance`, `square_appearance`, `circle_appearance`, `text_note_appearance`, `stamp_appearance`, `redact_appearance`

## Reference behaviour
**None.** Clauses to compare against: ISO 32000-2 §12.5.5, §8.10 (Form XObject BBox/Matrix).

## Cross-cutting invariants
- [Object syntax roundtrip](../invariant/object-syntax-roundtrip.md)
- [Content text encoding](../invariant/content-text-encoding.md) — stamp text.

## Blast radius
- [Annotations](annotations.md) — the caller.
- [Render annotations](render-annotations.md) — the side that draws this XObject. When changing the coordinate rule, look at it together with the render side's Rect mapping.
- [Form appearance](form-appearance.md), [Signature appearance](signature-appearance.md) — the two other generators doing the same job.

## Known holes / open
- Mixed coordinate systems (above). No test verifies an appearance stream by rendering it.
- Tracked: #34 (non-ASCII content text), #41 (annotation appearance coordinates)
