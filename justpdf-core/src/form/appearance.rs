use std::fmt::Write;

use crate::object::{IndirectRef, Number, PdfObject, string_syntax};
use crate::writer::encode::make_stream;
use crate::writer::modify::DocumentModifier;

use super::types::*;

/// Generate an appearance stream for a form field widget.
/// Returns the indirect reference to the Form XObject.
pub fn generate_field_appearance(
    field: &FormField,
    modifier: &mut DocumentModifier,
) -> Option<IndirectRef> {
    let rect = field.rect?;
    let w = rect.width();
    let h = rect.height();
    if w <= 0.0 || h <= 0.0 {
        return None;
    }

    let content = match field.field_type {
        FieldType::Text => text_field_appearance(field, w, h),
        FieldType::Checkbox => checkbox_appearance(field, w, h),
        FieldType::RadioButton => radio_appearance(field, w, h),
        FieldType::ComboBox => combo_appearance(field, w, h),
        FieldType::ListBox => list_appearance(field, w, h),
        FieldType::PushButton => button_appearance(field, w, h),
        FieldType::Signature => return None, // Signatures have their own appearance
    };

    let (stream_dict, stream_data) = make_stream(content.as_bytes(), true);
    let mut form_dict = stream_dict;
    form_dict.insert(b"Type".to_vec(), PdfObject::Name(b"XObject".to_vec()));
    form_dict.insert(b"Subtype".to_vec(), PdfObject::Name(b"Form".to_vec()));
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
    Some(modifier.add_object(form_xobj))
}

fn text_field_appearance(field: &FormField, w: f64, h: f64) -> String {
    let mut buf = String::new();

    // Border
    buf.push_str("0.75 g\n");
    let _ = write!(buf, "0 0 {} {} re\nf\n", Number(w), Number(h));
    buf.push_str("0 G\n0.5 w\n");
    let _ = write!(buf, "0.5 0.5 {} {} re\nS\n", Number(w - 1.0), Number(h - 1.0));

    // Text value
    if let Some(text) = field.value_as_string() {
        if !text.is_empty() {
            // Use DA if present, otherwise default
            if let Some(da) = &field.default_appearance {
                let _ = write!(buf, "BT\n{da}\n");
            } else {
                buf.push_str("BT\n/Helvetica 10 Tf\n");
            }
            buf.push_str("0 g\n");
            let _ = write!(buf, "2 {} Td\n", Number((h - 10.0) / 2.0));
            let _ = write!(buf, "{} Tj\nET\n", string_syntax(text.as_bytes()));
        }
    }
    buf
}

fn checkbox_appearance(field: &FormField, w: f64, h: f64) -> String {
    let mut buf = String::new();

    // Border
    buf.push_str("1 g\n");
    let _ = write!(buf, "0 0 {} {} re\nf\n", Number(w), Number(h));
    buf.push_str("0 G\n0.5 w\n");
    let _ = write!(buf, "0.5 0.5 {} {} re\nS\n", Number(w - 1.0), Number(h - 1.0));

    // Checkmark if checked
    if field.is_checked() {
        buf.push_str("0 G\n1.5 w\n1 J\n");
        let _ = write!(
            buf,
            "{} {} m\n{} {} l\n{} {} l\nS\n",
            Number(w * 0.2), Number(h * 0.5),
            Number(w * 0.4), Number(h * 0.2),
            Number(w * 0.8), Number(h * 0.8),
        );
    }
    buf
}

fn radio_appearance(field: &FormField, w: f64, h: f64) -> String {
    let mut buf = String::new();
    let cx = w / 2.0;
    let cy = h / 2.0;
    let r = (w.min(h) / 2.0) - 1.0;
    let k = 0.5522847498;

    // Circle border
    buf.push_str("1 g\n");
    append_circle(&mut buf, cx, cy, r, k);
    buf.push_str("f\n");
    buf.push_str("0 G\n0.5 w\n");
    append_circle(&mut buf, cx, cy, r, k);
    buf.push_str("S\n");

    // Filled dot if selected
    if field.is_checked() {
        let ir = r * 0.5;
        buf.push_str("0 g\n");
        append_circle(&mut buf, cx, cy, ir, k);
        buf.push_str("f\n");
    }
    buf
}

