pub mod format;
pub mod layout;
pub mod search;
pub mod text_layout;

#[cfg(test)]
use std::collections::HashMap;

#[cfg(test)]
use crate::content::Operand;
pub use crate::content::interpret::Matrix;
use crate::content::interpret::{
    ContentResources, ContentVisitor, DocResources, Glyph, GraphicsState, InterpretOptions,
    interpret,
};
#[cfg(test)]
use crate::content::interpret::{MapResources, ResolvedFont};
use crate::content::{ContentOp, parse_content_stream};
use crate::error::Result;
use crate::font::decode_text;
#[cfg(test)]
use crate::font::{Encoding, FontInfo};
use crate::object::PdfObject;
use crate::page::{PageInfo, collect_pages};
use crate::parser::PdfDocument;

// ---------------------------------------------------------------------------
// Extracted text output types
// ---------------------------------------------------------------------------

/// A single extracted character with its position.
#[derive(Debug, Clone)]
pub struct TextChar {
    /// The Unicode character(s) for this glyph.
    pub unicode: String,
    /// X position in user space (points from page origin).
    pub x: f64,
    /// Y position in user space.
    pub y: f64,
    /// Effective font size in user space.
    pub font_size: f64,
    /// Font name (resource name); empty for a font an ExtGState's `/Font`
    /// set through `gs`.
    pub font_name: String,
    /// Character advance width in user space.
    pub width: f64,
}

/// A word: a sequence of characters not separated by large gaps.
#[derive(Debug, Clone)]
pub struct TextWord {
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub font_size: f64,
}

/// A line of text: a sequence of words on roughly the same baseline.
#[derive(Debug, Clone)]
pub struct TextLine {
    pub text: String,
    pub words: Vec<TextWord>,
    pub x: f64,
    pub y: f64,
}

/// A block of text: a sequence of lines grouped spatially.
#[derive(Debug, Clone)]
pub struct TextBlock {
    pub text: String,
    pub lines: Vec<TextLine>,
}

/// Full text extraction result for a page.
#[derive(Debug, Clone)]
pub struct PageText {
    pub page_index: usize,
    pub chars: Vec<TextChar>,
    pub lines: Vec<TextLine>,
    pub blocks: Vec<TextBlock>,
}

