# justpdf map

<!-- grill-map build stamp: d525035 -->
<!-- grill-map synced at: d38cda8 (2026-10-26) -->

This map answers two questions.

1. **Horizontal — "if I touch this, what else moves?"** Open the territory note the code you touch belongs to and follow its `## Blast radius` **as a checklist** (opening one and finding nothing to do is normal too). Then open the invariant notes under `## Cross-cutting invariants` and hold their `## Where it will recur` against your change.
2. **Vertical — "which design or decision did this code come from?"** Read the same note's `## Governing decisions` (why) and `## Design model` (rules). A `**None.**` there is itself the answer: nobody decided.

## Reading rules

- **Read before designing a change.** Open the territory and its invariants before settling a design; opening them after the change is done defeats the purpose of this map.
- **Write after it lands.** Check that the touched notes' `## Code` symbols are still in their files, and that their blast edges and holes still match the code.
- **Promotion, at the first fix.** When a fix exposes a fact, ask in a form grep can answer whether it holds at every site that shares the assumption — "does this function produce PDF syntax bytes outside `PdfObject`'s Display?", "does this function put human-readable text into `PdfObject::String`?". If it does, the fix lands with an invariant note, or a new site in an existing one.
- When a note cites an issue, **the fact comes first and the issue is a tracking pointer** ("X cannot do Y. Tracked: #N"). A sentence that turns false when the issue closes ("#N is open") is not written.

## Why this layer exists

The same fact — **"PDF syntax justpdf writes must read back to the same value through justpdf's parser"** — was found three separate times, and each time only the one central site was fixed: 0be6cc7 (spaces in a Name → Korean text lost), 0be6cc7 (parentheses in a String), 7be23c0/#20 (high bytes in a String → a file encrypted with certain passwords would not open). Eight more hand-written sites the same rule should have covered only became a list while the first version of this map was written (2026-09-23) — [Object syntax roundtrip](invariant/object-syntax-roundtrip.md). Preventing a fourth discovery is this map's acceptance criterion.

## The repository's shape

A pure-Rust PDF engine in eight workspace crates and three bindings outside it. `justpdf-core` (about 63,000 lines of `src/` at d38cda8, measured) holds the PDF specification itself: reading (tokenizer, objects, xref and object streams, repair, filters, decryption), the shared content interpreter that text extraction runs on, fonts, writing (builder, modifier, serializer, incremental save, linearization, encryption), compression, document features and signing. `justpdf-render` (about 8,500 lines) rasterizes with tiny-skia and writes SVG through its own content interpreter. `justpdf-formats` and `justpdf-special` add input converters and add-on features; `justpdf` is the facade over core and render; `justpdf-cli` is the one CLI (ADR-0003); `justpdf-compress-wasm` is the browser compressor that the external repository `kihyun1998/just-pdf-web` consumes. The crate split is by external dependency (ADR-0001).

What the orientation for this map found that belongs to no single note (re-measured 2026-10-30 at d38cda8):

- **Public surface vs decision records.** About 485 `pub fn` lines in core, 86 in render and 70 in the facade (`grep -c 'pub fn'`, measured). Six decision records exist: three place crates (ADR-0001–0003), two decide behaviour — redaction (ADR-0004) and sanitizing (ADR-0005) — and one decides that the public API may break before 1.0 (ADR-0006). Every other behaviour is decided only in code and issues; that is what most `## Governing decisions` sections say.
- **Decided, not built.** ADR-0004's redaction rules are mostly not what `annot/redact.rs` does yet, and ADR-0005's sanitize has no code — [Redaction](territory/redaction.md), [Sanitize](territory/sanitize.md). A reader planning from the records alone assumes behaviour that does not exist.
- **Three content interpreters.** The render interpreter, the SVG renderer and core's shared content interpreter each walk content operators, none calling another; four invariants exist because of it ([Content stream recursion](invariant/content-stream-recursion.md), [Hidden content state](invariant/hidden-content-state.md), [Resource scope](invariant/resource-scope.md), [Page content assembly](invariant/page-content-assembly.md)).
- **File size vs notes.** `writer/compress.rs` (4,667 lines) and `justpdf-render/src/interpreter.rs` (2,736) are each split into finer notes than the file: compression into nine members under an [aggregate](territory/compress.md), the render interpreter into images, clipping, transparency, shading, patterns, glyphs and annotations. `font/agl.rs` (4,330 lines) is a generated table (`scripts/gen-font-tables.py`) and sits in [Font encodings](territory/font-encodings.md).
- **A stale prediction is worse than a stale pointer** (measured 2026-09-23, when the first version of this map was written): the old `roadmap.md` had every box ticked while inline images, Type 0 functions and tile rendering were missing, and CHANGELOG listed disabled object-stream packing as done. The roadmap was deleted that day.

## Conventions

