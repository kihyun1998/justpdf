//! Text search: exact string, case-insensitive, and regex with quad coordinates.

use super::{PageText, TextChar};

/// A rectangle in user space (quad coordinates for a search hit).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextQuad {
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
}

/// A search result: matched text with page location.
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// 0-based page index.
    pub page_index: usize,
    /// The matched text.
    pub matched_text: String,
    /// Bounding quad for the match.
    pub quad: TextQuad,
    /// Start index into the page's char array.
    pub char_start: usize,
    /// End index (exclusive) into the page's char array.
    pub char_end: usize,
}

/// Search options.
#[derive(Debug, Clone)]
#[derive(Default)]
pub struct SearchOptions {
    /// Case-insensitive search.
    pub case_insensitive: bool,
    /// Use regex pattern.
    pub regex: bool,
    /// Maximum number of results (0 = unlimited).
    pub max_results: usize,
}


/// Search for a text pattern in a single page's extracted text.
pub fn search_page(
    page_text: &PageText,
    pattern: &str,
    options: &SearchOptions,
) -> Vec<SearchResult> {
    if pattern.is_empty() || page_text.chars.is_empty() {
        return Vec::new();
    }

    if options.regex {
        search_page_regex(page_text, pattern, options)
    } else {
        search_page_exact(page_text, pattern, options)
    }
}

/// Search for exact string matches.
fn search_page_exact(
    page_text: &PageText,
    pattern: &str,
    options: &SearchOptions,
) -> Vec<SearchResult> {
    let mut results = Vec::new();

    // Build a flat string from chars with index mapping
    let (flat_text, char_indices) = build_flat_text(&page_text.chars);

    let search_text;
    let search_pattern;

    if options.case_insensitive {
        search_text = flat_text.to_lowercase();
        search_pattern = pattern.to_lowercase();
    } else {
        search_text = flat_text.clone();
        search_pattern = pattern.to_string();
    };

    let mut search_start = 0;
    while let Some(byte_pos) = search_text[search_start..].find(&search_pattern) {
        let abs_byte_pos = search_start + byte_pos;
        let match_end_byte = abs_byte_pos + search_pattern.len();

        // Map byte positions to char indices
        let char_start = byte_pos_to_char_index(&char_indices, abs_byte_pos);
        let char_end = byte_pos_to_char_index(&char_indices, match_end_byte);

        if char_start < page_text.chars.len() && char_end <= page_text.chars.len() && char_start < char_end {
            let matched = &flat_text[abs_byte_pos..match_end_byte];
            let quad = compute_quad(&page_text.chars[char_start..char_end]);

            results.push(SearchResult {
                page_index: page_text.page_index,
                matched_text: matched.to_string(),
                quad,
                char_start,
                char_end,
            });

            if options.max_results > 0 && results.len() >= options.max_results {
                break;
            }
        }

        search_start = abs_byte_pos + 1;
        if search_start >= search_text.len() {
            break;
        }
    }

    results
}

/// Search using regex.
fn search_page_regex(
    page_text: &PageText,
    pattern: &str,
    options: &SearchOptions,
) -> Vec<SearchResult> {
    // Build regex pattern with optional case-insensitive flag
    let regex_pattern = if options.case_insensitive {
        format!("(?i){}", pattern)
    } else {
        pattern.to_string()
    };

    // Simple regex implementation using basic pattern matching
    // For a full implementation, we'd use the `regex` crate.
    // Here we implement a subset: literal, ., *, +, ?, \d, \w, \s, character classes [...], and alternation |
    
    match_regex(&regex_pattern, page_text, options.max_results)
}

/// Build a flat text string from chars, tracking byte→char_index mapping.
fn build_flat_text(chars: &[TextChar]) -> (String, Vec<(usize, usize)>) {
    let mut text = String::new();
    // (byte_start, char_index) pairs
    let mut indices: Vec<(usize, usize)> = Vec::new();

    for (i, ch) in chars.iter().enumerate() {
        let byte_start = text.len();
        indices.push((byte_start, i));
        text.push_str(&ch.unicode);
    }

    // Add sentinel for end
    indices.push((text.len(), chars.len()));

    (text, indices)
}

