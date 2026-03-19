pub mod format;
pub mod layout;
pub mod search;
pub mod text_layout;

use std::collections::HashMap;

use crate::content::{ContentOp, Operand, parse_content_stream};
use crate::error::Result;
use crate::font::{Encoding, FontInfo, ToUnicodeCMap, decode_text, parse_font_info};
use crate::object::{PdfDict, PdfObject};
use crate::page::{PageInfo, collect_pages};
use crate::parser::PdfDocument;

// ---------------------------------------------------------------------------
// 2D affine transformation matrix [a b 0; c d 0; e f 1]
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
pub struct Matrix {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub e: f64,
    pub f: f64,
}

impl Matrix {
    pub fn identity() -> Self {
        Self {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            e: 0.0,
            f: 0.0,
        }
    }

    /// Multiply: self × other (row-vector convention as in PDF spec).
    pub fn concat(&self, other: &Matrix) -> Matrix {
        Matrix {
            a: self.a * other.a + self.b * other.c,
            b: self.a * other.b + self.b * other.d,
            c: self.c * other.a + self.d * other.c,
            d: self.c * other.b + self.d * other.d,
            e: self.e * other.a + self.f * other.c + other.e,
            f: self.e * other.b + self.f * other.d + other.f,
        }
    }

    /// Transform a point (x, y) by this matrix.
    pub fn transform_point(&self, x: f64, y: f64) -> (f64, f64) {
        (
            self.a * x + self.c * y + self.e,
            self.b * x + self.d * y + self.f,
        )
    }

    /// Translation matrix.
    pub fn translate(tx: f64, ty: f64) -> Self {
        Self {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            e: tx,
            f: ty,
        }
    }

    /// Get the effective font size (y-scale factor).
    pub fn font_size_scale(&self) -> f64 {
        (self.b * self.b + self.d * self.d).sqrt()
    }
}

// ---------------------------------------------------------------------------
// Font entry resolved for text extraction
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct ResolvedFont {
    info: FontInfo,
    cmap: Option<ToUnicodeCMap>,
}

impl ResolvedFont {
    /// Get the width of a character code in text space units (1/1000).
    fn char_width(&self, code: u32) -> f64 {
        self.info.widths.get_width(code)
    }
}

// ---------------------------------------------------------------------------
// Text state (per PDF spec 9.3)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct TextState {
    /// Character spacing (Tc)
    char_spacing: f64,
    /// Word spacing (Tw)
    word_spacing: f64,
    /// Horizontal scaling (Tz), stored as fraction (1.0 = 100%)
    horiz_scaling: f64,
    /// Text leading (TL)
    leading: f64,
    /// Current font name (resource name, e.g. "F1")
    font_name: Vec<u8>,
    /// Font size (Tfs)
    font_size: f64,
    /// Text rise (Ts)
    text_rise: f64,
}

