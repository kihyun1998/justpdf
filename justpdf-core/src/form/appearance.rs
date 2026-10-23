use std::fmt::Write;

use crate::content::Operand;
use crate::content::{ContentOp, write_content};
use crate::font::{FontEncoder, encode_winansi, standard14_ascent_descent, standard14_widths};
use crate::object::{IndirectRef, Number, PdfDict, PdfObject, string_syntax};
use crate::writer::document::standard_font_dict;
use crate::writer::encode::make_stream;
use crate::writer::modify::DocumentModifier;

use super::types::*;

/// Why [`generate_field_appearance`] made no appearance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum AppearanceError {
    /// The widget has no `/Rect`, or one with no area.
    #[error("the widget has no area to draw in")]
    NoRect,
    /// A signature field, whose appearance comes with its signature.
    #[error("a signature field has its own appearance")]
    Signature,
    /// The field's `/DA` names no font, or a font the AcroForm `/DR` does not
    /// hold, or one that cannot encode every character of the text shown.
    #[error("the field's /DA font cannot show its text")]
    NotEncodable,
}

/// Generate an appearance stream for a form field widget and add it to
/// `modifier`. Returns the indirect reference to the Form XObject.
///
/// A text field, combo box, list box or push button with `/DA` draws in the
/// font `/DA` names, looked up in the AcroForm `/DR` of `modifier`'s document
/// and declared under the same name in the appearance's `/Resources`, its text
/// in that font's codes. Without `/DA` it draws in a WinAnsi `/Helvetica`.
/// Nothing is added when it returns an error.
pub fn generate_field_appearance(
    field: &FormField,
    modifier: &mut DocumentModifier,
) -> Result<IndirectRef, AppearanceError> {
    let rect = field.rect.ok_or(AppearanceError::NoRect)?;
    let w = rect.width();
    let h = rect.height();
    if w <= 0.0 || h <= 0.0 {
        return Err(AppearanceError::NoRect);
    }

    let font = FieldFont::of(field, modifier)?;
    let look = WidgetLook::of(field, modifier);
    let content = match field.field_type {
        FieldType::Text => text_field_appearance(field, &font, &look, w, h)?,
        FieldType::Checkbox => checkbox_appearance(field, &look, w, h),
        FieldType::RadioButton => radio_appearance(field, &look, w, h),
        FieldType::ComboBox => combo_appearance(field, &font, &look, w, h)?,
        FieldType::ListBox => list_appearance(field, &font, &look, w, h)?,
        FieldType::PushButton => button_appearance(field, &font, &look, w, h)?,
        FieldType::Signature => return Err(AppearanceError::Signature),
    };

    let (stream_dict, stream_data) = make_stream(content.as_bytes(), true);
    let mut form_dict = stream_dict;
    form_dict.insert(b"Type".to_vec(), PdfObject::Name(b"XObject".to_vec()));
    form_dict.insert(b"Subtype".to_vec(), PdfObject::Name(b"Form".to_vec()));
    if shows_text(field) {
        form_dict.insert(b"Resources".to_vec(), PdfObject::Dict(font.resources()));
    }
    form_dict.insert(
        b"BBox".to_vec(),
        PdfObject::Array(vec![
            PdfObject::Real(0.0),
            PdfObject::Real(0.0),
            PdfObject::Real(w),
            PdfObject::Real(h),
        ]),
    );

    let form_xobj = PdfObject::Stream {
        dict: form_dict,
        data: stream_data,
    };
    Ok(modifier.add_object(form_xobj))
}

/// Whether the appearance of `field` shows text: a text field, combo box,
/// list box or push button.
fn shows_text(field: &FormField) -> bool {
    matches!(
        field.field_type,
        FieldType::Text | FieldType::ComboBox | FieldType::ListBox | FieldType::PushButton
    )
}

/// The font a field's text is drawn in.
enum FieldFont {
    /// justpdf's WinAnsi `/Helvetica`, for a field without `/DA`.
    Default,
    /// The AcroForm `/DR` font `/DA` names.
    Dr {
        /// The `/DA` string, which selects the font.
        da: String,
        /// The size the last `Tf` of `/DA` sets; 0 is auto size.
        size: f64,
        /// The font's name in `/DR` and `/DA`.
        name: Vec<u8>,
        /// The `/DR` entry: a reference to the font, or the font dictionary.
        font: PdfObject,
        encoder: FontEncoder,
    },
}

impl FieldFont {
    fn of(field: &FormField, modifier: &DocumentModifier) -> Result<FieldFont, AppearanceError> {
        match &field.default_appearance {
            Some(da) if shows_text(field) => {
                dr_font(da, modifier).ok_or(AppearanceError::NotEncodable)
            }
            _ => Ok(FieldFont::Default),
        }
    }

    /// The operators that select the font, for after `BT`.
    fn select(&self) -> &str {
        match self {
            FieldFont::Default => "/Helvetica 10 Tf",
            FieldFont::Dr { da, .. } => da,
        }
    }