fn byte_pos_to_char_index(indices: &[(usize, usize)], byte_pos: usize) -> usize {
    // Binary search for the char index at or before this byte position
    match indices.binary_search_by_key(&byte_pos, |&(bp, _)| bp) {
        Ok(i) => indices[i].1,
        Err(i) => {
            if i > 0 {
                indices[i - 1].1
            } else {
                0
            }
        }
    }
}

/// Compute bounding quad for a slice of chars.
fn compute_quad(chars: &[TextChar]) -> TextQuad {
    if chars.is_empty() {
        return TextQuad {
            x0: 0.0,
            y0: 0.0,
            x1: 0.0,
            y1: 0.0,
        };
    }

    let x0 = chars
        .iter()
        .map(|c| c.x)
        .fold(f64::MAX, f64::min);
    let y0 = chars
        .iter()
        .map(|c| c.y)
        .fold(f64::MAX, f64::min);
    let x1 = chars
        .iter()
        .map(|c| c.x + c.width)
        .fold(f64::MIN, f64::max);
    let y1 = chars
        .iter()
        .map(|c| c.y + c.font_size)
        .fold(f64::MIN, f64::max);

    TextQuad { x0, y0, x1, y1 }
}

/// Simple regex matching without the regex crate.
/// Supports basic patterns using Rust's built-in str::contains and manual matching.
fn match_regex(
    pattern: &str,
    page_text: &PageText,
    max_results: usize,
) -> Vec<SearchResult> {
    // We'll implement a basic NFA-style regex matcher for common patterns
    // For now, convert common regex patterns to string searches where possible

    let (flat_text, char_indices) = build_flat_text(&page_text.chars);

    // Try to compile as a simple pattern
    let compiled = match SimpleRegex::compile(pattern) {
        Some(r) => r,
        None => return Vec::new(), // Unsupported pattern
    };

    let mut results = Vec::new();
    let mut search_start = 0;

    while search_start < flat_text.len() {
        if let Some((match_start, match_end)) = compiled.find_at(&flat_text, search_start) {
            if match_start >= match_end {
                search_start = match_start + 1;
                continue;
            }

            let char_start = byte_pos_to_char_index(&char_indices, match_start);
            let char_end = byte_pos_to_char_index(&char_indices, match_end);

            if char_start < page_text.chars.len() && char_end <= page_text.chars.len() && char_start < char_end {
                let matched = &flat_text[match_start..match_end];
                let quad = compute_quad(&page_text.chars[char_start..char_end]);

                results.push(SearchResult {
                    page_index: page_text.page_index,
                    matched_text: matched.to_string(),
                    quad,
                    char_start,
                    char_end,
                });

                if max_results > 0 && results.len() >= max_results {
                    break;
                }
            }

            search_start = match_start + 1;
        } else {
            break;
        }
    }

    results
}

// ---------------------------------------------------------------------------
// Simple regex engine (subset of regex syntax)
// ---------------------------------------------------------------------------

#[derive(Debug)]
enum RegexNode {
    Literal(char),
    AnyChar,                    // .
    Digit,                      // \d
    Word,                       // \w
    Whitespace,                 // \s
    CharClass(Vec<CharRange>),  // [...]
    NegCharClass(Vec<CharRange>), // [^...]
}

#[derive(Debug)]
enum Quantifier {
    One,
    ZeroOrMore,  // *
    OneOrMore,   // +
    ZeroOrOne,   // ?
    Exact(usize),      // {n}
    Range(usize, Option<usize>), // {n,m}
}

#[derive(Debug)]
struct RegexPart {
    node: RegexNode,
    quantifier: Quantifier,
}

#[derive(Debug, Clone)]
struct CharRange {
    start: char,
    end: char,
}

impl CharRange {
    fn contains(&self, c: char) -> bool {
        c >= self.start && c <= self.end
    }
}