impl Default for TextState {
    fn default() -> Self {
        Self {
            char_spacing: 0.0,
            word_spacing: 0.0,
            horiz_scaling: 1.0,
            leading: 0.0,
            font_name: Vec::new(),
            font_size: 12.0,
            text_rise: 0.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Graphics state (subset needed for text extraction)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct GraphicsState {
    ctm: Matrix,
    text: TextState,
}

impl Default for GraphicsState {
    fn default() -> Self {
        Self {
            ctm: Matrix::identity(),
            text: TextState::default(),
        }
    }
}

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
    /// Font name (resource name).
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
    /// Get plain text, joining lines with newlines.
    pub fn plain_text(&self) -> String {
        self.blocks
            .iter()
            .map(|b| b.text.as_str())
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}

// ---------------------------------------------------------------------------
// Content stream text interpreter
// ---------------------------------------------------------------------------

struct TextInterpreter {
    /// Graphics state stack.
    gs_stack: Vec<GraphicsState>,
    /// Current graphics state.
    gs: GraphicsState,
    /// Text matrix (Tm).
    tm: Matrix,
    /// Text line matrix (Tlm) — set at the start of each text line.
    tlm: Matrix,
    /// Whether we're inside BT...ET.
    in_text: bool,
    /// Resolved fonts keyed by resource name.
    fonts: HashMap<Vec<u8>, ResolvedFont>,
    /// Extracted characters.
    chars: Vec<TextChar>,
}

impl TextInterpreter {
    fn new(fonts: HashMap<Vec<u8>, ResolvedFont>) -> Self {
        Self {
            gs_stack: Vec::new(),
            gs: GraphicsState::default(),
            tm: Matrix::identity(),
            tlm: Matrix::identity(),
            in_text: false,
            fonts,
            chars: Vec::new(),
        }
    }

    fn run(mut self, ops: &[ContentOp]) -> Vec<TextChar> {
        for op in ops {
            self.process_op(op);
        }
        self.chars
    }

    fn process_op(&mut self, op: &ContentOp) {
        match op.operator.as_slice() {
            // Graphics state
            b"q" => self.gs_stack.push(self.gs.clone()),
            b"Q" => {
                if let Some(gs) = self.gs_stack.pop() {
                    self.gs = gs;
                }
            }
            b"cm" => {
                if let Some(m) = self.read_matrix(&op.operands) {
                    self.gs.ctm = m.concat(&self.gs.ctm);
                }
            }

            // Text state operators
            b"Tc" => {
                if let Some(v) = op.operands.first().and_then(|o| o.as_f64()) {
                    self.gs.text.char_spacing = v;
                }
            }
            b"Tw" => {
                if let Some(v) = op.operands.first().and_then(|o| o.as_f64()) {
                    self.gs.text.word_spacing = v;
                }
            }
            b"Tz" => {
                if let Some(v) = op.operands.first().and_then(|o| o.as_f64()) {
                    self.gs.text.horiz_scaling = v / 100.0;
                }
            }
            b"TL" => {
                if let Some(v) = op.operands.first().and_then(|o| o.as_f64()) {
                    self.gs.text.leading = v;
                }
            }
            b"Tf" => {
                if op.operands.len() >= 2 {
                    if let Some(name) = op.operands[0].as_name() {
                        self.gs.text.font_name = name.to_vec();
                    }
                    if let Some(size) = op.operands[1].as_f64() {
                        self.gs.text.font_size = size;
                    }
                }
            }
            b"Ts" => {
                if let Some(v) = op.operands.first().and_then(|o| o.as_f64()) {
                    self.gs.text.text_rise = v;
                }
            }

            // Text object
            b"BT" => {
                self.in_text = true;
                self.tm = Matrix::identity();
                self.tlm = Matrix::identity();
            }
            b"ET" => {
                self.in_text = false;
            }

            // Text positioning
            b"Td" => {
                if op.operands.len() >= 2 {
                    let tx = op.operands[0].as_f64().unwrap_or(0.0);
                    let ty = op.operands[1].as_f64().unwrap_or(0.0);
                    self.tlm = Matrix::translate(tx, ty).concat(&self.tlm);
                    self.tm = self.tlm;
                }
            }
            b"TD" => {
                if op.operands.len() >= 2 {
                    let tx = op.operands[0].as_f64().unwrap_or(0.0);
                    let ty = op.operands[1].as_f64().unwrap_or(0.0);
                    self.gs.text.leading = -ty;
                    self.tlm = Matrix::translate(tx, ty).concat(&self.tlm);
                    self.tm = self.tlm;
                }
            }
            b"Tm" => {
                if let Some(m) = self.read_matrix(&op.operands) {
                    self.tm = m;
                    self.tlm = m;
                }
            }
            b"T*" => {
                let tl = self.gs.text.leading;
                self.tlm = Matrix::translate(0.0, -tl).concat(&self.tlm);
                self.tm = self.tlm;
            }

            // Text showing
            b"Tj" => {
                if let Some(s) = op.operands.first().and_then(|o| o.as_str()) {
                    self.show_string(s);
                }
            }
            b"TJ" => {
                if let Some(arr) = op.operands.first().and_then(|o| o.as_array()) {
                    self.show_tj_array(arr);
                }
            }
            b"'" => {
                // Move to next line, then show string
                let tl = self.gs.text.leading;
                self.tlm = Matrix::translate(0.0, -tl).concat(&self.tlm);
                self.tm = self.tlm;
                if let Some(s) = op.operands.first().and_then(|o| o.as_str()) {
                    self.show_string(s);
                }
            }
            b"\"" => {
                // Set Tw, Tc, then T* + Tj
                if op.operands.len() >= 3 {
                    if let Some(aw) = op.operands[0].as_f64() {
                        self.gs.text.word_spacing = aw;
                    }
                    if let Some(ac) = op.operands[1].as_f64() {
                        self.gs.text.char_spacing = ac;
                    }
                    let tl = self.gs.text.leading;
                    self.tlm = Matrix::translate(0.0, -tl).concat(&self.tlm);
                    self.tm = self.tlm;
                    if let Some(s) = op.operands[2].as_str() {
                        self.show_string(s);
                    }
                }
            }

            // ExtGState (may contain font)
            b"gs" => {
                // We don't resolve ExtGState for now — would need document access
            }

            _ => {} // Ignore non-text operators
        }
    }

    /// Show a text string, extracting characters and updating the text matrix.
    fn show_string(&mut self, raw: &[u8]) {
        let font = self.fonts.get(&self.gs.text.font_name);

        let is_two_byte = font
            .map(|f| {
                matches!(f.info.encoding, Encoding::Identity) || f.info.subtype == b"Type0"
            })
            .unwrap_or(false);

        let tfs = self.gs.text.font_size;
        let tc = self.gs.text.char_spacing;
        let tw = self.gs.text.word_spacing;
        let th = self.gs.text.horiz_scaling;
        let rise = self.gs.text.text_rise;

        // Iterate over character codes
        let mut i = 0;
        while i < raw.len() {
            let (code, byte_len) = if is_two_byte && i + 1 < raw.len() {
                (((raw[i] as u32) << 8) | raw[i + 1] as u32, 2)
            } else {
                (raw[i] as u32, 1)
            };

            // Decode to unicode
            let unicode = if let Some(f) = font {
                if let Some(ref cmap) = f.cmap {
                    cmap.lookup(code)
                        .unwrap_or_else(|| decode_text(&raw[i..i + byte_len], f.info.encoding))
                } else {
                    decode_text(&raw[i..i + byte_len], f.info.encoding)
                }
            } else {
                String::from_utf8_lossy(&raw[i..i + byte_len]).into_owned()
            };

            // Get glyph width in text space (1/1000 units)
            let w0 = font
                .map(|f| f.char_width(code))
                .unwrap_or(500.0);

            // Calculate position in user space:
            // Text rendering matrix = [fontSize*Tz 0 0; 0 fontSize 0; 0 rise 1] × Tm × CTM
            let trm = self.text_rendering_matrix(tfs, th, rise);
            let (x, y) = trm.transform_point(0.0, 0.0);
            let effective_size = trm.font_size_scale();

            // Glyph displacement in text space
            let tx = w0 / 1000.0 * tfs;
            let advance = (tx + tc) * th;

            // Add word spacing for space character (code 32)
            let total_advance = if code == 32 {
                advance + tw * th
            } else {
                advance
            };

            // Calculate width in user space
            let width = (w0 / 1000.0 * tfs * th).abs();

            // Only emit non-control characters
            if !unicode.is_empty() {
                self.chars.push(TextChar {
                    unicode,
                    x,
                    y,
                    font_size: effective_size,
                    font_name: String::from_utf8_lossy(&self.gs.text.font_name).into_owned(),
                    width,
                });
            }

            // Update text matrix: translate by glyph displacement
            self.tm = Matrix::translate(total_advance, 0.0).concat(&self.tm);

            i += byte_len;
        }
    }

    /// Process a TJ array: [(string) number (string) number ...]
    fn show_tj_array(&mut self, items: &[Operand]) {
        let th = self.gs.text.horiz_scaling;
        let tfs = self.gs.text.font_size;

        for item in items {
            match item {
                Operand::String(s) => {
                    self.show_string(s);
                }
                Operand::Integer(n) => {
                    // Displacement in thousandths of text space unit
                    let displacement = -*n as f64 / 1000.0 * tfs * th;
                    self.tm = Matrix::translate(displacement, 0.0).concat(&self.tm);
                }
                Operand::Real(n) => {
                    let displacement = -n / 1000.0 * tfs * th;
                    self.tm = Matrix::translate(displacement, 0.0).concat(&self.tm);
                }
                _ => {}
            }
        }
    }

    /// Compute the text rendering matrix (Trm = text_state_matrix × Tm × CTM).
    fn text_rendering_matrix(&self, tfs: f64, th: f64, rise: f64) -> Matrix {
        let text_state = Matrix {
            a: tfs * th,
            b: 0.0,
            c: 0.0,
            d: tfs,
            e: 0.0,
            f: rise,
        };
        text_state.concat(&self.tm).concat(&self.gs.ctm)
    }

    fn read_matrix(&self, operands: &[Operand]) -> Option<Matrix> {
        if operands.len() < 6 {
            return None;
        }
        Some(Matrix {
            a: operands[0].as_f64()?,
            b: operands[1].as_f64()?,
            c: operands[2].as_f64()?,
            d: operands[3].as_f64()?,
            e: operands[4].as_f64()?,
            f: operands[5].as_f64()?,
        })
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

    TextLine {
        text,
        x,
        y,
        words,
    }
}

// ---------------------------------------------------------------------------
// Font resolution from page resources
// ---------------------------------------------------------------------------

/// Resolve all fonts from a page's Resources dictionary.
fn resolve_fonts(
    doc: &mut PdfDocument,
    resources_ref: &Option<PdfObject>,
) -> HashMap<Vec<u8>, ResolvedFont> {
    let mut fonts = HashMap::new();

    let resources = match resources_ref {
        Some(PdfObject::Dict(d)) => d.clone(),
        Some(PdfObject::Reference(r)) => {
            let r = r.clone();
            match doc.resolve(&r) {
                Ok(PdfObject::Dict(d)) => d.clone(),
                _ => return fonts,
            }
        }
        _ => return fonts,
    };

    let font_dict = match resources.get(b"Font") {
        Some(PdfObject::Dict(d)) => d.clone(),
        Some(PdfObject::Reference(r)) => {
            let r = r.clone();
            match doc.resolve(&r) {
                Ok(PdfObject::Dict(d)) => d.clone(),
                _ => return fonts,
            }
        }
        _ => return fonts,
    };

    for (name, obj) in font_dict.iter() {
        let font_dict = match obj {
            PdfObject::Dict(d) => d.clone(),
            PdfObject::Reference(r) => {
                let r = r.clone();
                match doc.resolve(&r) {
                    Ok(PdfObject::Dict(d)) => d.clone(),
                    _ => continue,
                }
            }
            _ => continue,
        };

        let mut info = parse_font_info(&font_dict);

        // Resolve ToUnicode CMap
        let cmap = resolve_to_unicode(doc, &font_dict);
        if cmap.is_some() {
            info.to_unicode = None; // We already parsed it
        }

        // For Type0 fonts, try to get widths from descendant CIDFont
        if font_dict.get_name(b"Subtype") == Some(b"Type0") {
            resolve_type0_descendant(doc, &font_dict, &mut info);
        }

        fonts.insert(name.clone(), ResolvedFont { info, cmap });
    }

    fonts
}

fn resolve_to_unicode(doc: &mut PdfDocument, font_dict: &PdfDict) -> Option<ToUnicodeCMap> {
    let tu_obj = font_dict.get(b"ToUnicode")?;
    match tu_obj {
        PdfObject::Reference(r) => {
            let r = r.clone();
            let obj = doc.resolve(&r).ok()?.clone();
            match obj {
                PdfObject::Stream { dict, data } => {
                    let decoded = doc.decode_stream(&dict, &data).ok()?;
                    Some(ToUnicodeCMap::parse(&decoded))
                }
                _ => None,
            }
        }
        PdfObject::Stream { dict, data } => {
            let decoded = doc.decode_stream(dict, data).ok()?;
            Some(ToUnicodeCMap::parse(&decoded))
        }
        _ => None,
    }
}

fn resolve_type0_descendant(doc: &mut PdfDocument, font_dict: &PdfDict, info: &mut FontInfo) {
    let descendants = match font_dict.get(b"DescendantFonts") {
        Some(PdfObject::Array(arr)) => arr.clone(),
        _ => return,
    };

    let descendant_ref = match descendants.first() {
        Some(PdfObject::Reference(r)) => r.clone(),
        _ => return,
    };

    let descendant = match doc.resolve(&descendant_ref) {
        Ok(PdfObject::Dict(d)) => d.clone(),
        _ => return,
    };

    // Get /W array for CID widths
    if let Some(PdfObject::Array(w_array)) = descendant.get(b"W") {
        info.widths = parse_cid_widths(w_array);
    }

    // Get /DW (default width)
    if let Some(dw) = descendant.get(b"DW").and_then(|o| o.as_f64()) {
        match &mut info.widths {
            crate::font::FontWidths::CID {
                default_width,
                ..
            } => *default_width = dw,
            crate::font::FontWidths::None {
                default_width,
            } => *default_width = dw,
            _ => {}
        }
    }

    // Mark as Identity encoding for Type0
    info.encoding = Encoding::Identity;
}

fn parse_cid_widths(w_array: &[PdfObject]) -> crate::font::FontWidths {
    use crate::font::{CIDWidthEntry, FontWidths};

    let mut entries = Vec::new();
    let mut i = 0;

    while i < w_array.len() {
        let first = match w_array[i].as_i64() {
            Some(v) => v as u32,
            None => {
                i += 1;
                continue;
            }
        };
        i += 1;

        if i >= w_array.len() {
            break;
        }

        match &w_array[i] {
            PdfObject::Array(widths) => {
                let ws: Vec<f64> = widths.iter().filter_map(|o| o.as_f64()).collect();
                entries.push(CIDWidthEntry::List {
                    first,
                    widths: ws,
                });
                i += 1;
            }
            PdfObject::Integer(_) | PdfObject::Real(_) => {
                if i + 1 < w_array.len() {
                    let last = w_array[i].as_f64().unwrap_or(0.0) as u32;
                    i += 1;
                    let width = w_array.get(i).and_then(|o| o.as_f64()).unwrap_or(1000.0);
                    i += 1;
                    entries.push(CIDWidthEntry::Range {
                        first,
                        last,
                        width,
                    });
                } else {
                    break;
                }
            }
            _ => {
                i += 1;
            }
        }
    }

    FontWidths::CID {
        default_width: 1000.0,
        w_entries: entries,
    }
}

// ---------------------------------------------------------------------------
// Get content stream data for a page
// ---------------------------------------------------------------------------

fn get_page_content_data(doc: &mut PdfDocument, page: &PageInfo) -> Result<Vec<u8>> {
    let contents_obj = match &page.contents_ref {
        Some(obj) => obj.clone(),
        None => return Ok(Vec::new()),
    };

    match contents_obj {
        PdfObject::Reference(r) => {
            let obj = doc.resolve(&r)?.clone();
            decode_content_obj(doc, &obj)
        }
        PdfObject::Array(arr) => {
            let mut combined = Vec::new();
            for item in &arr {
                let data = match item {
                    PdfObject::Reference(r) => {
                        let r = r.clone();
                        let obj = doc.resolve(&r)?.clone();
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
pub fn extract_page_text(doc: &mut PdfDocument, page: &PageInfo) -> Result<PageText> {
    // Resolve fonts
    let fonts = resolve_fonts(doc, &page.resources_ref);

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
    let interpreter = TextInterpreter::new(fonts);
    let chars = interpreter.run(&ops);

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
pub fn extract_all_text(doc: &mut PdfDocument) -> Result<Vec<PageText>> {
    let pages = collect_pages(doc)?;
    let mut results = Vec::with_capacity(pages.len());

    for page in &pages {
        results.push(extract_page_text(doc, page)?);
    }

    Ok(results)
}

/// Extract plain text from a single page as a string.
pub fn extract_page_text_string(doc: &mut PdfDocument, page: &PageInfo) -> Result<String> {
    let page_text = extract_page_text(doc, page)?;
    Ok(page_text.plain_text())
}

/// Extract plain text from all pages, joining with form feeds.
pub fn extract_all_text_string(doc: &mut PdfDocument) -> Result<String> {
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
                operands: vec![
                    Operand::Name(b"F1".to_vec()),
                    Operand::Integer(12),
                ],
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
                    widths: crate::font::FontWidths::None {
                        default_width: 600.0,
                    },
                    to_unicode: None,
                    is_standard14: true,
                    descriptor: None,
                },
                cmap: None,
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
                operands: vec![
                    Operand::Name(b"F1".to_vec()),
                    Operand::Integer(12),
                ],
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
                    widths: crate::font::FontWidths::None {
                        default_width: 500.0,
                    },
                    to_unicode: None,
                    is_standard14: true,
                    descriptor: None,
                },
                cmap: None,
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
                operands: vec![
                    Operand::Name(b"F1".to_vec()),
                    Operand::Integer(12),
                ],
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
                    widths: crate::font::FontWidths::None {
                        default_width: 600.0,
                    },
                    to_unicode: None,
                    is_standard14: true,
                    descriptor: None,
                },
                cmap: None,
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
                operands: vec![
                    Operand::Name(b"F1".to_vec()),
                    Operand::Integer(12),
                ],
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
                operands: vec![
                    Operand::Name(b"F1".to_vec()),
                    Operand::Integer(12),
                ],
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
                    widths: crate::font::FontWidths::None {
                        default_width: 600.0,
                    },
                    to_unicode: None,
                    is_standard14: true,
                    descriptor: None,
                },
                cmap: None,
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