fn combo_appearance(field: &FormField, w: f64, h: f64) -> String {
    let mut buf = String::new();

    // Background + border
    buf.push_str("1 g\n");
    let _ = write!(buf, "0 0 {} {} re\nf\n", Number(w), Number(h));
    buf.push_str("0 G\n0.5 w\n");
    let _ = write!(buf, "0.5 0.5 {} {} re\nS\n", Number(w - 1.0), Number(h - 1.0));

    // Dropdown arrow area
    let arrow_w = h.min(20.0);
    buf.push_str("0.9 g\n");
    let _ = write!(buf, "{} 0 {} {} re\nf\n", Number(w - arrow_w), Number(arrow_w), Number(h));
    // Arrow triangle
    buf.push_str("0 g\n");
    let ax = w - arrow_w / 2.0;
    let _ = write!(
        buf,
        "{} {} m\n{} {} l\n{} {} l\nf\n",
        Number(ax - 3.0), Number(h * 0.6),
        Number(ax + 3.0), Number(h * 0.6),
        Number(ax), Number(h * 0.3),
    );

    // Selected value text
    if let Some(text) = field.value_as_string() {
        if !text.is_empty() {
            buf.push_str("BT\n0 g\n/Helvetica 10 Tf\n");
            let _ = write!(buf, "2 {} Td\n", Number((h - 10.0) / 2.0));
            let _ = write!(buf, "{} Tj\nET\n", string_syntax(text.as_bytes()));
        }
    }
    buf
}

fn list_appearance(field: &FormField, w: f64, h: f64) -> String {
    let mut buf = String::new();

    // Background + border
    buf.push_str("1 g\n");
    let _ = write!(buf, "0 0 {} {} re\nf\n", Number(w), Number(h));
    buf.push_str("0 G\n0.5 w\n");
    let _ = write!(buf, "0.5 0.5 {} {} re\nS\n", Number(w - 1.0), Number(h - 1.0));

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
            let _ = write!(buf, "1 {} {} {} re\nf\n", Number(y), Number(w - 2.0), Number(line_height));
        }
        buf.push_str("BT\n0 g\n/Helvetica 10 Tf\n");
        let _ = write!(buf, "3 {} Td\n", Number(y + 2.0));
        let _ = write!(buf, "{} Tj\nET\n", string_syntax(opt.as_bytes()));
        y -= line_height;
    }
    buf
}

fn button_appearance(field: &FormField, w: f64, h: f64) -> String {
    let mut buf = String::new();

    // 3D button look
    buf.push_str("0.85 g\n");
    let _ = write!(buf, "0 0 {} {} re\nf\n", Number(w), Number(h));
    buf.push_str("1 G\n1 w\n");
    let _ = write!(buf, "0 0 m\n0 {} l\n{} {} l\nS\n", Number(h), Number(w), Number(h));
    buf.push_str("0.5 G\n");
    let _ = write!(buf, "{} {} m\n{} 0 l\n0 0 l\nS\n", Number(w), Number(h), Number(w));

    // Button caption
    if let Some(text) = field.value_as_string() {
        if !text.is_empty() {
            buf.push_str("BT\n0 g\n/Helvetica 10 Tf\n");
            let _ = write!(buf, "4 {} Td\n", Number((h - 10.0) / 2.0));
            let _ = write!(buf, "{} Tj\nET\n", string_syntax(text.as_bytes()));
        }
    }
    buf
}

