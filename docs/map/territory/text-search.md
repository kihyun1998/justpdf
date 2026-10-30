# Text search

## What it is
Finds a string in a page's extracted text — exactly, case-insensitively, or by a small regular-expression subset — and returns each match with its matched text, its character range in `PageText::chars` and a bounding rectangle (`TextQuad`). The facade's `Document::search`, `Page::search` and `Page::search_case_insensitive` call `search_page`; the module's multi-page `search_exact`, `search_case_insensitive` and `search_regex` have no caller outside its tests.

## Governing decisions
**None.**

## Design model
From reading the source (`text/search.rs`); no record.
- **What is searched**: the concatenation of every `TextChar::unicode` on the page, in content order — not the words or lines. A space exists in it only where the page shows a space glyph, so `Hello World` is not found on a page that positions the words apart with no space glyph (measured 2026-10-30, probe: 0 matches over chars `HelloWorld`). Matches can span line breaks of the layout.
- **Exact and case-insensitive** (`search_page_exact`): `str::find` repeated from one character after each match's start, so overlapping matches are found and no slice falls inside a character. Case-insensitive search lowercases each `TextChar`, and the pattern, character by character (`build_flat_text_with`, `byte_pos_to_char_index_ceil`, `char::to_lowercase`), so both sides use the same mapping — a whole-string lowercase would turn a final capital sigma into `ς` on one side only — and byte positions map to char indices through the lowercased text's own table. A case-insensitive match reports whole glyphs: its end is rounded up when it falls inside a `TextChar` of several characters (`byte_pos_to_char_index_ceil`; `OFF` in `o|ffi|ce` gives `offi`), and its matched text is those original characters. Pinned by `a_non_ascii_pattern_is_found_at_every_occurrence` and `case_insensitive_matches_map_to_the_original_characters` (measured 2026-10-30, #416).
- **Regex** (`SimpleRegex`, instead of a regex crate): literals, `.`, `\d \w \s \D \W \S`, `[…]` and `[^…]`, `* + ? {n} {n,} {n,m}`, top-level `|`, and a leading `(?i)`. Everything else is a literal: `(ab)` matches the text `(ab)` and `^ab` matches nothing at a line start (measured 2026-10-30). Only a trailing `\` or a malformed `{…}` fails to compile: `search_regex` returns `Err` for those, `search_page` returns no matches. Matching backtracks per start position.
- **Quad** (`compute_quad`): the box over the matched characters from their minimum baseline to baseline + font size, and from the first x to the furthest `x + width`. It has no descent, and `width` is in text space (#318), so a match in text sized through `Tm` gets a box much narrower than the glyphs.
- `SearchOptions::max_results` caps results per page (0 = unlimited).
- **Bounds on input**: the pattern comes from the caller and the text from the page; neither is bounded. Regex search rebuilds the page's character vector on every match attempt (`find_at`), so its time grows with the square of the number of matches (measured 2026-10-30, release: pattern `a` over 5,000 / 10,000 / 20,000 `a`s took 0.12 / 0.45 / 1.84 s, exact search under 2 ms).

## Code
- `justpdf-core/src/text/search.rs` — `TextQuad`, `SearchResult`, `SearchOptions`, `search_page`, `search_page_exact`, `search_page_regex`, `build_flat_text`, `build_flat_text_with`, `byte_pos_to_char_index`, `byte_pos_to_char_index_ceil`, `compute_quad`, `match_regex`, `SimpleRegex`, `SimpleRegex::compile`, `SimpleRegex::find_at`, `split_alternatives`, `compile_sequence`, `parse_char_class`, `parse_quantifier_braces`, `search_exact`, `search_case_insensitive`, `search_regex`
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
- Exact search rounds both ends of a match down to a char index, so a match ending inside a `TextChar` of several characters (a ligature extracted as `ffi`) is cut short in `char_end` and the quad while `matched_text` is the exact substring, and one starting and ending inside the same glyph is dropped (inferred, 2026-10-30; case-insensitive search rounds the end up instead). Tracked: #435
- The regex path's `(?i)` folds each character to the first character of its lowercase (`c.to_lowercase().next()` in `SimpleRegex`), while plain case-insensitive search uses the whole lowercase: `İ` matches `i` there and not here (inferred, 2026-10-30).
- Unsupported regex syntax (groups, anchors) is matched literally instead of being rejected (measured, above).
- Regex search is quadratic in the number of matches (measured, above).
- Queries containing spaces miss text laid out without space glyphs (measured, above); depends on #298 and #318 for how extraction produces spaces and widths.
- Open question for the maintainer: keep the hand-written `SimpleRegex`, or take a regex crate dependency?