    /// The font size `/DA` sets: 0 is auto size. 10 without `/DA`.
    fn size(&self) -> f64 {
        match self {
            FieldFont::Default => 10.0,
            FieldFont::Dr { size, .. } => *size,
        }
    }

    /// The operators that select the font at `size`: [`Self::select`] as it
    /// is when `size` is the one it sets, else with its last `Tf` set to
    /// `size`.
    fn select_at(&self, size: f64) -> String {
        if size == self.size() {
            return self.select().to_string();
        }
        let Ok(mut ops) = crate::content::parse_content_stream(self.select().as_bytes()) else {
            return self.select().to_string();
        };
        if let Some(tf) = ops.iter_mut().rev().find(|op| op.operator == b"Tf")
            && let Some(operand) = tf.operands.get_mut(1)
        {
            *operand = Operand::Real(size);
        }
        let ops: Vec<ContentOp> = ops;
        String::from_utf8_lossy(&write_content(&ops))
            .trim_end()
            .replace('\n', " ")
    }

    /// The width of `text` in em (text space units at size 1).
    fn width(&self, text: &str) -> Result<f64, AppearanceError> {
        match self {
            FieldFont::Default => {
                let widths = standard14_widths(b"Helvetica");
                Ok(encode_winansi(text)
                    .iter()
                    .map(|&code| widths[usize::from(code)])
                    .sum::<f64>()
                    / 1000.0)
            }
            FieldFont::Dr { encoder, .. } => encoder
                .width(text)
                .map(|w| w / 1000.0)
                .ok_or(AppearanceError::NotEncodable),
        }
    }

    /// The font's ascent and descent magnitude in em, when known.
    fn metrics(&self) -> Option<(f64, f64)> {
        let known = match self {
            FieldFont::Default => standard14_ascent_descent(b"Helvetica"),
            FieldFont::Dr { encoder, .. } => encoder.ascent_descent(),
        };
        known.map(|(a, d)| (a / 1000.0, d / 1000.0))
    }

    /// [`Self::metrics`], else pdf.js's fallback.
    fn ascent_descent(&self) -> (f64, f64) {
        self.metrics()
            .unwrap_or((LINE_FACTOR - LINE_DESCENT_FACTOR, LINE_DESCENT_FACTOR))
    }

    /// `text` in the font's codes.
    fn encode(&self, text: &str) -> Result<Vec<u8>, AppearanceError> {
        match self {
            FieldFont::Default => Ok(encode_winansi(text)),
            FieldFont::Dr { encoder, .. } => {
                encoder.encode(text).ok_or(AppearanceError::NotEncodable)
            }
        }
    }

    /// `/Resources` declaring the font.
    fn resources(&self) -> PdfDict {
        let mut fonts = PdfDict::new();
        match self {
            FieldFont::Default => fonts.insert(
                b"Helvetica".to_vec(),
                PdfObject::Dict(standard_font_dict("Helvetica")),
            ),
            FieldFont::Dr { name, font, .. } => fonts.insert(name.clone(), font.clone()),
        };
        let mut resources = PdfDict::new();
        resources.insert(b"Font".to_vec(), PdfObject::Dict(fonts));
        resources
    }
}

/// The font the last `Tf` of `da` names, from the AcroForm `/DR` of
/// `modifier`'s document. `None` when there is no such font or it can encode
/// nothing.
fn dr_font(da: &str, modifier: &DocumentModifier) -> Option<FieldFont> {
    let tf = crate::content::parse_content_stream(da.as_bytes())
        .ok()?
        .into_iter()
        .rev()
        .find(|op| op.operator == b"Tf")?;
    let Some(Operand::Name(name)) = tf.operands.first().cloned() else {
        return None;
    };
    let size = match tf.operands.get(1) {
        Some(Operand::Integer(i)) => *i as f64,
        Some(Operand::Real(r)) => *r,
        _ => 0.0,
    };
    let resolve = |r: &IndirectRef| modifier.find_object_pub(r.obj_num).cloned();
    let direct = |o: &PdfObject| match o {
        PdfObject::Reference(r) => resolve(r),
        other => Some(other.clone()),
    };
    let catalog = resolve(&modifier.catalog_ref())?;
    let acroform = direct(catalog.as_dict()?.get(b"AcroForm")?)?;
    let dr = direct(acroform.as_dict()?.get(b"DR")?)?;
    let fonts = direct(dr.as_dict()?.get(b"Font")?)?;
    let font = fonts.as_dict()?.get(&name)?.clone();
    let PdfObject::Dict(font_dict) = direct(&font)? else {
        return None;
    };
    let encoder = FontEncoder::new(&font_dict, resolve, |dict, data| {
        crate::stream::decode_stream(data, dict).ok()
    })?;
    Some(FieldFont::Dr {
        da: da.to_string(),
        size,
        name,
        font,
        encoder,
    })
}