fn append_circle(buf: &mut String, cx: f64, cy: f64, r: f64, k: f64) {
    let _ = write!(buf, "{} {} m\n", Number(cx + r), Number(cy));
    let _ = write!(
        buf,
        "{} {} {} {} {} {} c\n",
        Number(cx + r), Number(cy + r * k),
        Number(cx + r * k), Number(cy + r),
        Number(cx), Number(cy + r)
    );
    let _ = write!(
        buf,
        "{} {} {} {} {} {} c\n",
        Number(cx - r * k), Number(cy + r),
        Number(cx - r), Number(cy + r * k),
        Number(cx - r), Number(cy)
    );
    let _ = write!(
        buf,
        "{} {} {} {} {} {} c\n",
        Number(cx - r), Number(cy - r * k),
        Number(cx - r * k), Number(cy - r),
        Number(cx), Number(cy - r)
    );
    let _ = write!(
        buf,
        "{} {} {} {} {} {} c\n",
        Number(cx + r * k), Number(cy - r),
        Number(cx + r), Number(cy - r * k),
        Number(cx + r), Number(cy)
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::object::IndirectRef;
    use crate::page::Rect;

    /// Operators in `content` that come from a number written as `NaN` or `inf`.
    fn non_finite_operators(content: &[u8]) -> Vec<Vec<u8>> {
        crate::content::parse_content_stream(content)
            .unwrap()
            .into_iter()
            .map(|op| op.operator)
            .filter(|op| op.starts_with(b"NaN") || op.starts_with(b"inf") || op.starts_with(b"-inf"))
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
            field_ref: IndirectRef { obj_num: 1, gen_num: 0 },
            page_obj_num: None,
        };
        let text = PdfObject::String(b"x".to_vec());
        let yes = PdfObject::Name(b"Yes".to_vec());
        let (w, h) = (f64::NAN, f64::INFINITY);
        let contents = [
            text_field_appearance(&field(FieldType::Text, text.clone()), w, h),
            checkbox_appearance(&field(FieldType::Checkbox, yes.clone()), w, h),
            radio_appearance(&field(FieldType::RadioButton, yes), w, h),
            combo_appearance(&field(FieldType::ComboBox, text.clone()), w, h),
            list_appearance(&field(FieldType::ListBox, text.clone()), w, h),
            button_appearance(&field(FieldType::PushButton, text), w, h),
        ];
        for (i, content) in contents.iter().enumerate() {
            assert_eq!(non_finite_operators(content.as_bytes()), Vec::<Vec<u8>>::new(), "generator {i}: {content}");
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
            rect: Some(Rect { llx: 0.0, lly: 0.0, urx: 200.0, ury: 20.0 }),
            default_appearance: None,
            field_ref: IndirectRef { obj_num: 1, gen_num: 0 },
            page_obj_num: None,
        };
        let content = text_field_appearance(&field, 200.0, 20.0);
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
            rect: Some(Rect { llx: 0.0, lly: 0.0, urx: 14.0, ury: 14.0 }),
            default_appearance: None,
            field_ref: IndirectRef { obj_num: 1, gen_num: 0 },
            page_obj_num: None,
        };
        let content = checkbox_appearance(&field, 14.0, 14.0);
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
            rect: Some(Rect { llx: 0.0, lly: 0.0, urx: 14.0, ury: 14.0 }),
            default_appearance: None,
            field_ref: IndirectRef { obj_num: 1, gen_num: 0 },
            page_obj_num: None,
        };
        let content = checkbox_appearance(&field, 14.0, 14.0);
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
            page_obj_num: None,
        };
        let content = text_field_appearance(&field, 200.0, 20.0);

        let ops = crate::content::parse_content_stream(content.as_bytes()).unwrap();
        let tj = ops.iter().find(|op| op.operator == b"Tj").unwrap();
        assert_eq!(
            tj.operands,
            vec![crate::content::Operand::String(text.as_bytes().to_vec())]
        );
    }
}
