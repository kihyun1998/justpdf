//! Text layout engine for PDF text writing.
//!
//! Provides word wrapping, alignment, and vertical layout for rendering text
//! into a bounded area.

use crate::font::FontInfo;

/// Text alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextAlignment {
    Left,
    Center,
    Right,
    Justify,
}

/// A laid-out line of text.
#[derive(Debug, Clone)]
pub struct LayoutLine {
    /// The text content of this line.
    pub text: String,
    /// X offset from the left edge of the bounding box.
    pub x_offset: f64,
    /// Y offset from the top of the bounding box (positive downward in layout coords).
    pub y_offset: f64,
    /// Width of the text in this line (in points).
    pub width: f64,
    /// Extra word spacing for justified text.
    pub word_spacing: f64,
}

/// Result of text layout.
#[derive(Debug, Clone)]
pub struct LayoutResult {
    /// Laid-out lines.
    pub lines: Vec<LayoutLine>,
    /// Total height of the laid-out text.
    pub total_height: f64,
    /// Whether all text fit in the bounding box.
    pub overflow: bool,
}

/// Options for text layout.
#[derive(Debug, Clone)]
pub struct LayoutOptions {
    /// Font size in points.
    pub font_size: f64,
    /// Line height as a multiple of font size (default 1.2).
    pub line_height_factor: f64,
    /// Text alignment.
    pub alignment: TextAlignment,
    /// Maximum width for text wrapping (in points). None = no wrapping.
    pub max_width: Option<f64>,
    /// Maximum height (in points). None = unlimited.
    pub max_height: Option<f64>,
    /// First line indent (in points).
    pub first_line_indent: f64,
}

impl Default for LayoutOptions {
    fn default() -> Self {
        Self {
            font_size: 12.0,
            line_height_factor: 1.2,
            alignment: TextAlignment::Left,
            max_width: None,
            max_height: None,
            first_line_indent: 0.0,
        }
    }
}

/// Calculate the width of a single character in font units (1/1000 of text space).
pub fn char_width(ch: char, font: &FontInfo) -> f64 {
    let code = ch as u32;
    font.widths.get_width(code)
}

/// Calculate the width of a string in points for a given font and size.
pub fn measure_text_width(text: &str, font: &FontInfo, font_size: f64) -> f64 {
    let total_units: f64 = text.chars().map(|ch| char_width(ch, font)).sum();
    total_units * font_size / 1000.0
}

/// Lay out text within a bounding box.
///
/// Performs word wrapping, alignment, and vertical stacking of text lines
/// according to the given options.
pub fn layout_text(text: &str, font: &FontInfo, options: &LayoutOptions) -> LayoutResult {
    let line_height = options.font_size * options.line_height_factor;
    let max_width = options.max_width;

    // Split input into paragraphs on explicit newlines.
    let paragraphs: Vec<&str> = text.split('\n').collect();

    let mut raw_lines: Vec<(String, bool)> = Vec::new(); // (text, is_last_line_of_paragraph)

    for (para_idx, para) in paragraphs.iter().enumerate() {
        let trimmed = *para;
        if trimmed.is_empty() {
            // Preserve empty lines from explicit newlines.
            raw_lines.push((String::new(), true));
            continue;
        }

        let indent = if para_idx == 0 {
            options.first_line_indent
        } else {
            // Only the very first line of the entire text gets the indent.
            0.0
        };

        let wrapped = wrap_paragraph(trimmed, font, options.font_size, max_width, indent);
        let count = wrapped.len();
        for (i, line_text) in wrapped.into_iter().enumerate() {
            let is_last = i == count - 1;
            raw_lines.push((line_text, is_last));
        }
    }

    // Now apply alignment and vertical positioning.
    let mut lines = Vec::new();
    let mut y_offset = 0.0;
    let mut overflow = false;

    for (i, (line_text, is_last_of_para)) in raw_lines.into_iter().enumerate() {
        // Check overflow before adding this line.
        if let Some(max_h) = options.max_height {
            if y_offset + line_height > max_h + 1e-9 {
                overflow = true;
                break;
            }
        }

        let line_width = measure_text_width(&line_text, font, options.font_size);

        let effective_max = max_width.unwrap_or(line_width);

        let (x_offset, word_spacing) = match options.alignment {
            TextAlignment::Left => {
                let indent = if i == 0 { options.first_line_indent } else { 0.0 };
                (indent, 0.0)
            }
            TextAlignment::Center => {
                let offset = (effective_max - line_width) / 2.0;
                (offset.max(0.0), 0.0)
            }
            TextAlignment::Right => {
                let offset = effective_max - line_width;
                (offset.max(0.0), 0.0)
            }
            TextAlignment::Justify => {
                let indent = if i == 0 { options.first_line_indent } else { 0.0 };
                if is_last_of_para || max_width.is_none() {
                    // Last line of paragraph: left-align.
                    (indent, 0.0)
                } else {
                    let word_count = line_text.split_whitespace().count();
                    let gap_count = if word_count > 1 { word_count - 1 } else { 0 };
                    if gap_count > 0 {
                        let extra = effective_max - line_width - indent;
                        let ws = if extra > 0.0 {
                            extra / gap_count as f64
                        } else {
                            0.0
                        };
                        (indent, ws)
                    } else {
                        (indent, 0.0)
                    }
                }
            }
        };

        lines.push(LayoutLine {
            text: line_text,
            x_offset,
            y_offset,
            width: line_width,
            word_spacing,
        });

        y_offset += line_height;
    }

    let total_height = if lines.is_empty() {
        0.0
    } else {
        y_offset
    };

    LayoutResult {
        lines,
        total_height,
        overflow,
    }
}

