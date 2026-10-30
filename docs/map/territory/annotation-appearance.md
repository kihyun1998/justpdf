# Annotation appearance streams

## What it is
Generates the `/AP /N` Form XObject for highlight, underline, strikeout, squiggly, square, circle, line, ink, text note, stamp and Redact annotations when [Annotations](annotations.md) adds one (`generate_appearance`). Any other subtype gets no appearance.

## Governing decisions
**None.** The glossary's **Appearance font** (the caller-supplied font justpdf's generated appearances would draw with, the stamp included) is a term for #175, not a decision this generator follows yet.

## Design model
From reading the source; no record governs it.
- The XObject has `/BBox [0 0 w h]` and `/Matrix [1 0 0 1 −llx −lly]`, from the normalised `/Rect` (`generate_appearance`).
- **The coordinate systems are mixed**: highlight, underline, strikeout, squiggly, square, circle and Redact draw in absolute page coordinates (`rect.llx`…), line and ink at the absolute coordinates of `/L` and `/InkList`, while text note and stamp draw in local coordinates (`0 0 w h`). With the BBox and Matrix above, the shapes in absolute coordinates fall outside the BBox and are clipped away:
  - MuPDF 1.28.2 draws 0 of 4000 pixels of a highlight at Rect `[100 500 300 520]`, and draws the text note in its Rect (measured, recorded in the old note; date not given).
  - justpdf's own renderer, which maps and clips by §12.5.5 since #206, draws 0 red pixels for a red highlight added at Rect `[100 500 300 520]` with `add_annotation`, and 1412 yellow pixels of a 40×40 text note's 1600 (measured 2026-10-30 at 88b2a08, scratch probe; the note's border and fold lines account for the rest).
- **Only a Stamp's XObject declares `/Resources`**: `/Font /Helvetica` as a WinAnsi Helvetica (`standard_font_dict`), its `/Name` text `encode_winansi`d and written with `string_syntax` (#34, #29; `stamp_writes_win_ansi_and_declares_its_font`, `non_text_annotations_declare_no_font`) — [Content text encoding](../invariant/content-text-encoding.md), [Object syntax roundtrip](../invariant/object-syntax-roundtrip.md). Characters outside WinAnsi become `?`.
- Coordinates and colours are written as `Number` (#90; the test module's `non_finite_operators` check).
- The circle's Bézier constant `0.5522847498` is written inline here and again as `CIRCLE_K` in [Form appearance](form-appearance.md) (`form/appearance.rs`).
- **Bounds on input**: the squiggly zigzag writes one segment per 4 units of Rect width, and ink writes one segment per `/InkList` point; neither is capped, so the stream size follows the Rect width and the point count the caller passes (inferred).

## Code
- `justpdf-core/src/annot/appearance.rs` — `generate_appearance`, `set_stroke_color`, `set_fill_color`, `highlight_appearance`, `underline_appearance`, `strikeout_appearance`, `squiggly_appearance`, `square_appearance`, `circle_appearance`, `line_appearance`, `ink_appearance`, `text_note_appearance`, `stamp_appearance`, `redact_appearance`

## Reference behaviour
**None.** Clauses to compare against: ISO 32000-2 §12.5.5, §8.10 (Form XObject BBox/Matrix).

## Cross-cutting invariants
- [Object syntax roundtrip](../invariant/object-syntax-roundtrip.md)
- [Content text encoding](../invariant/content-text-encoding.md) — stamp text.

## Blast radius
- [Annotations](annotations.md) — `add_annotation` attaches whatever this returns; a subtype added or dropped here changes which added annotations carry `/AP`.
- [Render annotations](render-annotations.md) — maps `/BBox` through `/Matrix` onto `/Rect` and clips to `/BBox` (`annot::placement`); a change to the coordinate rule here must hold under that mapping, or the shape is clipped away as it is now.
- [Form appearance](form-appearance.md), [Signature appearance](signature-appearance.md) — the two other generators writing appearance XObjects. All three draw justpdf-chosen text in a WinAnsi Helvetica (`standard_font_dict`, `encode_winansi`), the rule #175 would replace with one appearance font for all of them; form appearance also repeats the circle constant.

## Known holes / open
- Mixed coordinate systems (above): highlight, underline, strikeout, squiggly, square, circle, line, ink and Redact appearances draw nothing in justpdf's renderer and MuPDF. Tracked: #41
- No test renders an appearance stream this generator made (`justpdf-render/tests/render_annotations.rs` builds its appearances by hand).
- Stamp text beyond WinAnsi through a caller-supplied appearance font. Tracked: #175