#[derive(Debug)]
struct SimpleRegex {
    /// Alternatives separated by |
    alternatives: Vec<Vec<RegexPart>>,
    case_insensitive: bool,
}

impl SimpleRegex {
    fn compile(pattern: &str) -> Option<Self> {
        let mut pat = pattern;
        let mut case_insensitive = false;

        // Handle (?i) prefix
        if pat.starts_with("(?i)") {
            case_insensitive = true;
            pat = &pat[4..];
        }

        // Split by unescaped |
        let alternatives = split_alternatives(pat);
        let mut compiled_alts = Vec::new();

        for alt in alternatives {
            let parts = compile_sequence(&alt)?;
            compiled_alts.push(parts);
        }

        Some(SimpleRegex {
            alternatives: compiled_alts,
            case_insensitive,
        })
    }

    /// Find the first match at or after `start` in `text`.
    fn find_at(&self, text: &str, start: usize) -> Option<(usize, usize)> {
        let text_chars: Vec<char> = text.chars().collect();
        let byte_offsets: Vec<usize> = text.char_indices().map(|(i, _)| i).collect();

        // Map byte start to char index
        let char_start = match byte_offsets.binary_search(&start) {
            Ok(i) => i,
            Err(i) => i,
        };

        for ci in char_start..text_chars.len() {
            for alt in &self.alternatives {
                if let Some(match_len) = self.try_match(alt, &text_chars, ci) {
                    let byte_start = byte_offsets[ci];
                    let byte_end = if ci + match_len < byte_offsets.len() {
                        byte_offsets[ci + match_len]
                    } else {
                        text.len()
                    };
                    return Some((byte_start, byte_end));
                }
            }
        }

        None
    }

    fn try_match(&self, parts: &[RegexPart], text: &[char], start: usize) -> Option<usize> {
        let end_pos = self.try_match_parts(parts, 0, text, start)?;
        Some(end_pos - start)
    }

    /// Returns Some(end_position) if the remaining parts match starting at `pos`.
    fn try_match_parts(
        &self,
        parts: &[RegexPart],
        part_idx: usize,
        text: &[char],
        pos: usize,
    ) -> Option<usize> {
        if part_idx >= parts.len() {
            return Some(pos);
        }

        let part = &parts[part_idx];

        match &part.quantifier {
            Quantifier::One => {
                if self.matches_node(&part.node, text, pos) {
                    self.try_match_parts(parts, part_idx + 1, text, pos + 1)
                } else {
                    None
                }
            }
            Quantifier::ZeroOrMore => {
                let mut count = 0;
                while pos + count < text.len() && self.matches_node(&part.node, text, pos + count) {
                    count += 1;
                }
                for c in (0..=count).rev() {
                    if let Some(end) = self.try_match_parts(parts, part_idx + 1, text, pos + c) {
                        return Some(end);
                    }
                }
                None
            }
            Quantifier::OneOrMore => {
                if !self.matches_node(&part.node, text, pos) {
                    return None;
                }
                let mut count = 1;
                while pos + count < text.len() && self.matches_node(&part.node, text, pos + count) {
                    count += 1;
                }
                for c in (1..=count).rev() {
                    if let Some(end) = self.try_match_parts(parts, part_idx + 1, text, pos + c) {
                        return Some(end);
                    }
                }
                None
            }
            Quantifier::ZeroOrOne => {
                if self.matches_node(&part.node, text, pos)
                    && let Some(end) = self.try_match_parts(parts, part_idx + 1, text, pos + 1) {
                        return Some(end);
                    }
                self.try_match_parts(parts, part_idx + 1, text, pos)
            }
            Quantifier::Exact(n) => {
                let n = *n;
                for i in 0..n {
                    if !self.matches_node(&part.node, text, pos + i) {
                        return None;
                    }
                }
                self.try_match_parts(parts, part_idx + 1, text, pos + n)
            }
            Quantifier::Range(min, max) => {
                let min = *min;
                let max = *max;
                for i in 0..min {
                    if !self.matches_node(&part.node, text, pos + i) {
                        return None;
                    }
                }
                let actual_max = max.unwrap_or(text.len() - pos);
                let mut count = min;
                while count < actual_max
                    && pos + count < text.len()
                    && self.matches_node(&part.node, text, pos + count)
                {
                    count += 1;
                }
                for c in (min..=count).rev() {
                    if let Some(end) = self.try_match_parts(parts, part_idx + 1, text, pos + c) {
                        return Some(end);
                    }
                }
                None
            }
        }
    }

