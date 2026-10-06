# Compress unused resources

## What it is
Collects the `Tf` (font), `Do` (XObject) and `gs` (ExtGState) names that page content actually uses, and deletes the entries of the resource dict that are not used. Recurses into Form XObjects. The GC that follows sweeps up the deleted objects.

## Governing decisions
**None.**

## Design model
- Only three subdicts are cleaned: `Font`, `XObject` and `ExtGState`. `ColorSpace`, `Pattern`, `Shading` and `Properties` are left alone.
- Use is judged only from the content stream parsing result. Resources used by annotation appearance streams are not collected separately (inferred).

## Code
- `justpdf-core/src/writer/compress.rs` — `remove_unused_resources`, `clean_resource_subdict`, `count_removed`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [Content stream parsing](content-stream-parsing.md) — the input to the use check.
- [Optional content](optional-content.md) — no record says why `/Properties` is kept.
- [Compress pipeline](compress-pipeline.md) — the GC that follows.
- [Compress presets](compress-presets.md) — the `remove_unused_resources` knob (also missing from the plan for CLI exposure).

## Known holes / open
- In a file where resources used outside pages (annotation appearances, patterns) are shared through the page resources, they can be deleted (inferred, no test).
- `DocumentBuilder` cannot put a font into a page's Resources that the content does not use (a font not passed to `PageBuilder::add_font` is not in Resources at all), so a test of removing an unused `Font` entry has to build or patch that Resources dictionary by hand. An unused helper in `writer/compress.rs` tests that tried it with the builder was removed in #189.
