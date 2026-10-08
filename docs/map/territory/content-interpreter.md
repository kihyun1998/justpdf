# Content interpreter

## What it is
The graphics- and text-state interpreter in core that walks a content stream's parsed operations (`interpret`) and reports to a `ContentVisitor`: each operation with the state in effect before it runs, and each shown glyph (`Glyph`) with its code, the operation and `TJ` element it came from, its bytes in that string, its width, its text rendering matrix and its box. Resource names resolve through `ContentResources`: `DocResources` reads a document, and the test-only `MapResources` reads maps. [Text extraction](text-extraction.md) is built on it, and [Redaction](redaction.md) is meant to be (ADR 0004). Crate-internal (`pub(crate)`): the public API stays the same, and `text::Matrix` is a re-export.

## Governing decisions
- [ADR 0004 — Redaction never under-erases](../../adr/0004-redaction-never-under-erases.md), section "One interpreter for extraction and redaction": glyph positions for redaction come from the interpreter text extraction uses, so a glyph extraction reports inside an area is one redaction erases. The glyph box rule (advance width by the `/FontBBox` height, −0.25…1.0 × size without one, through the text rendering matrix and the CTM) is the ADR's.

## Design model
- Tracks q/Q, `cm`, Tm/Tlm, and the text state Tc, Tw, Tz, TL, Tf, Ts, Tr (ISO 32000-2 §8.4, §9.3). Tm is not part of the saved state, as the spec says. `T*`, `'` and `"` move down by the current TL, and `TD` sets TL to −ty.
- Glyph displacement and position follow §9.4.4 exactly as text extraction computed them before the move (#207), and extraction output is unchanged — *measured* 2026-10-08: `justpdf text --format json` on all 30 PDFs in the repository and 11 local PDFs (~80 MB of JSON) gives byte-identical output before and after. That includes a behaviour kept as it was: word spacing is added to any code 32, including a two-byte one (the spec limits it to a single-byte code 32).
- The glyph box spans the advance width (the font's width, without Tc or Tw) in glyph space, and the `/FontBBox` height, through the text rendering matrix, so Tz, Ts, Tm and the CTM all apply. A `/FontBBox` given by its other two corners is normalised (§7.9.5 allows any two opposite corners). One with no height (`[0 0 0 0]` is common) is treated as absent and the default height is used, because a zero-height box would erase nothing (ADR 0004: when unsure, erase). The box uses the font's own descriptor, or a Type0's descendant's. A font with no resolvable `Tf` gets width 500 and the default height.
- `/FontDescriptor` is resolved by `font::resolve_font_entries`, so `FontInfo::descriptor` (and its `/FontBBox`) is filled in for every consumer. Before #207 an indirect descriptor (the usual case) read as absent; every consumer of the resolved dictionary already accepted both a reference and a dictionary there (*inferred*, read 2026-10-08: the raster and SVG renderers' descriptor readers, `simple_font_glyph_ids`).
- A font is resolved **when `Tf` runs**, and the text state holds the resolved font, not the name — [Resource name scope](../invariant/resource-scope.md). Each scope chain remembers what a name resolved to, so a repeated `Tf` does not look the name up again; fonts reached by reference are also cached by reference across the page and its forms.
- Form XObjects are entered only with `InterpretOptions::enter_forms` (off for text extraction; turning it on there is #48). Entering applies `/Matrix`, pushes the form's `/Resources` in front of the caller's (a form without `/Resources` uses the caller's), saves the graphics state and the text matrices, and restores them on leaving. Inside a form, `Q` cannot pop below the stack depth at which the form was entered. A form already being interpreted is refused as `FormRefusal::Cycle`; one that would be the eleventh nested form is refused as `FormRefusal::Depth` (`MAX_FORM_DEPTH` = 10; the raster renderer enters up to 11, `xobject_depth > 10`). `/BBox` clipping is not tracked.
- `gs` applies only the ExtGState's `/Font` entry (#235): when it is a direct two-element array of an indirect font reference and a number, the font and size become those, as `Tf` would set them, and the font name becomes empty. The font is looked up by reference (`DocResources::font_by_ref`), sharing the cache with the same object reached through `Tf`, so it need not appear in any `/Font` dictionary. Any other shape leaves font and size unchanged — the same rule as the raster and SVG renderers (#238), as [Font resolution](../invariant/font-resolution.md) asks. The ExtGState name resolves innermost scope first, like every resource name. Other ExtGState keys are ignored.
- Not interpreted: marked content (`BDC`/`EMC`, #219), clipping, colour, paths and images. Codes are split as before: two bytes for an `Identity` encoding or a Type0 font, otherwise one (#303 is about CMaps other than Identity).

## Code
- `justpdf-core/src/content/interpret.rs` — `interpret`, `InterpretOptions`, `ContentVisitor`, `Glyph`, `FormRefusal`, `MAX_FORM_DEPTH`, `ContentResources`, `ext_gstate_font`, `FormXObject`, `DocResources`, `font_by_ref`, `MapResources`, `ResolvedFont`, `load_font`, `GraphicsState`, `TextState`, `Matrix`
- `justpdf-core/src/font/mod.rs` — `resolve_font_entries`
- `justpdf-core/tests/extgstate_font.rs` — `gs_sets_the_font_and_size_with_no_tf`, `a_malformed_font_entry_changes_nothing`

## Reference behaviour
**None.** Clauses to compare against: ISO 32000-2 §8.4 (graphics state), §9.3 (text state), §9.4.4 (text space details), §8.10 (Form XObjects).

## Cross-cutting invariants
- [Font resolution](../invariant/font-resolution.md) — `load_font`.
- [Resource name scope](../invariant/resource-scope.md) — `DocResources`, `Tf`.

## Blast radius
- [Text extraction](text-extraction.md) — built on it; any change in position or width shows in extracted text and in [Reading order](reading-order.md), [Text output formats](text-output-formats.md) and [Text search](text-search.md).
- [Redaction](redaction.md) — meant to take glyph boxes from here (ADR 0004).
- [Content stream parsing](content-stream-parsing.md) — its input.
- [Font loading](font-loading.md), [CID fonts](cid-fonts.md), [ToUnicode](tounicode.md) — `load_font`'s input.
- [Render interpreter](render-interpreter.md), [SVG renderer](svg-renderer.md) — separate operator walkers in `justpdf-render`, which depends on core; a positioning fix on one side has to decide whether the other needs it.

## Known holes / open
- Text inside Form XObjects is not extracted, because extraction does not turn on `enter_forms`. Tracked: #48.
- Hidden optional content is not told apart. Tracked: #219.
- Codes are split by the font's encoding name, not by its CMap. Tracked: #303.
