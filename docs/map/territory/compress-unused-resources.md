# Compress unused resources

## What it is
For each page, collects the `Tf` (font), `Do` (XObject) and `gs` (ExtGState) names its content uses, follows used Form XObjects for more names, and deletes the entries of the page's `/Font`, `/XObject` and `/ExtGState` resource dictionaries that no collected name matches. The GC that follows removes the objects nothing references any more.

## Governing decisions
**None.** `dev/pdf-compress-wasm-design.md` Phase E describes the stage (collect names from `Tf`/`Do`/`gs`, remove unused entries, let GC collect the orphans). It is a design doc, not a decision record.

## Design model
- **Pages** are found by scanning every object for `/Type /Page`. A page whose `/Contents` is not a reference or a direct array, whose content decodes to nothing, or whose content does not parse is skipped. A page without its own `/Resources` (inherited from the page tree) is skipped (`remove_unused_resources`, read at 88b2a08).
- Only three subdictionaries are cleaned: `Font`, `XObject` and `ExtGState`. `ColorSpace`, `Pattern`, `Shading` and `Properties` are left alone (no record says why).
- Use is judged from the parsed content only. Resources used by annotation appearance streams are not collected (inferred).
- **Form recursion** reads each used form's own content into the page's name sets, and looks up a nested `Do` in the page's `/XObject` map, not the form's own `/Resources` (Tracked: #295). A global `checked` set of names ends the recursion.
- **The cleaned dictionary is written to wherever the page's `/Resources` lives.** When `/Resources` is a reference, the resources object itself is overwritten with the page's cleaned copy. When a subdictionary (`/Font`) is a reference, a cleaned direct copy replaces the reference in the resources dictionary.
- **A `/Resources` object shared by several pages loses the names every page but the last one used.** Each page cleans the shared object against its own names and writes it back, and the next page starts from that result. Measured 2026-10-30 (`justpdf compress --preset medium -v`, a hand-built two-page file whose pages share `/Resources 5 0 R` with `/F1` Helvetica and `/F2` Courier, page 1 drawing with `/F1` and page 2 with `/F2`): the output's object 5 is `<< /Font << >> >>`, "Unused resources removed: 2", "Objects removed (GC): 2" — both fonts are gone while both pages still select them.
- **Input bounds**: one pass per page plus each form reached by name, each name once per page (`checked`). Nothing bounds a page's content size.

## Code
- `justpdf-core/src/writer/compress.rs` — `remove_unused_resources`, `clean_resource_subdict`, `count_removed`, `extract_xobject_map`, `test_remove_unused_resources_valid`, `test_remove_unused_resources_text_preserved`, `test_remove_unused_resources_disabled_low`, `test_remove_unused_resources_images_intact`

## Reference behaviour
**None.**

## Cross-cutting invariants
- [Resource scope](../invariant/resource-scope.md) — a name inside a form resolves in the form's `/Resources` first. The form recursion here reads form names into the page's scope instead, which is the violation #295 tracks.
- [Page content assembly](../invariant/page-content-assembly.md) — `remove_unused_resources` matches `/Contents` as a reference or a direct array, missing an indirect array (#138).

## Blast radius
- [Content stream parsing](content-stream-parsing.md) — a name the parser does not return as the first operand of `Tf`/`Do`/`gs` counts as unused, and its resource is deleted.
- [Compress pipeline](compress-pipeline.md) — GC runs after this stage and removes what it unhooked, so a wrongly deleted entry loses its object for good.
- [Optional content](optional-content.md) — `/Properties` (marked-content and OCG names) is kept on purpose or by omission; no record says which.
- [Page tree](page-tree.md) — the stage assumes each page owns its `/Resources`; inherited and shared resources are where it skips or over-deletes.
- [Compress presets](compress-presets.md) — the `remove_unused_resources` knob is on from `medium` up and has no CLI flag.

## Known holes / open
- A `/Resources` object shared by several pages loses fonts, images and ExtGStates that only the earlier pages use (measured above). Tracked: #420.
- The form recursion reads a form's names into the page's sets and looks up a nested `Do` in the page's map. Measured 2026-10-08 on #295: a page `/F2` (Courier) used by nothing survives because a drawn form uses `/F2` for its own Times-Roman; renaming the form's font to `/F3` removes it. Over-keeping is measured; under-keeping is inferred for one narrow shape and not reproduced (#295 triage, 2026-10-08). The shared content interpreter with a form hook it waited for has landed (#207, closed 2026-10-08), and `compress.rs` does not call it yet (grep, 2026-10-30). Tracked: #295.
- Resources used outside page content (annotation appearances, patterns) but held in a page's resource dictionary can be deleted (inferred, no test).
- A page whose `/Contents` is a reference to an array is skipped, which keeps its resources (inferred). Decided on #138: one compress-local helper reads `/Contents` for all four compress sites. Tracked: #138.
- No test removes an unused `Font` entry. With the current `Font` API a page's Resources holds only the fonts its content selects (`PageBuilder::write_tf` registers a font when `set_font` draws with it); only the deprecated `PageBuilder::add_font` and `PageBuilder::add_font_ref` register a font without drawing it (read at 88b2a08). An unused test helper in `writer/compress.rs` that tried to build such a page was removed in the clippy cleanup of #189 (old map note, not re-checked).
