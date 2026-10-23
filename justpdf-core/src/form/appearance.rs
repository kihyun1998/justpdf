use std::fmt::Write;

use crate::content::Operand;
use crate::font::{FontEncoder, encode_winansi};
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
    let name = crate::content::parse_content_stream(da.as_bytes())
        .ok()?
        .into_iter()
        .rev()
        .find(|op| op.operator == b"Tf")
        .and_then(|op| match op.operands.into_iter().next() {
            Some(Operand::Name(n)) => Some(n),
            _ => None,
        })?;
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
    let mut buf = String::new();
    look.rect(&mut buf, w, h);

    // Text value
    if let Some(text) = field.value_as_string()
        && !text.is_empty()
    {
        let _ = write!(buf, "BT\n0 g\n{}\n", font.select());
        let _ = writeln!(buf, "2 {} Td", Number((h - 10.0) / 2.0));
        let _ = write!(buf, "{} Tj\nET\n", string_syntax(&font.encode(&text)?));
    }
    Ok(buf)
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

    // Selected value text
    if let Some(text) = field.value_as_string()
        && !text.is_empty()
    {
        let _ = write!(buf, "BT\n0 g\n{}\n", font.select());
        let _ = writeln!(buf, "2 {} Td", Number((h - 10.0) / 2.0));
        let _ = write!(buf, "{} Tj\nET\n", string_syntax(&font.encode(&text)?));
    }
    Ok(buf)
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