impl PageText {
    /// Get plain text, joining blocks with blank lines.
    pub fn plain_text(&self) -> String {
        self.blocks
            .iter()
            .map(|b| b.text.as_str())
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}

// ---------------------------------------------------------------------------
// Characters from the shared content interpreter
// ---------------------------------------------------------------------------

/// Runs the shared content interpreter and turns its glyphs into characters.
struct TextInterpreter<R: ContentResources> {
    resources: R,
}

#[cfg(test)]
impl TextInterpreter<MapResources> {
    fn new(fonts: HashMap<Vec<u8>, ResolvedFont>) -> Self {
        let fonts = fonts
            .into_iter()
            .map(|(name, font)| (name, std::rc::Rc::new(font)))
            .collect();
        Self {
            resources: MapResources {
                fonts,
                ..MapResources::default()
            },
        }
    }
}

impl<R: ContentResources> TextInterpreter<R> {
    fn run(mut self, ops: &[ContentOp]) -> Vec<TextChar> {
        let mut collector = CharCollector { chars: Vec::new() };
        interpret(
            ops,
            &mut self.resources,
            Matrix::identity(),
            InterpretOptions::default(),
            &mut collector,
        );
        collector.chars
    }
}

struct CharCollector {
    chars: Vec<TextChar>,
}

impl ContentVisitor for CharCollector {
    fn glyph(&mut self, glyph: &Glyph<'_>, state: &GraphicsState) {
        // Unicode: ToUnicode, then the font's encoding.
        let unicode = match glyph.font {
            Some(f) => {
                let decode = || match glyph.bytes {
                    [byte] => f.info.decode_simple_code(*byte),
                    bytes => decode_text(bytes, f.info.encoding),
                };
                match f.cmap.as_ref().and_then(|cmap| cmap.lookup(glyph.code)) {
                    Some(text) => text,
                    None => decode(),
                }
            }
            None => String::from_utf8_lossy(glyph.bytes).into_owned(),
        };
        if unicode.is_empty() {
            return;
        }

        let text = &state.text;
        let (x, y) = glyph.trm.transform_point(0.0, 0.0);
        self.chars.push(TextChar {
            unicode,
            x,
            y,
            font_size: glyph.trm.font_size_scale(),
            font_name: String::from_utf8_lossy(&text.font_name).into_owned(),
            width: (glyph.width / 1000.0 * text.font_size * text.horiz_scaling).abs(),
        });
    }
}

// ---------------------------------------------------------------------------
// Word / line / block grouping
// ---------------------------------------------------------------------------

/// Group extracted characters into words based on spatial gaps.
fn group_into_words(chars: &[TextChar]) -> Vec<TextWord> {
    if chars.is_empty() {
        return Vec::new();
    }

    let mut words: Vec<TextWord> = Vec::new();
    let mut current_text = String::new();
    let mut word_x = chars[0].x;
    let mut word_y = chars[0].y;
    let mut word_end_x = chars[0].x;
    let mut word_font_size = chars[0].font_size;

    for (i, ch) in chars.iter().enumerate() {
        if i > 0 {
            let prev = &chars[i - 1];
            let expected_x = prev.x + prev.width;
            let gap = (ch.x - expected_x).abs();
            let y_diff = (ch.y - prev.y).abs();
            let threshold = prev.font_size * 0.3;

            // Start new word if there's a significant gap or Y change
            if gap > threshold || y_diff > prev.font_size * 0.5 {
                if !current_text.is_empty() {
                    words.push(TextWord {
                        text: current_text.trim().to_string(),
                        x: word_x,
                        y: word_y,
                        width: word_end_x - word_x,
                        font_size: word_font_size,
                    });
                }
                current_text = String::new();
                word_x = ch.x;
                word_y = ch.y;
                word_font_size = ch.font_size;
            }
        }

        // Treat space as word boundary
        if ch.unicode == " " {
            if !current_text.is_empty() {
                words.push(TextWord {
                    text: current_text.trim().to_string(),
                    x: word_x,
                    y: word_y,
                    width: word_end_x - word_x,
                    font_size: word_font_size,
                });
                current_text = String::new();
                word_x = ch.x + ch.width;
                word_y = ch.y;
                word_font_size = ch.font_size;
            }
        } else {
            current_text.push_str(&ch.unicode);
            word_end_x = ch.x + ch.width;
        }
    }

    // Flush last word
    if !current_text.is_empty() {
        words.push(TextWord {
            text: current_text.trim().to_string(),
            x: word_x,
            y: word_y,
            width: word_end_x - word_x,
            font_size: word_font_size,
        });
    }

    // Remove empty words
    words.retain(|w| !w.text.is_empty());
    words
}

/// Group words into lines (same baseline, sorted left to right).
fn group_into_lines(words: &[TextWord]) -> Vec<TextLine> {
    if words.is_empty() {
        return Vec::new();
    }

    // Sort words by Y descending (top to bottom), then X ascending
    let mut sorted: Vec<&TextWord> = words.iter().collect();
    sorted.sort_by(|a, b| {
        let y_cmp = b.y.partial_cmp(&a.y).unwrap_or(std::cmp::Ordering::Equal);
        if y_cmp == std::cmp::Ordering::Equal {
            a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal)
        } else {
            y_cmp
        }
    });

    let mut lines: Vec<TextLine> = Vec::new();
    let mut current_line_words: Vec<TextWord> = Vec::new();
    let mut line_y = sorted[0].y;

    for word in sorted {
        let y_threshold = word.font_size * 0.5;
        if (word.y - line_y).abs() > y_threshold && !current_line_words.is_empty() {
            // Flush current line
            lines.push(build_line(std::mem::take(&mut current_line_words)));
            line_y = word.y;
        }
        current_line_words.push(word.clone());
        if current_line_words.len() == 1 {
            line_y = word.y;
        }
    }