/// Wrap a single paragraph into lines that fit within `max_width`.
fn wrap_paragraph(
    text: &str,
    font: &FontInfo,
    font_size: f64,
    max_width: Option<f64>,
    first_line_indent: f64,
) -> Vec<String> {
    let max_w = match max_width {
        Some(w) => w,
        None => return vec![text.to_string()],
    };

    let words: Vec<&str> = text.split_whitespace().collect();
    if words.is_empty() {
        return vec![String::new()];
    }

    let space_width = measure_text_width(" ", font, font_size);

    let mut lines: Vec<String> = Vec::new();
    let mut current_line = String::new();
    let mut current_width: f64 = 0.0;
    let mut is_first_line = true;

    for word in &words {
        let word_width = measure_text_width(word, font, font_size);
        let indent = if is_first_line { first_line_indent } else { 0.0 };
        let available = max_w - indent;

        if current_line.is_empty() {
            // First word on this line.
            if word_width > available {
                // Word itself is wider than available space: force-break at character level.
                let broken = force_break_word(word, font, font_size, available);
                for (j, part) in broken.into_iter().enumerate() {
                    if j > 0 || !current_line.is_empty() {
                        if !current_line.is_empty() {
                            lines.push(current_line);
                        }
                        is_first_line = false;
                    }
                    current_line = part.clone();
                    current_width = measure_text_width(&part, font, font_size);
                }
            } else {
                current_line = word.to_string();
                current_width = word_width;
            }
        } else {
            // Not the first word: check if adding a space + this word fits.
            let new_width = current_width + space_width + word_width;
            if new_width <= available + 1e-9 {
                current_line.push(' ');
                current_line.push_str(word);
                current_width = new_width;
            } else {
                // Doesn't fit: push current line, start new one.
                lines.push(current_line);
                is_first_line = false;

                let new_indent = 0.0; // indent only on first line
                let new_available = max_w - new_indent;

                if word_width > new_available {
                    let broken = force_break_word(word, font, font_size, new_available);
                    current_line = String::new();
                    current_width = 0.0;
                    for (j, part) in broken.into_iter().enumerate() {
                        if j > 0 && !current_line.is_empty() {
                            lines.push(current_line);
                        }
                        current_line = part.clone();
                        current_width = measure_text_width(&part, font, font_size);
                    }
                } else {
                    current_line = word.to_string();
                    current_width = word_width;
                }
            }
        }
    }

    if !current_line.is_empty() {
        lines.push(current_line);
    }

    if lines.is_empty() {
        lines.push(String::new());
    }

    lines
}