    fn matches_node(&self, node: &RegexNode, text: &[char], pos: usize) -> bool {
        if pos >= text.len() {
            return false;
        }
        let c = text[pos];
        let c_lower = if self.case_insensitive {
            c.to_lowercase().next().unwrap_or(c)
        } else {
            c
        };

        match node {
            RegexNode::Literal(lit) => {
                if self.case_insensitive {
                    c_lower == lit.to_lowercase().next().unwrap_or(*lit)
                } else {
                    c == *lit
                }
            }
            RegexNode::AnyChar => c != '\n',
            RegexNode::Digit => c.is_ascii_digit(),
            RegexNode::Word => c.is_alphanumeric() || c == '_',
            RegexNode::Whitespace => c.is_whitespace(),
            RegexNode::CharClass(ranges) => ranges.iter().any(|r| r.contains(c_lower)),
            RegexNode::NegCharClass(ranges) => !ranges.iter().any(|r| r.contains(c_lower)),
        }
    }
}

fn split_alternatives(pattern: &str) -> Vec<String> {
    let mut alts = Vec::new();
    let mut current = String::new();
    let mut escaped = false;
    let mut bracket_depth = 0;

    for c in pattern.chars() {
        if escaped {
            current.push('\\');
            current.push(c);
            escaped = false;
            continue;
        }
        match c {
            '\\' => escaped = true,
            '[' => {
                bracket_depth += 1;
                current.push(c);
            }
            ']' if bracket_depth > 0 => {
                bracket_depth -= 1;
                current.push(c);
            }
            '|' if bracket_depth == 0 => {
                alts.push(std::mem::take(&mut current));
            }
            _ => current.push(c),
        }
    }
    if escaped {
        current.push('\\');
    }
    alts.push(current);
    alts
}

fn compile_sequence(pattern: &str) -> Option<Vec<RegexPart>> {
    let mut parts = Vec::new();
    let chars: Vec<char> = pattern.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let node = match chars[i] {
            '\\' => {
                i += 1;
                if i >= chars.len() {
                    return None;
                }
                match chars[i] {
                    'd' => RegexNode::Digit,
                    'w' => RegexNode::Word,
                    's' => RegexNode::Whitespace,
                    'D' => RegexNode::NegCharClass(vec![CharRange { start: '0', end: '9' }]),
                    'W' => {
                        // Non-word: we'll approximate
                        RegexNode::NegCharClass(vec![
                            CharRange { start: 'a', end: 'z' },
                            CharRange { start: 'A', end: 'Z' },
                            CharRange { start: '0', end: '9' },
                            CharRange { start: '_', end: '_' },
                        ])
                    }
                    'S' => RegexNode::NegCharClass(vec![
                        CharRange { start: ' ', end: ' ' },
                        CharRange { start: '\t', end: '\t' },
                        CharRange { start: '\n', end: '\n' },
                        CharRange { start: '\r', end: '\r' },
                    ]),
                    c => RegexNode::Literal(c), // Escaped literal
                }
            }
            '.' => RegexNode::AnyChar,
            '[' => {
                i += 1;
                let (node, consumed) = parse_char_class(&chars[i..])?;
                i += consumed;
                node
            }
            c => RegexNode::Literal(c),
        };

        i += 1;

        // Check for quantifier
        let quantifier = if i < chars.len() {
            match chars[i] {
                '*' => {
                    i += 1;
                    Quantifier::ZeroOrMore
                }
                '+' => {
                    i += 1;
                    Quantifier::OneOrMore
                }
                '?' => {
                    i += 1;
                    Quantifier::ZeroOrOne
                }
                '{' => {
                    let (q, consumed) = parse_quantifier_braces(&chars[i..])?;
                    i += consumed;
                    q
                }
                _ => Quantifier::One,
            }
        } else {
            Quantifier::One
        };

        parts.push(RegexPart { node, quantifier });
    }

    Some(parts)
}