    if !current_line_words.is_empty() {
        lines.push(build_line(current_line_words));
    }

    lines
}

fn build_line(mut words: Vec<TextWord>) -> TextLine {
    // Sort words left to right within the line
    words.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal));

    let text = words
        .iter()
        .map(|w| w.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");

    let x = words.first().map(|w| w.x).unwrap_or(0.0);
    let y = words.first().map(|w| w.y).unwrap_or(0.0);

    TextLine { text, x, y, words }
}

// ---------------------------------------------------------------------------
// Get content stream data for a page
// ---------------------------------------------------------------------------

fn get_page_content_data(doc: &PdfDocument, page: &PageInfo) -> Result<Vec<u8>> {
    let contents_obj = match &page.contents_ref {
        Some(obj) => obj.clone(),
        None => return Ok(Vec::new()),
    };

    match contents_obj {
        PdfObject::Reference(r) => {
            let obj = doc.resolve(&r)?;
            decode_content_obj(doc, &obj)
        }
        PdfObject::Array(arr) => {
            let mut combined = Vec::new();
            for item in &arr {
                let data = match item {
                    PdfObject::Reference(r) => {
                        let r = r.clone();
                        let obj = doc.resolve(&r)?;
                        decode_content_obj(doc, &obj)?
                    }
                    _ => Vec::new(),
                };
                if !combined.is_empty() {
                    combined.push(b' ');
                }
                combined.extend_from_slice(&data);
            }
            Ok(combined)
        }
        PdfObject::Stream { dict, data } => doc.decode_stream(&dict, &data),
        _ => Ok(Vec::new()),
    }
}

