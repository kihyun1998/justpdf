# Text search

## What it is
Finds a string in a page's extracted text — exactly, case-insensitively, or by a small regular-expression subset — and returns each match with its matched text, its character range in `PageText::chars` and a bounding rectangle (`TextQuad`). The facade's `Document::search`, `Page::search` and `Page::search_case_insensitive` call `search_page`; the module's multi-page `search_exact`, `search_case_insensitive` and `search_regex` have no caller outside its tests.

## Governing decisions
**None.**

## Design model
From reading the source (`text/search.rs`); no record.
- **What is searched**: the concatenation of every `TextChar::unicode` on the page, in content order — not the words or lines. A space exists in it only where the page shows a space glyph, so `Hello World` is not found on a page that positions the words apart with no space glyph (measured 2026-10-30, probe: 0 matches over chars `HelloWorld`). Matches can span line breaks of the layout.
- **Exact and case-insensitive** (`search_page_exact`): `str::find` repeated from one byte after each match. Case-insensitive search lowercases text and pattern, then maps byte positions found in the lowercased text back through the original text's byte offsets; lowercasing that changes a character's UTF-8 length shifts every later position (measured 2026-10-30: case-insensitive `x` in `İx` finds nothing).
- **Regex** (`SimpleRegex`, instead of a regex crate): literals, `.`, `\d \w \s \D \W \S`, `[…]` and `[^…]`, `* + ? {n} {n,} {n,m}`, top-level `|`, and a leading `(?i)`. Everything else is a literal: `(ab)` matches the text `(ab)` and `^ab` matches nothing at a line start (measured 2026-10-30). Only a trailing `\` or a malformed `{…}` fails to compile: `search_regex` returns `Err` for those, `search_page` returns no matches. Matching backtracks per start position.
- **Quad** (`compute_quad`): the box over the matched characters from their minimum baseline to baseline + font size, and from the first x to the furthest `x + width`. It has no descent, and `width` is in text space (#318), so a match in text sized through `Tm` gets a box much narrower than the glyphs.
- `SearchOptions::max_results` caps results per page (0 = unlimited).
- **Bounds on input**: the pattern comes from the caller and the text from the page; neither is bounded. Regex search rebuilds the page's character vector on every match attempt (`find_at`), so its time grows with the square of the number of matches (measured 2026-10-30, release: pattern `a` over 5,000 / 10,000 / 20,000 `a`s took 0.12 / 0.45 / 1.84 s, exact search under 2 ms).

## Code
- `justpdf-core/src/text/search.rs` — `TextQuad`, `SearchResult`, `SearchOptions`, `search_page`, `search_page_exact`, `search_page_regex`, `build_flat_text`, `byte_pos_to_char_index`, `compute_quad`, `match_regex`, `SimpleRegex`, `SimpleRegex::compile`, `SimpleRegex::find_at`, `split_alternatives`, `compile_sequence`, `parse_char_class`, `parse_quantifier_braces`, `search_exact`, `search_case_insensitive`, `search_regex`
- `justpdf/src/lib.rs` — `Document::search`, `Page::search`, `Page::search_case_insensitive`

## Reference behaviour
**None.** No comparison with MuPDF's search (`fz_search_stext_page`) is recorded.

## Cross-cutting invariants
**None.**

## Blast radius
- [Text extraction](text-extraction.md) — `TextChar` order, Unicode, `width` and `font_size` are the search text and the quads; a change in how spaces or widths are produced changes what is found and where.
- [Facade](facade.md) — the only product caller; `search` results are public types (`SearchResult`, `TextQuad`).
- [Published docs](published-docs.md) — `docs/src/text-extraction.md` shows `doc.search` and `search_case_insensitive`.

## Known holes / open
- Exact search for a pattern starting with a non-ASCII character panics after the first match: the next search starts one byte into that character (measured 2026-10-30, probe: `é` in `aéb é` panics at `search.rs` `search_page_exact`, "byte index 2 is not a char boundary"). Every facade search goes through this path. Tracked: #416.
- Case-insensitive positions are wrong when lowercasing changes byte length (measured, above). Tracked: #416.
- Unsupported regex syntax (groups, anchors) is matched literally instead of being rejected (measured, above).
- Regex search is quadratic in the number of matches (measured, above).
- Queries containing spaces miss text laid out without space glyphs (measured, above); depends on #298 and #318 for how extraction produces spaces and widths.
- Open question for the maintainer: keep the hand-written `SimpleRegex`, or take a regex crate dependency?
