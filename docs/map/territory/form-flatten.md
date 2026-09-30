# Form flatten

## What it is
Draws the field widgets' appearances into the page content and removes `/AcroForm`, turning the form into static content.

## Governing decisions
**None.**

## Design model
- It uses `/AP /N` only when it is a reference. A checkbox's state dictionary (`/Yes`/`/Off`) is skipped, but the widget is still consumed.
- It writes `q w 0 0 h llx lly cm /Fm{objnum} Do Q` but **does not register `/Fm{n}` in the page's `/Resources /XObject`** — it references an undefined name.
- The `cm` applies a (w, h) scale once more on top of a BBox that is already w×h (inferred).

## Code
- `justpdf-core/src/form/flatten.rs` — `flatten_form`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [Form appearance](form-appearance.md) — the coordinate rule of the appearances it draws in.
- [Render interpreter](render-interpreter.md) — the side that draws the flattened result (the undefined XObject name).
- [AcroForm](acroform.md), [Document modifier](document-modifier.md).

## Known holes / open
- The inline test module is empty. `test_flatten_form` only checks that `/AcroForm` is gone.
- Tracked: #40 (form fill and flatten)