- Territories **overlap**. One file can sit in several territories, and a fact true in several places that do not call each other becomes an invariant note.
- **An empty section stays**, with the sentinel `**None.**` and a line saying why. The sentinels mark different gaps: *nobody decided* (Governing decisions), *nobody compared* (Reference behaviour), *nobody built it* (Code).
- `## Code` lists **symbols by file**, never line numbers.
- Links are relative markdown links (they work on GitHub and in Obsidian's graph view).
- Section headings are fixed English strings; the queries below grep for them.
- A list of sites is split into the half a tool can see (with its command) and the half no tool can see (assumptions, not calls), each labelled.
- Every claim carries its evidence (a symbol, a record, a commit) or its source and date. A fact is *measured* when it was seen by running something and *inferred* when read from code.
- Reference sources ([`docs/agents/thegraph.md`](../agents/thegraph.md)): ISO 32000-2 is **binding**; MuPDF and Ghostscript are **examples**.

## What this map cannot answer

- Nodes are `.md` files only. Issues, PRs and source files appear only as text inside notes and do not show up in a graph view; the open backlog is invisible there.
- The external repository `kihyun1998/just-pdf-web` (private) is not a node. Its seam with `justpdf-compress-wasm` was read at d4410b8 (2026-10-06) and lives in [compress-wasm](territory/compress-wasm.md).
- Most invariants were found in a single reading with no recorded incident; their `## Discovery history` says so. Until a rediscovery is recorded they are predictions.

## Scope and what absence means

The whole repository: every workspace crate, the three bindings outside the workspace, CI, release, crate publishing and the published docs.

Out of scope, by reason:
- `logo/` — brand assets; the READMEs and docs.rs attributes that show them are in [Published docs](territory/published-docs.md).
- `.out-of-scope/` — triage records of rejected requests; they bind the issue tracker, not code.
- `docs/mupdf-feature-analysis.md` — a reading of MuPDF's feature set, a planning input; no code depends on it.
- `dev/pdf-compress-wasm-design.md` — the browser compressor's original design, read as a lead for [compress-wasm](territory/compress-wasm.md).
- `justpdf-core/examples/`, `benches/` in core and render, and the test fixtures — debugging and measuring tools; nothing ships them. A test or example that pins a note's fact is cited in that note.

What an absent note means:
- **Code exists and no note covers it → a gap.** A note is owed when the first piece of a new module lands on master.
- **A plan with nothing behind it → no note is right.** The issue tracker owns plans. A territory designed and not built gets a note once something in the repo commits to it — a decision record, a spec, an issue carrying a design ([Sanitize](territory/sanitize.md) is one).
- When one file holds several concepts, the notes are finer than the file: find a note by the concept's name, not the file's.

## Questions as commands

```sh
cd docs/map
# Territories nobody decided (the sentinel is scoped per section — it appears in several)
rg -lU '## Governing decisions\r?\n(\r?\n)?\*\*None\.\*\*' territory/
# Territories never compared against a reference
rg -lU '## Reference behaviour\r?\n(\r?\n)?\*\*None\.\*\*' territory/
# Territories with no code (designed, not built)
rg -lU '## Code\r?\n(\r?\n)?\*\*None\.\*\*' territory/
# Which territories claim this invariant (links inside ## Cross-cutting invariants only)
rg -lUP '## Cross-cutting invariants\n(?:(?!## ).*\n)*?.*invariant/object-syntax-roundtrip\.md' territory/
# Node list — the folders are the list
ls territory/ invariant/
```

## Nodes
- Reading: [tokenizer](territory/tokenizer.md) · [object-model](territory/object-model.md) · [xref](territory/xref.md) · [object-streams](territory/object-streams.md) · [document-access](territory/document-access.md) · [repair](territory/repair.md) · [linearization](territory/linearization.md) · [page-tree](territory/page-tree.md)
- Writing: [object-serialization](territory/object-serialization.md) · [file-serialization](territory/file-serialization.md) · [document-builder](territory/document-builder.md) · [document-modifier](territory/document-modifier.md) · [incremental-save](territory/incremental-save.md) · [clean](territory/clean.md) · [journal](territory/journal.md)
- Compression: [compress](territory/compress.md) · [compress-presets](territory/compress-presets.md) · [compress-pipeline](territory/compress-pipeline.md) · [compress-images](territory/compress-images.md) · [compress-grayscale](territory/compress-grayscale.md) · [font-subsetting](territory/font-subsetting.md) · [compress-stream-recompression](territory/compress-stream-recompression.md) · [compress-dedup](territory/compress-dedup.md) · [compress-unused-resources](territory/compress-unused-resources.md) · [compress-stripping](territory/compress-stripping.md)
- Streams, images, colour: [stream-filters](territory/stream-filters.md) · [image-decoding](territory/image-decoding.md) · [color-spaces](territory/color-spaces.md) · [pdf-functions](territory/pdf-functions.md)
- Fonts and text: [font-loading](territory/font-loading.md) · [font-encodings](territory/font-encodings.md) · [tounicode](territory/tounicode.md) · [cid-fonts](territory/cid-fonts.md) · [cjk-font-embedding](territory/cjk-font-embedding.md) · [cff](territory/cff.md) · [opentype-layout](territory/opentype-layout.md) · [type3-fonts](territory/type3-fonts.md) · [font-recovery](territory/font-recovery.md) · [content-stream-parsing](territory/content-stream-parsing.md) · [content-interpreter](territory/content-interpreter.md) · [text-extraction](territory/text-extraction.md) · [reading-order](territory/reading-order.md) · [text-output-formats](territory/text-output-formats.md) · [text-search](territory/text-search.md) · [text-wrapping](territory/text-wrapping.md)
- Encryption and signing: [encryption-model](territory/encryption-model.md) · [key-derivation](territory/key-derivation.md) · [password-authentication](territory/password-authentication.md) · [object-decryption](territory/object-decryption.md) · [object-encryption](territory/object-encryption.md) · [permissions](territory/permissions.md) · [signature-detection](territory/signature-detection.md) · [signing](territory/signing.md) · [signature-verification](territory/signature-verification.md) · [timestamps](territory/timestamps.md) · [signature-appearance](territory/signature-appearance.md)
- Interactive features: [annotations](territory/annotations.md) · [annotation-appearance](territory/annotation-appearance.md) · [redaction](territory/redaction.md) · [sanitize](territory/sanitize.md) · [acroform](territory/acroform.md) · [form-fill](territory/form-fill.md) · [form-flatten](territory/form-flatten.md) · [form-appearance](territory/form-appearance.md) · [actions](territory/actions.md) · [outlines](territory/outlines.md) · [optional-content](territory/optional-content.md) · [page-labels](territory/page-labels.md) · [embedded-files](territory/embedded-files.md)
- Rendering: [render-api](territory/render-api.md) · [render-interpreter](territory/render-interpreter.md) · [raster-device](territory/raster-device.md) · [svg-renderer](territory/svg-renderer.md) · [bbox-device](territory/bbox-device.md) · [display-list](territory/display-list.md) · [glyph-rendering](territory/glyph-rendering.md) · [render-shading](territory/render-shading.md) · [render-tiling-patterns](territory/render-tiling-patterns.md) · [render-images](territory/render-images.md) · [render-clipping](territory/render-clipping.md) · [render-transparency](territory/render-transparency.md) · [render-annotations](territory/render-annotations.md)
- Products: [facade](territory/facade.md) · [cli](territory/cli.md) · [compress-wasm](territory/compress-wasm.md) · [format-document](territory/format-document.md) · [format-detection](territory/format-detection.md) · [xps-input](territory/xps-input.md) · [epub-input](territory/epub-input.md) · [office-input](territory/office-input.md) · [svg-input](territory/svg-input.md) · [cbz-input](territory/cbz-input.md) · [mobi-input](territory/mobi-input.md) · [fb2-input](territory/fb2-input.md) · [plaintext-input](territory/plaintext-input.md) · [ocr](territory/ocr.md) · [barcode](territory/barcode.md) · [zugferd](territory/zugferd.md) · [bidi](territory/bidi.md) · [deskew](territory/deskew.md) · [language-bindings](territory/language-bindings.md) · [ffi-binding](territory/ffi-binding.md) · [python-binding](territory/python-binding.md) · [wasm-binding](territory/wasm-binding.md) · [node-binding](territory/node-binding.md)
- Infrastructure: [crate-layering](territory/crate-layering.md) · [ci](territory/ci.md) · [release](territory/release.md) · [crate-publishing](territory/crate-publishing.md) · [published-docs](territory/published-docs.md)

## Invariants
[bit-widths](invariant/bit-widths.md) · [clip-timing](invariant/clip-timing.md) · [content-stream-recursion](invariant/content-stream-recursion.md) · [content-text-encoding](invariant/content-text-encoding.md) · [encryption-identity](invariant/encryption-identity.md) · [font-resolution](invariant/font-resolution.md) · [hidden-content-state](invariant/hidden-content-state.md) · [image-pixel-layout](invariant/image-pixel-layout.md) · [incremental-trailer](invariant/incremental-trailer.md) · [object-syntax-roundtrip](invariant/object-syntax-roundtrip.md) · [page-content-assembly](invariant/page-content-assembly.md) · [resource-scope](invariant/resource-scope.md) · [signature-contents](invariant/signature-contents.md) · [source-generation](invariant/source-generation.md) · [text-string-encoding](invariant/text-string-encoding.md) · [tree-traversal-cycles](invariant/tree-traversal-cycles.md) · [xml-nesting-depth](invariant/xml-nesting-depth.md) · [xref-entry-format](invariant/xref-entry-format.md)
