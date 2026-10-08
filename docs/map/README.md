# justpdf map

<!-- grill-map build stamp: eb0cd67 -->

This map answers two questions.

1. **Horizontal — "if I touch this, what else moves?"** Open the territory note the code you touch belongs to and follow its `## Blast radius` **as a checklist** (opening one and finding nothing to do is normal too). Then open the invariant notes under `## Cross-cutting invariants` and hold their `## Where it will recur` against your change.
2. **Vertical — "which design or decision did this code come from?"** Read the same note's `## Governing decisions` (why) and `## Design model` (rules). If it says `**None.**`, that gap is itself the answer: nobody decided.

## Reading rules

- **Read before working.** Open the territory and its invariants before settling a design. Opening them after the change is done defeats the purpose of this map.
- **Write after working.** When a change is finished, check two things.
  1. *Coverage* — does the touched territory have a note, and is its `## Blast radius` still right? Are the symbols in `## Code` still in their files?
  2. *Promotion* — **is the fact this fix exposed true outside this territory too?** For example: "does this function produce PDF syntax bytes outside `PdfObject`'s Display?", "does this function put human-readable text into `PdfObject::String`?" — grep can answer these. If it is true, the fix is not finished until an invariant note exists (or gains the site).
- When a note cites an issue, **write the fact first and attach the issue as a tracking pointer** ("X cannot do Y. Tracked: #N"). Do not write sentences that become false the moment the issue closes ("#N is open").

## Why this layer is needed

The same fact — **"PDF syntax justpdf writes must read back to the same value through justpdf's parser"** — was discovered three separate times, and each time only the one central site was fixed: 0be6cc7 (spaces in a Name → Korean text lost), 0be6cc7 (parentheses in a String), 7be23c0/#20 (high bytes in a String → a file encrypted with certain passwords would not open). Eight more hand-written sites that the same rule should have covered existed, and they only became a list while this map was being written — [Object syntax roundtrip](invariant/object-syntax-roundtrip.md). Preventing a fourth discovery is this map's acceptance criterion.

## Measured (2026-09-23)