/// A widget's background and border: the colour operators of its `/MK /BG`
/// (fill) and `/MK /BC` (stroke), and its border width.
struct WidgetLook {
    background: Option<String>,
    border: Option<String>,
    /// `/BS /W`, else `/Border[2]`, else 1.
    width: f64,
    /// The widget's own `/Q`, when it is 0, 1 or 2.
    quadding: Option<u8>,
}

impl WidgetLook {
    /// The look of the first of `field`'s widgets in `modifier`.
    fn of(field: &FormField, modifier: &DocumentModifier) -> WidgetLook {
        let direct = |o: &PdfObject| match o {
            PdfObject::Reference(r) => modifier.find_object_pub(r.obj_num).cloned(),
            other => Some(other.clone()),
        };
        let widget = field
            .widgets
            .first()
            .and_then(|r| modifier.find_object_pub(r.obj_num))
            .and_then(|o| o.as_dict().cloned())
            .unwrap_or_default();
        let mk = widget
            .get(b"MK")
            .and_then(direct)
            .and_then(|o| o.as_dict().cloned())
            .unwrap_or_default();
        let colour = |key: &[u8], stroke: bool| {
            let arr = direct(mk.get(key)?)?;
            colour_operator(arr.as_array()?, stroke)
        };
        let bs_width = widget
            .get(b"BS")
            .and_then(direct)
            .and_then(|bs| direct(bs.as_dict()?.get(b"W")?)?.as_f64());
        let border_width = || {
            let border = direct(widget.get(b"Border")?)?;
            direct(border.as_array()?.get(2)?)?.as_f64()
        };
        WidgetLook {
            background: colour(b"BG", false),
            border: colour(b"BC", true),
            width: bs_width.or_else(border_width).unwrap_or(1.0),
            quadding: widget
                .get_i64(b"Q")
                .and_then(|q| u8::try_from(q).ok())
                .filter(|q| *q <= 2),
        }
    }

    /// The background and border of a `w` × `h` rectangle.
    fn rect(&self, buf: &mut String, w: f64, h: f64) {
        if let Some(fill) = &self.background {
            let _ = write!(buf, "{fill}\n0 0 {} {} re\nf\n", Number(w), Number(h));
        }
        if let Some(stroke) = &self.border
            && self.width > 0.0
        {
            let b = self.width;
            let _ = write!(
                buf,
                "{} w\n{stroke}\n{} {} {} {} re\nS\n",
                Number(b),
                Number(b / 2.0),
                Number(b / 2.0),
                Number(w - b),
                Number(h - b)
            );
        }
    }

    /// The background and border of a circle of radius `r` at (`cx`, `cy`).
    fn circle(&self, buf: &mut String, cx: f64, cy: f64, r: f64) {
        if let Some(fill) = &self.background {
            let _ = writeln!(buf, "{fill}");
            append_circle(buf, cx, cy, r, CIRCLE_K);
            buf.push_str("f\n");
        }
        if let Some(stroke) = &self.border
            && self.width > 0.0
        {
            let _ = write!(buf, "{} w\n{stroke}\n", Number(self.width));
            append_circle(buf, cx, cy, r, CIRCLE_K);
            buf.push_str("S\n");
        }
    }
}

/// The Bézier control-point factor of a quarter circle.
const CIRCLE_K: f64 = 0.5522847498;

/// The operator setting the colour `components` give: gray for 1 component,
/// RGB for 3, CMYK for 4; `None` for any other length (0 is no colour).
fn colour_operator(components: &[PdfObject], stroke: bool) -> Option<String> {
    let values = components
        .iter()
        .map(|c| c.as_f64().map(|v| Number(v).to_string()))
        .collect::<Option<Vec<_>>>()?;
    let operator = match (values.len(), stroke) {
        (1, false) => "g",
        (1, true) => "G",
        (3, false) => "rg",
        (3, true) => "RG",
        (4, false) => "k",
        (4, true) => "K",
        _ => return None,
    };
    Some(format!("{} {operator}", values.join(" ")))
}

fn text_field_appearance(
    field: &FormField,
    font: &FieldFont,
    look: &WidgetLook,
    w: f64,
    h: f64,
) -> Result<String, AppearanceError> {
    let mut decoration = String::new();
    look.rect(&mut decoration, w, h);
    let b = look.width;
    single_line(
        field,
        font,
        look,
        &decoration,
        [b, b, w - 2.0 * b, h - 2.0 * b],
    )
}

/// pdf.js `TextWidgetAnnotation`'s padding inside the text box: horizontal,
/// then vertical.
const H_PADDING: f64 = 2.0;
const V_PADDING: f64 = 1.0;
/// pdf.js's line height and descent per unit font size (`LINE_FACTOR`,
/// `LINE_DESCENT_FACTOR`).
const LINE_FACTOR: f64 = 1.35;
const LINE_DESCENT_FACTOR: f64 = 0.35;
/// pdf.js's `BASELINE_FACTOR`: the descent per unit line height.
const BASELINE_FACTOR: f64 = LINE_DESCENT_FACTOR / LINE_FACTOR;