fn decode_content_obj(doc: &PdfDocument, obj: &PdfObject) -> Result<Vec<u8>> {
    match obj {
        PdfObject::Stream { dict, data } => doc.decode_stream(dict, data),
        _ => Ok(Vec::new()),
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Extract text from a single page.
pub fn extract_page_text(doc: &PdfDocument, page: &PageInfo) -> Result<PageText> {
    // Get content stream data
    let content_data = get_page_content_data(doc, page)?;

    if content_data.is_empty() {
        return Ok(PageText {
            page_index: page.index,
            chars: Vec::new(),
            lines: Vec::new(),
            blocks: Vec::new(),
        });
    }

    // Parse content stream
    let ops = parse_content_stream(&content_data)?;

    // Run text interpreter
    let resources = DocResources::page(doc, page.resources_ref.as_ref());
    let chars = TextInterpreter { resources }.run(&ops);

    // Group into structure
    let words = group_into_words(&chars);
    let lines = group_into_lines(&words);
    // Use advanced layout: column detection, reading order, dehyphenation
    let blocks = layout::detect_columns_and_reorder(&lines);

    Ok(PageText {
        page_index: page.index,
        chars,
        lines,
        blocks,
    })
}

/// Extract text from all pages of a document.
pub fn extract_all_text(doc: &PdfDocument) -> Result<Vec<PageText>> {
    let pages = collect_pages(doc)?;
    let mut results = Vec::with_capacity(pages.len());

    for page in &pages {
        results.push(extract_page_text(doc, page)?);
    }

    Ok(results)
}

/// Extract plain text from a single page as a string.
pub fn extract_page_text_string(doc: &PdfDocument, page: &PageInfo) -> Result<String> {
    let page_text = extract_page_text(doc, page)?;
    Ok(page_text.plain_text())
}

/// Extract plain text from all pages, joining pages with blank lines.
pub fn extract_all_text_string(doc: &PdfDocument) -> Result<String> {
    let pages = extract_all_text(doc)?;
    let texts: Vec<String> = pages.iter().map(|p| p.plain_text()).collect();
    Ok(texts.join("\n\n"))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_matrix_identity() {
        let m = Matrix::identity();
        let (x, y) = m.transform_point(10.0, 20.0);
        assert!((x - 10.0).abs() < 1e-10);
        assert!((y - 20.0).abs() < 1e-10);
    }

    #[test]
    fn test_matrix_translate() {
        let m = Matrix::translate(100.0, 200.0);
        let (x, y) = m.transform_point(0.0, 0.0);
        assert!((x - 100.0).abs() < 1e-10);
        assert!((y - 200.0).abs() < 1e-10);
    }

    #[test]
    fn test_matrix_concat() {
        let a = Matrix::translate(10.0, 20.0);
        let b = Matrix::translate(30.0, 40.0);
        let c = a.concat(&b);
        let (x, y) = c.transform_point(0.0, 0.0);
        assert!((x - 40.0).abs() < 1e-10);
        assert!((y - 60.0).abs() < 1e-10);
    }

    #[test]
    fn test_matrix_scale() {
        let m = Matrix {
            a: 2.0,
            b: 0.0,
            c: 0.0,
            d: 3.0,
            e: 0.0,
            f: 0.0,
        };
        let (x, y) = m.transform_point(10.0, 10.0);
        assert!((x - 20.0).abs() < 1e-10);
        assert!((y - 30.0).abs() < 1e-10);
    }

    #[test]
    fn test_interpreter_basic_text() {
        // Simulate: BT /F1 12 Tf 72 720 Td (Hello) Tj ET
        let ops = vec![
            ContentOp {
                operator: b"BT".to_vec(),
                operands: vec![],
            },
            ContentOp {
                operator: b"Tf".to_vec(),
                operands: vec![Operand::Name(b"F1".to_vec()), Operand::Integer(12)],
            },
            ContentOp {
                operator: b"Td".to_vec(),
                operands: vec![Operand::Integer(72), Operand::Integer(720)],
            },
            ContentOp {
                operator: b"Tj".to_vec(),
                operands: vec![Operand::String(b"Hello".to_vec())],
            },
            ContentOp {
                operator: b"ET".to_vec(),
                operands: vec![],
            },
        ];

        // Create a simple font (no CMap, WinAnsi)
        let mut fonts = HashMap::new();
        fonts.insert(
            b"F1".to_vec(),
            ResolvedFont {
                info: FontInfo {
                    base_font: b"Helvetica".to_vec(),
                    subtype: b"Type1".to_vec(),
                    encoding: Encoding::WinAnsiEncoding,
                    differences: Vec::new(),
                    widths: crate::font::FontWidths::None {
                        default_width: 600.0,
                    },
                    to_unicode: None,
                    is_standard14: true,
                    descriptor: None,
                },
                cmap: None,
                encoding: None,
            },
        );

        let interpreter = TextInterpreter::new(fonts);
        let chars = interpreter.run(&ops);

        assert_eq!(chars.len(), 5);
        assert_eq!(chars[0].unicode, "H");
        assert_eq!(chars[1].unicode, "e");
        assert_eq!(chars[4].unicode, "o");
        // Position: first char at (72, 720), font size 12
        assert!((chars[0].x - 72.0).abs() < 0.01);
        assert!((chars[0].y - 720.0).abs() < 0.01);
    }

    #[test]
    fn test_interpreter_tj_array() {
        // BT /F1 12 Tf 0 0 Td [(H) -100 (i)] TJ ET
        let ops = vec![
            ContentOp {
                operator: b"BT".to_vec(),
                operands: vec![],
            },
            ContentOp {
                operator: b"Tf".to_vec(),
                operands: vec![Operand::Name(b"F1".to_vec()), Operand::Integer(12)],
            },
            ContentOp {
                operator: b"TJ".to_vec(),
                operands: vec![Operand::Array(vec![
                    Operand::String(b"H".to_vec()),
                    Operand::Integer(-100),
                    Operand::String(b"i".to_vec()),
                ])],
            },
            ContentOp {
                operator: b"ET".to_vec(),
                operands: vec![],
            },
        ];

        let mut fonts = HashMap::new();
        fonts.insert(
            b"F1".to_vec(),
            ResolvedFont {
                info: FontInfo {
                    base_font: b"Helvetica".to_vec(),
                    subtype: b"Type1".to_vec(),
                    encoding: Encoding::WinAnsiEncoding,
                    differences: Vec::new(),
                    widths: crate::font::FontWidths::None {
                        default_width: 500.0,
                    },
                    to_unicode: None,
                    is_standard14: true,
                    descriptor: None,
                },
                cmap: None,
                encoding: None,
            },
        );

        let interpreter = TextInterpreter::new(fonts);
        let chars = interpreter.run(&ops);

        assert_eq!(chars.len(), 2);
        assert_eq!(chars[0].unicode, "H");
        assert_eq!(chars[1].unicode, "i");
        // "i" should be displaced by the kerning value
        let h_advance = 500.0 / 1000.0 * 12.0; // 6.0
        let kern = 100.0 / 1000.0 * 12.0; // 1.2
        let expected_i_x = h_advance + kern;
        assert!((chars[1].x - expected_i_x).abs() < 0.01);
    }

    #[test]
    fn test_word_grouping() {
        let chars = vec![
            TextChar {
                unicode: "H".into(),
                x: 72.0,
                y: 720.0,
                font_size: 12.0,
                font_name: "F1".into(),
                width: 7.0,
            },
            TextChar {
                unicode: "i".into(),
                x: 79.0,
                y: 720.0,
                font_size: 12.0,
                font_name: "F1".into(),
                width: 3.0,
            },
            TextChar {
                unicode: " ".into(),
                x: 82.0,
                y: 720.0,
                font_size: 12.0,
                font_name: "F1".into(),
                width: 3.0,
            },
            TextChar {
                unicode: "A".into(),
                x: 90.0,
                y: 720.0,
                font_size: 12.0,
                font_name: "F1".into(),
                width: 7.0,
            },
        ];

        let words = group_into_words(&chars);
        assert_eq!(words.len(), 2);
        assert_eq!(words[0].text, "Hi");
        assert_eq!(words[1].text, "A");
    }

    #[test]
    fn test_line_grouping() {
        let words = vec![
            TextWord {
                text: "Hello".into(),
                x: 72.0,
                y: 720.0,
                width: 30.0,
                font_size: 12.0,
            },
            TextWord {
                text: "World".into(),
                x: 110.0,
                y: 720.0,
                width: 30.0,
                font_size: 12.0,
            },
            TextWord {
                text: "Next".into(),
                x: 72.0,
                y: 700.0,
                width: 24.0,
                font_size: 12.0,
            },
        ];

        let lines = group_into_lines(&words);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].text, "Hello World");
        assert_eq!(lines[1].text, "Next");
    }

    #[test]
    fn test_interpreter_multiline() {
        // BT /F1 12 Tf 72 720 Td (Line1) Tj 0 -14 Td (Line2) Tj ET
        let ops = vec![
            ContentOp {
                operator: b"BT".to_vec(),
                operands: vec![],
            },
            ContentOp {
                operator: b"Tf".to_vec(),
                operands: vec![Operand::Name(b"F1".to_vec()), Operand::Integer(12)],
            },
            ContentOp {
                operator: b"Td".to_vec(),
                operands: vec![Operand::Integer(72), Operand::Integer(720)],
            },
            ContentOp {
                operator: b"Tj".to_vec(),
                operands: vec![Operand::String(b"Line1".to_vec())],
            },
            ContentOp {
                operator: b"Td".to_vec(),
                operands: vec![Operand::Integer(0), Operand::Integer(-14)],
            },
            ContentOp {
                operator: b"Tj".to_vec(),
                operands: vec![Operand::String(b"Line2".to_vec())],
            },
            ContentOp {
                operator: b"ET".to_vec(),
                operands: vec![],
            },
        ];

        let mut fonts = HashMap::new();
        fonts.insert(
            b"F1".to_vec(),
            ResolvedFont {
                info: FontInfo {
                    base_font: b"Courier".to_vec(),
                    subtype: b"Type1".to_vec(),
                    encoding: Encoding::WinAnsiEncoding,
                    differences: Vec::new(),
                    widths: crate::font::FontWidths::None {
                        default_width: 600.0,
                    },
                    to_unicode: None,
                    is_standard14: true,
                    descriptor: None,
                },
                cmap: None,
                encoding: None,
            },
        );

        let interpreter = TextInterpreter::new(fonts);
        let chars = interpreter.run(&ops);

        assert_eq!(chars.len(), 10);
        // Line1 starts at y=720
        assert!((chars[0].y - 720.0).abs() < 0.01);
        // Line2 starts at y=706 (720 - 14)
        assert!((chars[5].y - 706.0).abs() < 0.01);

        let words = group_into_words(&chars);
        let lines = group_into_lines(&words);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].text, "Line1");
        assert_eq!(lines[1].text, "Line2");
    }

    #[test]
    fn test_empty_chars() {
        let words = group_into_words(&[]);
        assert!(words.is_empty());
        let lines = group_into_lines(&[]);
        assert!(lines.is_empty());
        let blocks = layout::detect_columns_and_reorder(&[]);
        assert!(blocks.is_empty());
    }

    #[test]
    fn test_graphics_state_save_restore() {
        // q 2 0 0 2 0 0 cm BT /F1 12 Tf (A) Tj ET Q BT /F1 12 Tf (B) Tj ET
        let ops = vec![
            ContentOp {
                operator: b"q".to_vec(),
                operands: vec![],
            },
            ContentOp {
                operator: b"cm".to_vec(),
                operands: vec![
                    Operand::Integer(2),
                    Operand::Integer(0),
                    Operand::Integer(0),
                    Operand::Integer(2),
                    Operand::Integer(0),
                    Operand::Integer(0),
                ],
            },
            ContentOp {
                operator: b"BT".to_vec(),
                operands: vec![],
            },
            ContentOp {
                operator: b"Tf".to_vec(),
                operands: vec![Operand::Name(b"F1".to_vec()), Operand::Integer(12)],
            },
            ContentOp {
                operator: b"Tj".to_vec(),
                operands: vec![Operand::String(b"A".to_vec())],
            },
            ContentOp {
                operator: b"ET".to_vec(),
                operands: vec![],
            },
            ContentOp {
                operator: b"Q".to_vec(),
                operands: vec![],
            },
            ContentOp {
                operator: b"BT".to_vec(),
                operands: vec![],
            },
            ContentOp {
                operator: b"Tf".to_vec(),
                operands: vec![Operand::Name(b"F1".to_vec()), Operand::Integer(12)],
            },
            ContentOp {
                operator: b"Tj".to_vec(),
                operands: vec![Operand::String(b"B".to_vec())],
            },
            ContentOp {
                operator: b"ET".to_vec(),
                operands: vec![],
            },
        ];

        let mut fonts = HashMap::new();
        fonts.insert(
            b"F1".to_vec(),
            ResolvedFont {
                info: FontInfo {
                    base_font: b"Helvetica".to_vec(),
                    subtype: b"Type1".to_vec(),
                    encoding: Encoding::WinAnsiEncoding,
                    differences: Vec::new(),
                    widths: crate::font::FontWidths::None {
                        default_width: 600.0,
                    },
                    to_unicode: None,
                    is_standard14: true,
                    descriptor: None,
                },
                cmap: None,
                encoding: None,
            },
        );

        let interpreter = TextInterpreter::new(fonts);
        let chars = interpreter.run(&ops);

        assert_eq!(chars.len(), 2);
        assert_eq!(chars[0].unicode, "A");
        assert_eq!(chars[1].unicode, "B");
        // "A" should have scaled position (CTM = 2x)
        // "B" should have normal position (CTM restored to identity)
        // Both at origin since no Td, but font size differs due to CTM
        assert!((chars[0].font_size - 24.0).abs() < 0.01); // 12 * 2
        assert!((chars[1].font_size - 12.0).abs() < 0.01); // 12 * 1
    }
}