- **Public surface vs decision records.** The workspace crates have hundreds of `pub fn` (most in core), and all three decision records (ADR-0001 to 0003) deal with **crate placement**. No record decides how the API behaves. The only territories with a decision record are notes that span a crate boundary (command below).
- **Mentioned vs subject.** "compress" is mentioned in the text of all three ADRs and is the subject of none — the worst state, since grep makes it look "covered".
- **File size.** `writer/compress.rs` (the largest file in the repo) and `justpdf-render/src/interpreter.rs` (the second) are each split more finely than the code tree: compression into 9 plus an [aggregate](territory/compress.md), the render interpreter into images, clipping, transparency, shading, patterns, glyphs, annotations and OCG.
- **Forward-looking statements vs reality.** The old `roadmap.md` had every checkbox done, but the code had inline image rendering as a no-op, no Type 0 functions, tile rendering not wired up, and a TODO for `/Properties` OCG lookup. CHANGELOG and the compression design doc listed the disabled object stream packing as done. **A stale prediction** (a document saying it is built as designed) is more dangerous than a stale pointer — planning from it assumes something exists that does not. On 2026-09-23 the roadmap was deleted and the rest brought in line with the facts.
- **Backlog.** Before the map was written, every open issue was on the compression line. Defects that surfaced while writing the map were filed as issues (#25–#60) and attached as tracking pointers under the notes' `## Known holes / open`.
- **Where existing docs are wrong** (this map does not edit decision records): [ADR-0002](../adr/0002-language-bindings-outside-workspace.md) says all four bindings are outside the workspace, but `justpdf-ffi` is in the root `members`, and `justpdf-wasm` has no `[workspace]` block, so cargo cannot resolve its manifest ([Language bindings](territory/language-bindings.md)). The README and mdBook examples were brought in line with the code the same day ([Published docs](territory/published-docs.md)).

## Conventions

- Territories **overlap**. One file can appear in several territories, and a fact true in several places becomes an invariant node.
- **Do not delete an empty section.** Write emptiness as the `**None.**` sentinel, then say why it is empty (what adjacent thing does not govern this area). The three sentinels are different gaps: *nobody decided* (Governing decisions), *nobody compared* (Reference behaviour), *nobody built it* (Code).
- `## Code` lists **symbol names** only. No line numbers.
- Links are relative-path markdown links (they work both in the Obsidian graph and on GitHub).
- Headings are fixed English strings; the commands below grep for them, so do not change a single character. Note bodies are in English too, as `CLAUDE.md`'s language rule asks.
- Site lists are split into **the half a tool can see** (with the grep command alongside) and **the half a tool cannot see** (sites that are an assumption, not a call, kept by hand).
- A fact is marked *measured* when it was seen by running something, and *inferred* when it was read from code and not confirmed by running it.
- Reference sources: ISO 32000-2 is **binding**, MuPDF and Ghostscript are **example**. Clause numbers under `## Reference behaviour` are only pointers to what to compare against; every note says `**None.**` until a record of checking against the spec text exists.

## What this map cannot answer

- Nodes are `.md` files only. Issues, PRs and source files exist only as text inside notes and do not show up in the graph view. The hottest places (open issues) are invisible in the graph.
- The external repository Just-pdf-web (the compress-wasm consumer) is not a node. It exists only as text in [compress-presets](territory/compress-presets.md)'s checklist.
- 8 of the 11 invariants were found in a single read with no recorded incident. Their `## Discovery history` says so — until a history of rediscovery builds up, they are predictions.

## Scope and what absence means

It covers **the whole repository**: every workspace crate, the four bindings outside the workspace, CI, release, distribution and published docs.

- **Code exists but no note → a gap.** A module, crate or feature that appeared after this map. A note becomes **owed** when the first piece of a new module is merged to master.
- **Only a plan, no code → no note is normal.** The issue tracker owns the list of plans.
- When one file holds several concepts (e.g. `compress.rs`, `interpreter.rs`), the notes are finer than the file. Find a note by the concept's name, not the file's.

## Gate

`python3 scripts/check-map.py .` (from the repository root) — append a path to check a single note. It checks: the section set (per territory, invariant and aggregate), that each `## Code` symbol is in the file listed, that links and `#anchors` resolve (ignoring code spans and fenced blocks), and invariant ↔ territory back-links.

Its scope is `docs/map/` only. It is not wired into CI, so passing only means "someone ran it". It does not check a note's **claims** (design rules, inferences, blast edges) — after a refactor, fix addresses with the symbol check, and if a symbol has **disappeared**, reread that note's `## Design model` first.

## Questions as commands

```sh
cd docs/map
# Territories nobody decided (the sentinel has to be scoped per section — the same sentinel appears in several sections)
rg -lU '## Governing decisions\r?\n\*\*None\.\*\*' territory/
# Territories with a decision record (aggregate notes have no such section, so they are left out of the base set)
comm -23 <(rg -l '^## Governing decisions' territory/ | sort) <(rg -lU '## Governing decisions\r?\n\*\*None\.\*\*' territory/ | sort)
# Territories never compared against a reference
rg -lU '## Reference behaviour\r?\n\*\*None\.\*\*' territory/
# Territories with no code (design only)
rg -lU '## Code\r?\n\*\*None\.\*\*' territory/
# Which territories claim this invariant (only links inside the ## Cross-cutting invariants section — mentions in the body do not count)
rg -lUP '## Cross-cutting invariants\n(?:(?!## ).*\n)*?.*invariant/object-syntax-roundtrip\.md' territory/
# Node list — the folders are the list
ls territory/ invariant/
```

## Nodes
- core — reading: [tokenizer](territory/tokenizer.md) · [object-model](territory/object-model.md) · [xref](territory/xref.md) · [object-streams](territory/object-streams.md) · [document-access](territory/document-access.md) · [repair](territory/repair.md) · [linearization](territory/linearization.md) · [page-tree](territory/page-tree.md)
- core — writing: [object-serialization](territory/object-serialization.md) · [file-serialization](territory/file-serialization.md) · [document-builder](territory/document-builder.md) · [document-modifier](territory/document-modifier.md) · [incremental-save](territory/incremental-save.md) · [clean](territory/clean.md) · [journal](territory/journal.md)
- Compression: [compress](territory/compress.md) · [compress-presets](territory/compress-presets.md) · [compress-pipeline](territory/compress-pipeline.md) · [compress-images](territory/compress-images.md) · [compress-grayscale](territory/compress-grayscale.md) · [font-subsetting](territory/font-subsetting.md) · [compress-stream-recompression](territory/compress-stream-recompression.md) · [compress-dedup](territory/compress-dedup.md) · [compress-unused-resources](territory/compress-unused-resources.md) · [compress-stripping](territory/compress-stripping.md)
- Streams, images, color: [stream-filters](territory/stream-filters.md) · [image-decoding](territory/image-decoding.md) · [color-spaces](territory/color-spaces.md) · [pdf-functions](territory/pdf-functions.md)
- Fonts and text: [font-loading](territory/font-loading.md) · [font-encodings](territory/font-encodings.md) · [tounicode](territory/tounicode.md) · [cid-fonts](territory/cid-fonts.md) · [cjk-font-embedding](territory/cjk-font-embedding.md) · [cff](territory/cff.md) · [opentype-layout](territory/opentype-layout.md) · [type3-fonts](territory/type3-fonts.md) · [font-recovery](territory/font-recovery.md) · [content-stream-parsing](territory/content-stream-parsing.md) · [content-interpreter](territory/content-interpreter.md) · [text-extraction](territory/text-extraction.md) · [reading-order](territory/reading-order.md) · [text-output-formats](territory/text-output-formats.md) · [text-search](territory/text-search.md) · [text-wrapping](territory/text-wrapping.md)
- Encryption and signing: [encryption-model](territory/encryption-model.md) · [key-derivation](territory/key-derivation.md) · [password-authentication](territory/password-authentication.md) · [object-decryption](territory/object-decryption.md) · [object-encryption](territory/object-encryption.md) · [permissions](territory/permissions.md) · [signature-detection](territory/signature-detection.md) · [signing](territory/signing.md) · [signature-verification](territory/signature-verification.md) · [timestamps](territory/timestamps.md) · [signature-appearance](territory/signature-appearance.md)
- Interactive features: [annotations](territory/annotations.md) · [annotation-appearance](territory/annotation-appearance.md) · [redaction](territory/redaction.md) · [acroform](territory/acroform.md) · [form-fill](territory/form-fill.md) · [form-flatten](territory/form-flatten.md) · [form-appearance](territory/form-appearance.md) · [actions](territory/actions.md) · [outlines](territory/outlines.md) · [optional-content](territory/optional-content.md) · [page-labels](territory/page-labels.md) · [embedded-files](territory/embedded-files.md)
- Rendering: [render-interpreter](territory/render-interpreter.md) · [raster-device](territory/raster-device.md) · [svg-renderer](territory/svg-renderer.md) · [bbox-device](territory/bbox-device.md) · [display-list](territory/display-list.md) · [glyph-rendering](territory/glyph-rendering.md) · [render-shading](territory/render-shading.md) · [render-tiling-patterns](territory/render-tiling-patterns.md) · [render-images](territory/render-images.md) · [render-clipping](territory/render-clipping.md) · [render-transparency](territory/render-transparency.md) · [render-annotations](territory/render-annotations.md) · [render-api](territory/render-api.md)
- Products: [facade](territory/facade.md) · [cli](territory/cli.md) · [compress-wasm](territory/compress-wasm.md) · [format-document](territory/format-document.md) · [format-detection](territory/format-detection.md) · [xps-input](territory/xps-input.md) · [epub-input](territory/epub-input.md) · [office-input](territory/office-input.md) · [svg-input](territory/svg-input.md) · [cbz-input](territory/cbz-input.md) · [mobi-input](territory/mobi-input.md) · [fb2-input](territory/fb2-input.md) · [plaintext-input](territory/plaintext-input.md) · [ocr](territory/ocr.md) · [barcode](territory/barcode.md) · [zugferd](territory/zugferd.md) · [bidi](territory/bidi.md) · [deskew](territory/deskew.md) · [language-bindings](territory/language-bindings.md) · [ffi-binding](territory/ffi-binding.md) · [python-binding](territory/python-binding.md) · [wasm-binding](territory/wasm-binding.md) · [node-binding](territory/node-binding.md)
- Infrastructure: [crate-layering](territory/crate-layering.md) · [ci](territory/ci.md) · [release](territory/release.md) · [crate-publishing](territory/crate-publishing.md) · [published-docs](territory/published-docs.md)
- Invariants: [content-stream-recursion](invariant/content-stream-recursion.md) · [content-text-encoding](invariant/content-text-encoding.md) · [font-resolution](invariant/font-resolution.md) · [image-pixel-layout](invariant/image-pixel-layout.md) · [incremental-trailer](invariant/incremental-trailer.md) · [object-syntax-roundtrip](invariant/object-syntax-roundtrip.md) · [page-content-assembly](invariant/page-content-assembly.md) · [resource-scope](invariant/resource-scope.md) · [source-generation](invariant/source-generation.md) · [text-string-encoding](invariant/text-string-encoding.md) · [tree-traversal-cycles](invariant/tree-traversal-cycles.md) · [xref-entry-format](invariant/xref-entry-format.md)