fn parse_char_class(chars: &[char]) -> Option<(RegexNode, usize)> {
    let mut i = 0;
    let negated = if i < chars.len() && chars[i] == '^' {
        i += 1;
        true
    } else {
        false
    };

    let mut ranges = Vec::new();

    while i < chars.len() && chars[i] != ']' {
        let start = if chars[i] == '\\' && i + 1 < chars.len() {
            i += 1;
            chars[i]
        } else {
            chars[i]
        };
        i += 1;

        if i + 1 < chars.len() && chars[i] == '-' && chars[i + 1] != ']' {
            i += 1; // skip '-'
            let end = if chars[i] == '\\' && i + 1 < chars.len() {
                i += 1;
                chars[i]
            } else {
                chars[i]
            };
            i += 1;
            ranges.push(CharRange { start, end });
        } else {
            ranges.push(CharRange { start, end: start });
        }
    }

    if i < chars.len() && chars[i] == ']' {
        i += 1; // skip ']'
    }

    let node = if negated {
        RegexNode::NegCharClass(ranges)
    } else {
        RegexNode::CharClass(ranges)
    };

    Some((node, i))
}

fn parse_quantifier_braces(chars: &[char]) -> Option<(Quantifier, usize)> {
    if chars.is_empty() || chars[0] != '{' {
        return None;
    }

    let mut i = 1;
    let mut num1 = String::new();

    while i < chars.len() && chars[i].is_ascii_digit() {
        num1.push(chars[i]);
        i += 1;
    }

    if i >= chars.len() {
        return None;
    }

    if chars[i] == '}' {
        let n: usize = num1.parse().ok()?;
        return Some((Quantifier::Exact(n), i + 1));
    }

    if chars[i] == ',' {
        i += 1;
        let mut num2 = String::new();
        while i < chars.len() && chars[i].is_ascii_digit() {
            num2.push(chars[i]);
            i += 1;
        }
        if i < chars.len() && chars[i] == '}' {
            let min: usize = num1.parse().ok()?;
            let max = if num2.is_empty() {
                None
            } else {
                Some(num2.parse().ok()?)
            };
            return Some((Quantifier::Range(min, max), i + 1));
        }
    }

    None
}

// ---------------------------------------------------------------------------
// Public search functions
// ---------------------------------------------------------------------------

/// Search for exact text across multiple pages.
pub fn search_exact(pages: &[PageText], query: &str) -> Vec<SearchResult> {
    let options = SearchOptions::default();
    let mut results = Vec::new();
    for page in pages {
        results.extend(search_page(page, query, &options));
    }
    results
}

/// Case-insensitive search across multiple pages.
pub fn search_case_insensitive(pages: &[PageText], query: &str) -> Vec<SearchResult> {
    let options = SearchOptions {
        case_insensitive: true,
        ..Default::default()
    };
    let mut results = Vec::new();
    for page in pages {
        results.extend(search_page(page, query, &options));
    }
    results
}