/// A single-line field's appearance: `/Tx BMC q`, `decoration` (background,
/// border), then the value laid out in `text_box` (`[x, y, width, height]`)
/// and clipped to it, then `Q EMC`. A password field, or one with no value,
/// draws no value.
fn single_line(
    field: &FormField,
    font: &FieldFont,
    look: &WidgetLook,
    decoration: &str,
    text_box: [f64; 4],
) -> Result<String, AppearanceError> {
    let mut buf = format!("/Tx BMC\nq\n{decoration}");
    let value = field
        .value_as_string()
        .filter(|t| !t.is_empty() && !field.flags.has(FieldFlags::PASSWORD));
    if let Some(text) = value {
        let quadding = look.quadding.unwrap_or(field.quadding);
        let flags = field.flags;
        let shown = if field.field_type == FieldType::Text && flags.has(FieldFlags::MULTILINE) {
            multiline_text(&text, font, quadding, text_box)?
        } else if field.field_type == FieldType::Text
            && flags.has(FieldFlags::COMB)
            && !flags.has(FieldFlags::PASSWORD)
            && !flags.has(FieldFlags::FILE_SELECT)
            && field.max_len > 0
        {
            comb_text(&text, font, quadding, field.max_len, text_box)?
        } else {
            line_text(&text, font, quadding, text_box)?
        };
        if let Some((size, shown)) = shown {
            let [x0, y0, bw, bh] = text_box;
            let _ = write!(
                buf,
                "{} {} {} {} re\nW\nn\nBT\n0 g\n{}\n{shown}ET\n",
                Number(x0),
                Number(y0),
                Number(bw),
                Number(bh),
                font.select_at(size),
            );
        }
    }
    buf.push_str("Q\nEMC\n");
    Ok(buf)
}

/// The single-line auto size of `text` in `text_box` (pdf.js
/// `_computeFontSize` with `lineCount == -1`), or the `/DA` size; `None`
/// when an auto size has no room.
fn line_size(
    text: &str,
    font: &FieldFont,
    text_box: [f64; 4],
) -> Result<Option<f64>, AppearanceError> {
    let [_, _, bw, bh] = text_box;
    Ok(match font.size() {
        s if s > 0.0 => Some(s),
        _ if bw <= 2.0 * H_PADDING || bh <= 2.0 * V_PADDING => None,
        _ => {
            let em = font.width(text)?;
            let by_height = (bh - 2.0 * V_PADDING) / LINE_FACTOR;
            let by_width = if em > 0.0 {
                (bw - 2.0 * H_PADDING) / em
            } else {
                f64::INFINITY
            };
            Some(floor2(by_height.min(by_width)))
        }
    })
}

/// `v` rounded down to two decimals.
fn floor2(v: f64) -> f64 {
    (v * 100.0).floor() / 100.0
}

/// The baseline offset above the text box bottom that centres a line's
/// ascent to descent at `size` (pdf.js `vShift`).
fn baseline(font: &FieldFont, size: f64, bh: f64) -> f64 {
    let (ascent, descent) = font.ascent_descent();
    (bh - (ascent + descent) * size) / 2.0 + descent * size
}

/// One line: `(size, "x y Td (…) Tj")`.
fn line_text(
    text: &str,
    font: &FieldFont,
    quadding: u8,
    text_box: [f64; 4],
) -> Result<Option<(f64, String)>, AppearanceError> {
    let codes = font.encode(text)?;
    let Some(size) = line_size(text, font, text_box)? else {
        return Ok(None);
    };
    let [x0, y0, bw, bh] = text_box;
    let width = font.width(text)? * size;
    let x = match quadding {
        1 => x0 + (bw - width) / 2.0,
        2 => x0 + bw - width - H_PADDING,
        _ => x0 + H_PADDING,
    };
    let y = y0 + baseline(font, size, bh);
    let shown = format!(
        "{} {} Td\n{} Tj\n",
        Number(x),
        Number(y),
        string_syntax(&codes)
    );
    Ok(Some((size, shown)))
}

/// A comb: each character centred in one of `max_len` cells across the text
/// box (pdf.js `_getCombAppearance`).
fn comb_text(
    text: &str,
    font: &FieldFont,
    quadding: u8,
    max_len: u32,
    text_box: [f64; 4],
) -> Result<Option<(f64, String)>, AppearanceError> {
    font.encode(text)?;
    let Some(size) = line_size(text, font, text_box)? else {
        return Ok(None);
    };
    let [x0, y0, bw, bh] = text_box;
    let cell = bw / f64::from(max_len);
    let chars: Vec<String> = text.chars().map(String::from).collect();
    let run = cell * chars.len() as f64;
    let shift = match quadding {
        1 => ((bw - run) / (2.0 * cell)).floor() * cell,
        2 => bw - run,
        _ => 0.0,
    };
    let mut shown = format!(
        "{} {} Td\n",
        Number(x0 + shift),
        Number(y0 + baseline(font, size, bh))
    );
    let mut previous = 0.0;
    for (i, c) in chars.iter().enumerate() {
        let width = font.width(c)? * size;
        let dx = if i == 0 {
            (cell - width) / 2.0
        } else {
            cell + (previous - width) / 2.0
        };
        let _ = write!(
            shown,
            "{} 0 Td\n{} Tj\n",
            Number(dx),
            string_syntax(&font.encode(c)?)
        );
        previous = width;
    }
    Ok(Some((size, shown)))
}

