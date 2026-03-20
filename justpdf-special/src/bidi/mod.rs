//! Bidirectional (BiDi) text support.
//!
//! Provides Unicode Bidirectional Algorithm (UBA) integration for
//! correct rendering of mixed left-to-right and right-to-left text
//! (e.g. Arabic, Hebrew mixed with Latin).

use crate::Result;

/// The resolved direction of a text run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Ltr,
    Rtl,
}

/// A run of text with a resolved direction.
#[derive(Debug, Clone)]
pub struct BidiRun {
    /// The text content of this run.
    pub text: String,
    /// The resolved direction.
    pub direction: Direction,
    /// The BiDi embedding level.
    pub level: u8,
}

/// Resolve bidirectional text into ordered runs.
///
/// Takes a paragraph of text and returns a list of runs in visual order,
/// each with its resolved direction. This is useful for correctly laying
/// out mixed LTR/RTL text in PDF content streams.
pub fn resolve_bidi(text: &str) -> Result<Vec<BidiRun>> {
    use unicode_bidi::BidiInfo;

    let bidi_info = BidiInfo::new(text, None);
    let mut runs = Vec::new();

    for para in &bidi_info.paragraphs {
        let line = para.range.clone();
        let (_levels, level_runs) = bidi_info.visual_runs(para, line);

        for run_range in &level_runs {
            // Get the original level of the first byte in this run
            let level = bidi_info.levels[run_range.start];
            let run_text = &text[run_range.clone()];
            let direction = if level.is_rtl() {
                Direction::Rtl
            } else {
                Direction::Ltr
            };
            runs.push(BidiRun {
                text: run_text.to_string(),
                direction,
                level: level.number(),
            });
        }
    }

    if runs.is_empty() && !text.is_empty() {
        // Fallback: treat entire text as LTR
        runs.push(BidiRun {
            text: text.to_string(),
            direction: Direction::Ltr,
            level: 0,
        });
    }

    Ok(runs)
}

/// Check if a string contains any right-to-left characters.
pub fn contains_rtl(text: &str) -> bool {
    text.chars().any(|c| is_rtl_char(c))
}

/// Check if a character is a right-to-left character.
fn is_rtl_char(c: char) -> bool {
    matches!(c as u32,
        0x0590..=0x05FF   // Hebrew
        | 0x0600..=0x06FF // Arabic
        | 0x0700..=0x074F // Syriac
        | 0x0780..=0x07BF // Thaana
        | 0x07C0..=0x07FF // NKo
        | 0x0800..=0x083F // Samaritan
        | 0x0840..=0x085F // Mandaic
        | 0x08A0..=0x08FF // Arabic Extended-A
        | 0xFB1D..=0xFB4F // Hebrew Presentation Forms
        | 0xFB50..=0xFDFF // Arabic Presentation Forms-A
        | 0xFE70..=0xFEFF // Arabic Presentation Forms-B
        | 0x10800..=0x1083F // Cypriot
        | 0x10900..=0x1091F // Phoenician
        | 0x10920..=0x1093F // Lydian
        | 0x1EE00..=0x1EEFF // Arabic Mathematical Alphabetic Symbols
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ltr_only() {
        let runs = resolve_bidi("Hello World").unwrap();
        assert!(!runs.is_empty());
        assert_eq!(runs[0].direction, Direction::Ltr);
    }

    #[test]
    fn test_contains_rtl_false() {
        assert!(!contains_rtl("Hello World"));
    }

    #[test]
    fn test_contains_rtl_true() {
        assert!(contains_rtl("Hello \u{0627}\u{0644}\u{0639}\u{0631}\u{0628}\u{064A}\u{0629}"));
    }

    #[test]
    fn test_empty_string() {
        let runs = resolve_bidi("").unwrap();
        assert!(runs.is_empty());
    }
}