/// Regex search across multiple pages.
pub fn search_regex(pages: &[PageText], pattern: &str) -> std::result::Result<Vec<SearchResult>, String> {
    // Validate pattern by trying to compile
    if SimpleRegex::compile(pattern).is_none() {
        return Err(format!("Invalid regex pattern: {}", pattern));
    }

    let options = SearchOptions {
        regex: true,
        ..Default::default()
    };
    let mut results = Vec::new();
    for page in pages {
        results.extend(search_page(page, pattern, &options));
    }
    Ok(results)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::TextChar;

    fn make_page_text(text: &str, page_index: usize) -> PageText {
        let mut x = 72.0;
        let chars: Vec<TextChar> = text
            .chars()
            .map(|c| {
                let ch = TextChar {
                    unicode: c.to_string(),
                    x,
                    y: 720.0,
                    font_size: 12.0,
                    font_name: "F1".into(),
                    width: 7.0,
                };
                x += 7.0;
                ch
            })
            .collect();

        PageText {
            page_index,
            chars,
            lines: Vec::new(),
            blocks: Vec::new(),
        }
    }

    #[test]
    fn test_exact_search() {
        let page = make_page_text("Hello World Test", 0);
        let results = search_page(&page, "World", &SearchOptions::default());
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].matched_text, "World");
        assert_eq!(results[0].page_index, 0);
    }

    #[test]
    fn test_case_insensitive_search() {
        let page = make_page_text("Hello World Test", 0);
        let options = SearchOptions {
            case_insensitive: true,
            ..Default::default()
        };
        let results = search_page(&page, "hello", &options);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].matched_text, "Hello");
    }

    #[test]
    fn test_multiple_matches() {
        let page = make_page_text("the cat and the dog", 0);
        let results = search_page(&page, "the", &SearchOptions::default());
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_no_match() {
        let page = make_page_text("Hello World", 0);
        let results = search_page(&page, "xyz", &SearchOptions::default());
        assert!(results.is_empty());
    }

    #[test]
    fn test_empty_pattern() {
        let page = make_page_text("Hello", 0);
        let results = search_page(&page, "", &SearchOptions::default());
        assert!(results.is_empty());
    }

    #[test]
    fn test_max_results() {
        let page = make_page_text("aaa", 0);
        let options = SearchOptions {
            max_results: 1,
            ..Default::default()
        };
        let results = search_page(&page, "a", &options);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_regex_digit_pattern() {
        let page = make_page_text("Phone: 123-4567", 0);
        let options = SearchOptions {
            regex: true,
            ..Default::default()
        };
        let results = search_page(&page, "\\d{3}-\\d{4}", &options);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].matched_text, "123-4567");
    }

    #[test]
    fn test_regex_word_pattern() {
        let page = make_page_text("Hello World", 0);
        let options = SearchOptions {
            regex: true,
            ..Default::default()
        };
        let results = search_page(&page, "\\w+", &options);
        assert!(results.len() >= 1);
        assert_eq!(results[0].matched_text, "Hello");
    }

    #[test]
    fn test_regex_dot_star() {
        let page = make_page_text("Hello World", 0);
        let options = SearchOptions {
            regex: true,
            ..Default::default()
        };
        let results = search_page(&page, "H.*d", &options);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].matched_text, "Hello World");
    }

    #[test]
    fn test_regex_alternation() {
        let page = make_page_text("cat and dog", 0);
        let options = SearchOptions {
            regex: true,
            ..Default::default()
        };
        let results = search_page(&page, "cat|dog", &options);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_regex_char_class() {
        let page = make_page_text("a1b2c3", 0);
        let options = SearchOptions {
            regex: true,
            ..Default::default()
        };
        let results = search_page(&page, "[a-c]", &options);
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_quad_coordinates() {
        let page = make_page_text("Hello", 0);
        let results = search_page(&page, "Hello", &SearchOptions::default());
        assert_eq!(results.len(), 1);
        let quad = &results[0].quad;
        assert!(quad.x0 >= 72.0);
        assert!(quad.x1 > quad.x0);
        assert!((quad.y0 - 720.0).abs() < 0.1);
    }

    #[test]
    fn test_search_across_pages() {
        let pages = vec![
            make_page_text("Hello World", 0),
            make_page_text("World Peace", 1),
        ];
        let results = search_exact(&pages, "World");
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].page_index, 0);
        assert_eq!(results[1].page_index, 1);
    }

    #[test]
    fn test_regex_error() {
        // Trailing backslash is invalid
        let result = search_regex(&[], "abc\\");
        assert!(result.is_err());
    }
}