/// Several lines: the value split at line breaks, each wrapped to the text
/// box and placed top down (pdf.js `_getMultilineAppearance`).
fn multiline_text(
    text: &str,
    font: &FieldFont,
    quadding: u8,
    text_box: [f64; 4],
) -> Result<Option<(f64, String)>, AppearanceError> {
    let [x0, y0, bw, bh] = text_box;
    let lines: Vec<&str> = text
        .split("\r\n")
        .flat_map(|l| l.split(['\r', '\n']))
        .collect();
    for line in &lines {
        font.encode(line)?;
    }
    let height = bh - 2.0 * V_PADDING;
    let width = bw - 2.0 * H_PADDING;
    let max_width = bw - H_PADDING;
    let (size, line_height) = match font.size() {
        s if s > 0.0 => {
            let lines_fitting = (height / (s * LINE_FACTOR)).round().max(1.0);
            (s, height / lines_fitting)
        }
        _ if bw <= 2.0 * H_PADDING || bh <= 2.0 * V_PADDING => return Ok(None),
        _ => {
            let size_for = |n: f64| floor2(height / n / LINE_FACTOR);
            let too_big = |size: f64| -> Result<bool, AppearanceError> {
                let mut total = 0.0;
                for line in &lines {
                    total += split_line(line, font, size, width, max_width)?.len() as f64 * size;
                    if total > height {
                        return Ok(true);
                    }
                }
                Ok(false)
            };
            let mut n = (height / (12.0 * LINE_FACTOR))
                .round()
                .max(lines.len() as f64)
                .max(1.0);
            if height > 0.0 && too_big(size_for(n))? {
                let (mut low, mut high) = (n, 2.0 * n);
                while too_big(size_for(high))? {
                    low = high;
                    high *= 2.0;
                }
                while high - low > 1.0 {
                    let mid = ((low + high) / 2.0).floor();
                    if too_big(size_for(mid))? {
                        low = mid;
                    } else {
                        high = mid;
                    }
                }
                n = high;
            }
            (size_for(n), height / n)
        }
    };

    let v_padding = ((bh - size) / 2.0).floor().min(V_PADDING);
    let descent = match font.metrics() {
        Some((_, d)) => (BASELINE_FACTOR * line_height).max(d * size),
        None => BASELINE_FACTOR * line_height,
    };
    let mut shown = format!("{} {} Td\n", Number(x0), Number(y0 + bh));
    let mut previous_shift = 0.0;
    let mut first = true;
    for line in &lines {
        let chunks = split_line(line, font, size, width, max_width)?;
        let padding = if chunks.len() == 1 && font.width(line)? * size > width {
            H_PADDING / 2.0
        } else {
            H_PADDING
        };
        for chunk in chunks {
            let chunk_width = font.width(&chunk)? * size;
            let shift = match quadding {
                1 => (bw - chunk_width) / 2.0,
                2 => bw - chunk_width - padding,
                _ => padding,
            };
            let dy = if first {
                -v_padding - (line_height - descent)
            } else {
                -line_height
            };
            first = false;
            let _ = write!(
                shown,
                "{} {} Td\n{} Tj\n",
                Number(shift - previous_shift),
                Number(dy),
                string_syntax(&font.encode(&chunk)?)
            );
            previous_shift = shift;
        }
    }
    Ok(Some((size, shown)))
}

/// `line` broken into chunks no wider than `width` at `size` (pdf.js
/// `_splitLine`): after the last space before the overflow, else before the
/// overflowing character. A line within `max_width` stays whole.
fn split_line(
    line: &str,
    font: &FieldFont,
    size: f64,
    width: f64,
    max_width: f64,
) -> Result<Vec<String>, AppearanceError> {
    let chars: Vec<char> = line.chars().collect();
    if chars.len() <= 1 || font.width(line)? * size <= max_width {
        return Ok(vec![line.to_string()]);
    }
    let widths = chars
        .iter()
        .map(|c| Ok(font.width(&c.to_string())? * size))
        .collect::<Result<Vec<f64>, AppearanceError>>()?;
    let mut chunks = Vec::new();
    let mut start = 0;
    let mut current = 0.0;
    let mut last_space: Option<usize> = None;
    let mut i = 0;
    while i < chars.len() {
        let glyph = widths[i];
        if chars[i] == ' ' {
            if current + glyph > width {
                chunks.push(chars[start..i].iter().collect());
                start = i;
                current = glyph;
                last_space = None;
            } else {
                current += glyph;
                last_space = Some(i);
            }
        } else if current + glyph > width {
            if let Some(space) = last_space {
                chunks.push(chars[start..=space].iter().collect());
                start = space + 1;
                i = space + 1;
                last_space = None;
                current = 0.0;
            } else {
                chunks.push(chars[start..i].iter().collect());
                start = i;
                current = glyph;
            }
        } else {
            current += glyph;
        }
        i += 1;
    }
    if start < chars.len() {
        chunks.push(chars[start..].iter().collect());
    }
    Ok(chunks)
}

