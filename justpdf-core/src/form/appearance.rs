use std::fmt::Write;

use crate::object::{IndirectRef, PdfObject};
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
    let _ = write!(buf, "0 0 {w} {h} re\nf\n");
    buf.push_str("0 G\n0.5 w\n");
    let _ = write!(buf, "0.5 0.5 {} {} re\nS\n", w - 1.0, h - 1.0);

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
            let _ = write!(buf, "2 {} Td\n", (h - 10.0) / 2.0);
            let escaped = escape_pdf_string(&text);
            let _ = write!(buf, "({escaped}) Tj\nET\n");
        }
    }
    buf
}

fn checkbox_appearance(field: &FormField, w: f64, h: f64) -> String {
    let mut buf = String::new();

    // Border
    buf.push_str("1 g\n");
    let _ = write!(buf, "0 0 {w} {h} re\nf\n");
    buf.push_str("0 G\n0.5 w\n");
    let _ = write!(buf, "0.5 0.5 {} {} re\nS\n", w - 1.0, h - 1.0);

    // Checkmark if checked
    if field.is_checked() {
        buf.push_str("0 G\n1.5 w\n1 J\n");
        let _ = write!(
            buf,
            "{} {} m\n{} {} l\n{} {} l\nS\n",
            w * 0.2, h * 0.5,
            w * 0.4, h * 0.2,
            w * 0.8, h * 0.8,
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
    let _ = write!(buf, "0 0 {w} {h} re\nf\n");
    buf.push_str("0 G\n0.5 w\n");
    let _ = write!(buf, "0.5 0.5 {} {} re\nS\n", w - 1.0, h - 1.0);

    // Dropdown arrow area
    let arrow_w = h.min(20.0);
    buf.push_str("0.9 g\n");
    let _ = write!(buf, "{} 0 {arrow_w} {h} re\nf\n", w - arrow_w);
    // Arrow triangle
    buf.push_str("0 g\n");
    let ax = w - arrow_w / 2.0;
    let _ = write!(
        buf,
        "{} {} m\n{} {} l\n{} {} l\nf\n",
        ax - 3.0, h * 0.6,
        ax + 3.0, h * 0.6,
        ax, h * 0.3,
    );

    // Selected value text
    if let Some(text) = field.value_as_string() {
        if !text.is_empty() {
            buf.push_str("BT\n0 g\n/Helvetica 10 Tf\n");
            let _ = write!(buf, "2 {} Td\n", (h - 10.0) / 2.0);
            let escaped = escape_pdf_string(&text);
            let _ = write!(buf, "({escaped}) Tj\nET\n");
        }
    }
    buf
}

fn list_appearance(field: &FormField, w: f64, h: f64) -> String {
    let mut buf = String::new();

    // Background + border
    buf.push_str("1 g\n");
    let _ = write!(buf, "0 0 {w} {h} re\nf\n");
    buf.push_str("0 G\n0.5 w\n");
    let _ = write!(buf, "0.5 0.5 {} {} re\nS\n", w - 1.0, h - 1.0);

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
            let _ = write!(buf, "1 {} {} {} re\nf\n", y, w - 2.0, line_height);
        }
        buf.push_str("BT\n0 g\n/Helvetica 10 Tf\n");
        let _ = write!(buf, "3 {} Td\n", y + 2.0);
        let escaped = escape_pdf_string(opt);
        let _ = write!(buf, "({escaped}) Tj\nET\n");
        y -= line_height;
    }
    buf
}

fn button_appearance(field: &FormField, w: f64, h: f64) -> String {
    let mut buf = String::new();

    // 3D button look
    buf.push_str("0.85 g\n");
    let _ = write!(buf, "0 0 {w} {h} re\nf\n");
    buf.push_str("1 G\n1 w\n");
    let _ = write!(buf, "0 0 m\n0 {h} l\n{w} {h} l\nS\n");
    buf.push_str("0.5 G\n");
    let _ = write!(buf, "{w} {h} m\n{w} 0 l\n0 0 l\nS\n");

    // Button caption
    if let Some(text) = field.value_as_string() {
        if !text.is_empty() {
            buf.push_str("BT\n0 g\n/Helvetica 10 Tf\n");
            let _ = write!(buf, "{} {} Td\n", 4.0, (h - 10.0) / 2.0);
            let escaped = escape_pdf_string(&text);
            let _ = write!(buf, "({escaped}) Tj\nET\n");
        }
    }
    buf
}

fn append_circle(buf: &mut String, cx: f64, cy: f64, r: f64, k: f64) {
    let _ = write!(buf, "{} {} m\n", cx + r, cy);
    let _ = write!(
        buf,
        "{} {} {} {} {} {} c\n",
        cx + r, cy + r * k,
        cx + r * k, cy + r,
        cx, cy + r
    );
    let _ = write!(
        buf,
        "{} {} {} {} {} {} c\n",
        cx - r * k, cy + r,
        cx - r, cy + r * k,
        cx - r, cy
    );
    let _ = write!(
        buf,
        "{} {} {} {} {} {} c\n",
        cx - r, cy - r * k,
        cx - r * k, cy - r,
        cx, cy - r
    );
    let _ = write!(
        buf,
        "{} {} {} {} {} {} c\n",
        cx + r * k, cy - r,
        cx + r, cy - r * k,
        cx + r, cy
    );
}

fn escape_pdf_string(s: &str) -> String {
    s.chars()
        .flat_map(|c| match c {
            '(' => vec!['\\', '('],
            ')' => vec!['\\', ')'],
            '\\' => vec!['\\', '\\'],
            _ => vec![c],
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::object::IndirectRef;
    use crate::page::Rect;

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
    fn test_escape_pdf_string() {
        assert_eq!(escape_pdf_string("Hello"), "Hello");
        assert_eq!(escape_pdf_string("A(B)C"), "A\\(B\\)C");
        assert_eq!(escape_pdf_string("a\\b"), "a\\\\b");
    }
}