/// Force-break a single word into chunks that each fit within `max_width`.
fn force_break_word(
    word: &str,
    font: &FontInfo,
    font_size: f64,
    max_width: f64,
) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut current_width = 0.0;

    for ch in word.chars() {
        let ch_width = char_width(ch, font) * font_size / 1000.0;
        if current_width + ch_width > max_width + 1e-9 && !current.is_empty() {
            parts.push(current);
            current = String::new();
            current_width = 0.0;
        }
        current.push(ch);
        current_width += ch_width;
    }

    if !current.is_empty() {
        parts.push(current);
    }

    if parts.is_empty() {
        parts.push(String::new());
    }

    parts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font::{Encoding, FontWidths};

    /// Create a simple test font with monospaced 600-unit widths.
    fn test_font() -> FontInfo {
        // Monospaced: every character is 600 units wide.
        // At font_size 10, each char = 600 * 10 / 1000 = 6.0 points.
        FontInfo {
            base_font: b"TestFont".to_vec(),
            subtype: b"Type1".to_vec(),
            encoding: Encoding::StandardEncoding,
            widths: FontWidths::None { default_width: 600.0 },
            to_unicode: None,
            is_standard14: false,
            descriptor: None,
        }
    }

    /// Create a font with varying widths for more realistic tests.
    fn variable_width_font() -> FontInfo {
        // Simple font: chars 32..127 with varying widths.
        let mut widths = vec![0.0; 128];
        widths[32] = 250.0; // space
        for i in 33..127u32 {
            widths[i as usize] = 500.0; // all visible chars = 500
        }
        // Make some chars wider/narrower for realism
        widths[b'i' as usize] = 250.0;
        widths[b'l' as usize] = 250.0;
        widths[b'm' as usize] = 750.0;
        widths[b'w' as usize] = 750.0;
        widths[b'W' as usize] = 750.0;

        FontInfo {
            base_font: b"VarFont".to_vec(),
            subtype: b"Type1".to_vec(),
            encoding: Encoding::StandardEncoding,
            widths: FontWidths::Simple {
                first_char: 0,
                widths,
                default_width: 500.0,
            },
            to_unicode: None,
            is_standard14: false,
            descriptor: None,
        }
    }

    #[test]
    fn test_measure_text_width() {
        let font = test_font();
        // Each char is 600 units. At font_size 10: 600 * 10 / 1000 = 6.0 per char.
        let w = measure_text_width("Hello", &font, 10.0);
        assert!((w - 30.0).abs() < 0.01, "expected 30.0, got {w}");
    }

    #[test]
    fn test_measure_text_width_empty() {
        let font = test_font();
        let w = measure_text_width("", &font, 12.0);
        assert!((w - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_measure_text_width_variable() {
        let font = variable_width_font();
        // "i" = 250, at font_size 10: 250 * 10 / 1000 = 2.5
        let w = measure_text_width("i", &font, 10.0);
        assert!((w - 2.5).abs() < 0.01, "expected 2.5, got {w}");
    }

    #[test]
    fn test_simple_single_line() {
        let font = test_font();
        let options = LayoutOptions {
            font_size: 10.0,
            ..Default::default()
        };
        let result = layout_text("Hello", &font, &options);
        assert_eq!(result.lines.len(), 1);
        assert_eq!(result.lines[0].text, "Hello");
        assert!(!result.overflow);
        assert!((result.lines[0].width - 30.0).abs() < 0.01);
    }

    #[test]
    fn test_word_wrapping() {
        let font = test_font();
        // Each char = 6pt at size 10. "Hello World" = 11 chars * 6 = 66pt.
        // With max_width = 40, "Hello" (30pt) fits, "World" needs a new line.
        let options = LayoutOptions {
            font_size: 10.0,
            max_width: Some(40.0),
            ..Default::default()
        };
        let result = layout_text("Hello World", &font, &options);
        assert_eq!(result.lines.len(), 2);
        assert_eq!(result.lines[0].text, "Hello");
        assert_eq!(result.lines[1].text, "World");
    }

    #[test]
    fn test_explicit_line_break() {
        let font = test_font();
        let options = LayoutOptions {
            font_size: 10.0,
            ..Default::default()
        };
        let result = layout_text("Line one\nLine two", &font, &options);
        assert_eq!(result.lines.len(), 2);
        assert_eq!(result.lines[0].text, "Line one");
        assert_eq!(result.lines[1].text, "Line two");
    }

    #[test]
    fn test_center_alignment() {
        let font = test_font();
        // "Hi" = 2 chars * 6pt = 12pt. max_width = 100. x_offset = (100 - 12) / 2 = 44.
        let options = LayoutOptions {
            font_size: 10.0,
            alignment: TextAlignment::Center,
            max_width: Some(100.0),
            ..Default::default()
        };
        let result = layout_text("Hi", &font, &options);
        assert_eq!(result.lines.len(), 1);
        assert!((result.lines[0].x_offset - 44.0).abs() < 0.01,
            "expected x_offset ~44.0, got {}", result.lines[0].x_offset);
    }

    #[test]
    fn test_right_alignment() {
        let font = test_font();
        // "Hi" = 12pt. max_width = 100. x_offset = 100 - 12 = 88.
        let options = LayoutOptions {
            font_size: 10.0,
            alignment: TextAlignment::Right,
            max_width: Some(100.0),
            ..Default::default()
        };
        let result = layout_text("Hi", &font, &options);
        assert_eq!(result.lines.len(), 1);
        assert!((result.lines[0].x_offset - 88.0).abs() < 0.01,
            "expected x_offset ~88.0, got {}", result.lines[0].x_offset);
    }

    #[test]
    fn test_justify_alignment() {
        let font = test_font();
        // "A B C" => 5 chars * 6 = 30pt. max_width = 60.
        // 3 words, 2 gaps. Extra space = 60 - 30 = 30. word_spacing = 30 / 2 = 15.
        // But this is a single-line paragraph (last line), so it should be left-aligned.
        let options = LayoutOptions {
            font_size: 10.0,
            alignment: TextAlignment::Justify,
            max_width: Some(60.0),
            ..Default::default()
        };
        let result = layout_text("A B C", &font, &options);
        assert_eq!(result.lines.len(), 1);
        // Last line of paragraph: no justification.
        assert!((result.lines[0].word_spacing - 0.0).abs() < 0.01);

        // Two-line case: "AA BB CC DD" with tight max_width to force wrapping.
        // Each word is 2 chars = 12pt. Space = 6pt.
        // "AA BB" = 12 + 6 + 12 = 30pt. max_width = 35 should keep "AA BB" on line 1.
        // "CC DD" on line 2 (last line, no justify).
        let result2 = layout_text("AA BB CC DD", &font, &LayoutOptions {
            font_size: 10.0,
            alignment: TextAlignment::Justify,
            max_width: Some(35.0),
            ..Default::default()
        });
        assert!(result2.lines.len() >= 2);
        // First line "AA BB" = 30pt. Extra = 35 - 30 = 5. 1 gap. word_spacing = 5.
        if result2.lines[0].text == "AA BB" {
            assert!((result2.lines[0].word_spacing - 5.0).abs() < 0.5,
                "expected word_spacing ~5.0, got {}", result2.lines[0].word_spacing);
        }
    }

    #[test]
    fn test_first_line_indent() {
        let font = test_font();
        let options = LayoutOptions {
            font_size: 10.0,
            alignment: TextAlignment::Left,
            first_line_indent: 20.0,
            ..Default::default()
        };
        let result = layout_text("Hello World", &font, &options);
        assert_eq!(result.lines.len(), 1);
        assert!((result.lines[0].x_offset - 20.0).abs() < 0.01);
    }

    #[test]
    fn test_first_line_indent_with_wrapping() {
        let font = test_font();
        // "Hello" = 30pt, indent = 20pt, so total = 50pt.
        // max_width = 55: "Hello" fits on first line with indent.
        // "World" = 30pt, no indent on second line.
        let options = LayoutOptions {
            font_size: 10.0,
            alignment: TextAlignment::Left,
            max_width: Some(55.0),
            first_line_indent: 20.0,
            ..Default::default()
        };
        let result = layout_text("Hello World", &font, &options);
        assert_eq!(result.lines.len(), 2);
        assert!((result.lines[0].x_offset - 20.0).abs() < 0.01);
        assert!((result.lines[1].x_offset - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_overflow_detection() {
        let font = test_font();
        // line_height = 10 * 1.2 = 12pt. max_height = 20.
        // Can fit 1 line (y=0..12), but not 2 (y=0..24).
        let options = LayoutOptions {
            font_size: 10.0,
            max_height: Some(20.0),
            ..Default::default()
        };
        let result = layout_text("Line1\nLine2\nLine3", &font, &options);
        assert!(result.overflow);
        // Should have only 1 line that fits (12 <= 20), second would go to 24 > 20.
        assert!(result.lines.len() < 3);
    }

    #[test]
    fn test_empty_text() {
        let font = test_font();
        let options = LayoutOptions::default();
        let result = layout_text("", &font, &options);
        // Empty text produces one empty line from the paragraph split.
        assert_eq!(result.lines.len(), 1);
        assert_eq!(result.lines[0].text, "");
        assert!((result.lines[0].width - 0.0).abs() < 0.01);
        assert!(!result.overflow);
    }

    #[test]
    fn test_force_break_wide_word() {
        let font = test_font();
        // "ABCDEFGHIJ" = 10 chars * 6pt = 60pt. max_width = 20pt.
        // Should break into: "ABC" (18pt), "DEF" (18pt), "GHI" (18pt), "J" (6pt).
        let options = LayoutOptions {
            font_size: 10.0,
            max_width: Some(20.0),
            ..Default::default()
        };
        let result = layout_text("ABCDEFGHIJ", &font, &options);
        assert!(result.lines.len() >= 3, "expected >=3 lines, got {}", result.lines.len());
        // Verify all text is present.
        let all_text: String = result.lines.iter().map(|l| l.text.as_str()).collect();
        assert_eq!(all_text, "ABCDEFGHIJ");
    }

    #[test]
    fn test_vertical_layout() {
        let font = test_font();
        let options = LayoutOptions {
            font_size: 10.0,
            line_height_factor: 1.5,
            ..Default::default()
        };
        let result = layout_text("A\nB\nC", &font, &options);
        assert_eq!(result.lines.len(), 3);
        // line_height = 10 * 1.5 = 15
        assert!((result.lines[0].y_offset - 0.0).abs() < 0.01);
        assert!((result.lines[1].y_offset - 15.0).abs() < 0.01);
        assert!((result.lines[2].y_offset - 30.0).abs() < 0.01);
        assert!((result.total_height - 45.0).abs() < 0.01);
    }

    #[test]
    fn test_default_options() {
        let opts = LayoutOptions::default();
        assert!((opts.font_size - 12.0).abs() < 0.01);
        assert!((opts.line_height_factor - 1.2).abs() < 0.01);
        assert_eq!(opts.alignment, TextAlignment::Left);
        assert!(opts.max_width.is_none());
        assert!(opts.max_height.is_none());
        assert!((opts.first_line_indent - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_multiple_spaces_collapsed() {
        let font = test_font();
        // Word wrapping uses split_whitespace which collapses multiple spaces.
        let options = LayoutOptions {
            font_size: 10.0,
            max_width: Some(500.0),
            ..Default::default()
        };
        let result = layout_text("Hello    World", &font, &options);
        assert_eq!(result.lines.len(), 1);
        assert_eq!(result.lines[0].text, "Hello World");
    }

    #[test]
    fn test_no_overflow_when_fits() {
        let font = test_font();
        let options = LayoutOptions {
            font_size: 10.0,
            max_height: Some(100.0),
            ..Default::default()
        };
        let result = layout_text("Short", &font, &options);
        assert!(!result.overflow);
    }

    #[test]
    fn test_char_width_function() {
        let font = test_font();
        let w = char_width('A', &font);
        assert!((w - 600.0).abs() < 0.01);
    }

    #[test]
    fn test_char_width_variable_font() {
        let font = variable_width_font();
        assert!((char_width('i', &font) - 250.0).abs() < 0.01);
        assert!((char_width('m', &font) - 750.0).abs() < 0.01);
        assert!((char_width(' ', &font) - 250.0).abs() < 0.01);
    }
}