fn checkbox_appearance(field: &FormField, look: &WidgetLook, w: f64, h: f64) -> String {
    let mut buf = String::new();
    look.rect(&mut buf, w, h);

    // Checkmark if checked
    if field.is_checked() {
        buf.push_str("0 G\n1.5 w\n1 J\n");
        let _ = write!(
            buf,
            "{} {} m\n{} {} l\n{} {} l\nS\n",
            Number(w * 0.2),
            Number(h * 0.5),
            Number(w * 0.4),
            Number(h * 0.2),
            Number(w * 0.8),
            Number(h * 0.8),
        );
    }
    buf
}

fn radio_appearance(field: &FormField, look: &WidgetLook, w: f64, h: f64) -> String {
    let mut buf = String::new();
    let cx = w / 2.0;
    let cy = h / 2.0;
    let r = w.min(h) / 2.0 - look.width / 2.0;
    let k = CIRCLE_K;
    look.circle(&mut buf, cx, cy, r);

    // Filled dot if selected
    if field.is_checked() {
        let ir = r * 0.5;
        buf.push_str("0 g\n");
        append_circle(&mut buf, cx, cy, ir, k);
        buf.push_str("f\n");
    }
    buf
}

fn combo_appearance(
    field: &FormField,
    font: &FieldFont,
    look: &WidgetLook,
    w: f64,
    h: f64,
) -> Result<String, AppearanceError> {
    let mut buf = String::new();
    look.rect(&mut buf, w, h);

    // Dropdown arrow area
    let arrow_w = h.min(20.0);
    buf.push_str("0.9 g\n");
    let _ = write!(
        buf,
        "{} 0 {} {} re\nf\n",
        Number(w - arrow_w),
        Number(arrow_w),
        Number(h)
    );
    // Arrow triangle
    buf.push_str("0 g\n");
    let ax = w - arrow_w / 2.0;
    let _ = write!(
        buf,
        "{} {} m\n{} {} l\n{} {} l\nf\n",
        Number(ax - 3.0),
        Number(h * 0.6),
        Number(ax + 3.0),
        Number(h * 0.6),
        Number(ax),
        Number(h * 0.3),
    );

    let b = look.width;
    single_line(
        field,
        font,
        look,
        &buf,
        [b, b, w - arrow_w - 2.0 * b, h - 2.0 * b],
    )
}

fn list_appearance(
    field: &FormField,
    font: &FieldFont,
    look: &WidgetLook,
    w: f64,
    h: f64,
) -> Result<String, AppearanceError> {
    let mut buf = String::new();
    look.rect(&mut buf, w, h);

    // List items
    let line_height = 12.0;
    let selected = field.value_as_string().unwrap_or_default();
    let mut y = h - line_height;
    for opt in &field.options {
        if y < 0.0 {
            break;
        }
        // Highlight selected
        if *opt == selected {
            buf.push_str("0.6 0.75 1 rg\n");
            let _ = write!(
                buf,
                "1 {} {} {} re\nf\n",
                Number(y),
                Number(w - 2.0),
                Number(line_height)
            );
        }
        let _ = write!(buf, "BT\n0 g\n{}\n", font.select());
        let _ = writeln!(buf, "3 {} Td", Number(y + 2.0));
        let _ = write!(buf, "{} Tj\nET\n", string_syntax(&font.encode(opt)?));
        y -= line_height;
    }
    Ok(buf)
}

fn button_appearance(
    field: &FormField,
    font: &FieldFont,
    look: &WidgetLook,
    w: f64,
    h: f64,
) -> Result<String, AppearanceError> {
    let mut buf = String::new();
    look.rect(&mut buf, w, h);

    // Button caption
    if let Some(text) = field.value_as_string()
        && !text.is_empty()
    {
        let _ = write!(buf, "BT\n0 g\n{}\n", font.select());
        let _ = writeln!(buf, "4 {} Td", Number((h - 10.0) / 2.0));
        let _ = write!(buf, "{} Tj\nET\n", string_syntax(&font.encode(&text)?));
    }
    Ok(buf)
}

fn append_circle(buf: &mut String, cx: f64, cy: f64, r: f64, k: f64) {
    let _ = writeln!(buf, "{} {} m", Number(cx + r), Number(cy));
    let _ = writeln!(
        buf,
        "{} {} {} {} {} {} c",
        Number(cx + r),
        Number(cy + r * k),
        Number(cx + r * k),
        Number(cy + r),
        Number(cx),
        Number(cy + r)
    );
    let _ = writeln!(
        buf,
        "{} {} {} {} {} {} c",
        Number(cx - r * k),
        Number(cy + r),
        Number(cx - r),
        Number(cy + r * k),
        Number(cx - r),
        Number(cy)
    );
    let _ = writeln!(
        buf,
        "{} {} {} {} {} {} c",
        Number(cx - r),
        Number(cy - r * k),
        Number(cx - r * k),
        Number(cy - r),
        Number(cx),
        Number(cy - r)
    );
    let _ = writeln!(
        buf,
        "{} {} {} {} {} {} c",
        Number(cx + r * k),
        Number(cy - r),
        Number(cx + r),
        Number(cy - r * k),
        Number(cx + r),
        Number(cy)
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::object::IndirectRef;
    use crate::page::Rect;

    /// A black 1 pt border and no background.
    fn bordered() -> WidgetLook {
        WidgetLook {
            background: None,
            border: Some("0 G".to_string()),
            width: 1.0,
            quadding: None,
        }
    }

    /// Operators in `content` that come from a number written as `NaN` or `inf`.
    fn non_finite_operators(content: &[u8]) -> Vec<Vec<u8>> {
        crate::content::parse_content_stream(content)
            .unwrap()
            .into_iter()
            .map(|op| op.operator)
            .filter(|op| {
                op.starts_with(b"NaN") || op.starts_with(b"inf") || op.starts_with(b"-inf")
            })
            .collect()
    }

    #[test]
    fn test_non_finite_sizes_are_written_as_numbers() {
        let field = |field_type: FieldType, value: PdfObject| FormField {
            name: "f".to_string(),
            partial_name: "f".to_string(),
            field_type,
            value: Some(value),
            default_value: None,
            flags: FieldFlags::default(),
            options: vec!["one".to_string()],
            rect: None,
            default_appearance: None,
            quadding: 0,
            max_len: 0,
            field_ref: IndirectRef {
                obj_num: 1,
                gen_num: 0,
            },
            widgets: Vec::new(),
            page_obj_num: None,
        };
        let text = PdfObject::String(b"x".to_vec());
        let yes = PdfObject::Name(b"Yes".to_vec());
        let (w, h) = (f64::NAN, f64::INFINITY);
        let d = FieldFont::Default;
        let look = WidgetLook {
            background: Some("1 g".to_string()),
            ..bordered()
        };
        let contents = [
            text_field_appearance(&field(FieldType::Text, text.clone()), &d, &look, w, h).unwrap(),
            checkbox_appearance(&field(FieldType::Checkbox, yes.clone()), &look, w, h),
            radio_appearance(&field(FieldType::RadioButton, yes), &look, w, h),
            combo_appearance(&field(FieldType::ComboBox, text.clone()), &d, &look, w, h).unwrap(),
            list_appearance(&field(FieldType::ListBox, text.clone()), &d, &look, w, h).unwrap(),
            button_appearance(&field(FieldType::PushButton, text), &d, &look, w, h).unwrap(),
        ];
        for (i, content) in contents.iter().enumerate() {
            assert_eq!(
                non_finite_operators(content.as_bytes()),
                Vec::<Vec<u8>>::new(),
                "generator {i}: {content}"
            );
        }
    }

    #[test]
    fn test_text_field_appearance() {
        let field = FormField {
            name: "name".to_string(),
            partial_name: "name".to_string(),
            field_type: FieldType::Text,
            value: Some(PdfObject::String(b"Hello".to_vec())),
            default_value: None,
            flags: FieldFlags::default(),
            options: Vec::new(),
            rect: Some(Rect {
                llx: 0.0,
                lly: 0.0,
                urx: 200.0,
                ury: 20.0,
            }),
            default_appearance: None,
            quadding: 0,
            max_len: 0,
            field_ref: IndirectRef {
                obj_num: 1,
                gen_num: 0,
            },
            widgets: Vec::new(),
            page_obj_num: None,
        };
        let content =
            text_field_appearance(&field, &FieldFont::Default, &bordered(), 200.0, 20.0).unwrap();
        assert!(content.contains("re"));
        assert!(content.contains("(Hello)"));
        assert!(content.contains("Tj"));
    }

    #[test]
    fn test_checkbox_appearance_checked() {
        let field = FormField {
            name: "cb".to_string(),
            partial_name: "cb".to_string(),
            field_type: FieldType::Checkbox,
            value: Some(PdfObject::Name(b"Yes".to_vec())),
            default_value: None,
            flags: FieldFlags::default(),
            options: Vec::new(),
            rect: Some(Rect {
                llx: 0.0,
                lly: 0.0,
                urx: 14.0,
                ury: 14.0,
            }),
            default_appearance: None,
            quadding: 0,
            max_len: 0,
            field_ref: IndirectRef {
                obj_num: 1,
                gen_num: 0,
            },
            widgets: Vec::new(),
            page_obj_num: None,
        };
        let content = checkbox_appearance(&field, &bordered(), 14.0, 14.0);
        assert!(content.contains("m")); // checkmark path
        assert!(content.contains("l"));
        assert!(content.contains("S"));
    }

    #[test]
    fn test_checkbox_appearance_unchecked() {
        let field = FormField {
            name: "cb".to_string(),
            partial_name: "cb".to_string(),
            field_type: FieldType::Checkbox,
            value: Some(PdfObject::Name(b"Off".to_vec())),
            default_value: None,
            flags: FieldFlags::default(),
            options: Vec::new(),
            rect: Some(Rect {
                llx: 0.0,
                lly: 0.0,
                urx: 14.0,
                ury: 14.0,
            }),
            default_appearance: None,
            quadding: 0,
            max_len: 0,
            field_ref: IndirectRef {
                obj_num: 1,
                gen_num: 0,
            },
            widgets: Vec::new(),
            page_obj_num: None,
        };
        let content = checkbox_appearance(&field, &bordered(), 14.0, 14.0);
        // No checkmark lines — just the border rectangle
        assert!(content.contains("re"));
        assert!(!content.contains("1.5 w")); // no thick stroke for checkmark
    }

    #[test]
    fn test_text_value_reads_back_unchanged() {
        let text = "1) a\\b\r(c";
        let field = FormField {
            name: "name".to_string(),
            partial_name: "name".to_string(),
            field_type: FieldType::Text,
            value: Some(PdfObject::String(text.as_bytes().to_vec())),
            default_value: None,
            flags: FieldFlags::default(),
            options: Vec::new(),
            rect: Some(Rect {
                llx: 0.0,
                lly: 0.0,
                urx: 200.0,
                ury: 20.0,
            }),
            default_appearance: None,
            quadding: 0,
            max_len: 0,
            field_ref: IndirectRef {
                obj_num: 1,
                gen_num: 0,
            },
            widgets: Vec::new(),
            page_obj_num: None,
        };
        let content =
            text_field_appearance(&field, &FieldFont::Default, &bordered(), 200.0, 20.0).unwrap();

        let ops = crate::content::parse_content_stream(content.as_bytes()).unwrap();
        let tj = ops.iter().find(|op| op.operator == b"Tj").unwrap();
        assert_eq!(
            tj.operands,
            vec![crate::content::Operand::String(text.as_bytes().to_vec())]
        );
    }

    fn field(field_type: FieldType, value: &str, da: Option<&str>) -> FormField {
        FormField {
            name: "name".to_string(),
            partial_name: "name".to_string(),
            field_type,
            value: Some(PdfObject::String(value.as_bytes().to_vec())),
            default_value: None,
            flags: FieldFlags::default(),
            options: vec![value.to_string()],
            rect: Some(Rect {
                llx: 0.0,
                lly: 0.0,
                urx: 200.0,
                ury: 20.0,
            }),
            default_appearance: da.map(str::to_string),
            quadding: 0,
            max_len: 0,
            field_ref: IndirectRef {
                obj_num: 1,
                gen_num: 0,
            },
            widgets: Vec::new(),
            page_obj_num: None,
        }
    }

    /// The decoded content and dictionary of the appearance stream for `field`.
    fn appearance(field: &FormField) -> (Vec<u8>, crate::object::PdfDict) {
        let mut builder = crate::writer::document::DocumentBuilder::new();
        builder.add_page(crate::writer::page::PageBuilder::new(100.0, 100.0));
        let doc = crate::parser::PdfDocument::from_bytes(builder.build().unwrap()).unwrap();
        let mut modifier = DocumentModifier::from_document(&doc).unwrap();
        let ap = generate_field_appearance(field, &mut modifier).unwrap();
        let Some(PdfObject::Stream { dict, data }) = modifier.find_object_pub(ap.obj_num) else {
            panic!("expected an appearance stream")
        };
        (
            crate::stream::decode_stream(data, dict).unwrap(),
            dict.clone(),
        )
    }

    fn shown_strings(content: &[u8]) -> Vec<Vec<u8>> {
        crate::content::parse_content_stream(content)
            .unwrap()
            .into_iter()
            .filter(|op| op.operator == b"Tj")
            .flat_map(|op| op.operands)
            .filter_map(|o| match o {
                crate::content::Operand::String(s) => Some(s),
                _ => None,
            })
            .collect()
    }

    fn helvetica_encoding(dict: &crate::object::PdfDict) -> Option<Vec<u8>> {
        dict.get_dict(b"Resources")
            .and_then(|r| r.get_dict(b"Font"))
            .and_then(|f| f.get_dict(b"Helvetica"))
            .and_then(|h| h.get_name(b"Encoding"))
            .map(<[u8]>::to_vec)
    }

    #[test]
    fn default_font_fields_write_win_ansi_and_declare_the_font() {
        for field_type in [
            FieldType::Text,
            FieldType::ComboBox,
            FieldType::ListBox,
            FieldType::PushButton,
        ] {
            let (content, dict) = appearance(&field(field_type, "Müller", None));
            assert_eq!(
                shown_strings(&content),
                vec![b"M\xFCller".to_vec()],
                "{field_type:?}"
            );
            assert_eq!(
                helvetica_encoding(&dict).as_deref(),
                Some(b"WinAnsiEncoding".as_slice()),
                "{field_type:?}"
            );
        }
    }
}
