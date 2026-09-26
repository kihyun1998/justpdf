#![allow(clippy::write_with_newline)]

use std::fmt::Write;

use crate::error::Result;
use crate::object::{IndirectRef, Number, PdfDict, PdfObject, string_syntax};
use crate::page::Rect;
use crate::writer::encode::make_stream;
use crate::writer::modify::DocumentModifier;

use super::types::*;

/// Generate an appearance stream for an annotation dict.
/// Returns the indirect reference to the Form XObject, or None if not applicable.
pub fn generate_appearance(
    annot_dict: &PdfDict,
    modifier: &mut DocumentModifier,
) -> Result<Option<IndirectRef>> {
    let subtype = match annot_dict.get_name(b"Subtype") {
        Some(s) => s,
        None => return Ok(None),
    };
    let rect_arr = match annot_dict.get_array(b"Rect") {
        Some(a) => a,
        None => return Ok(None),
    };
    let rect = match Rect::from_pdf_array(rect_arr) {
        Some(r) => r,
        None => return Ok(None),
    };

    let color = annot_dict
        .get_array(b"C")
        .and_then(AnnotColor::from_array);
    let border_width = annot_dict
        .get_dict(b"BS")
        .and_then(|bs| bs.get(b"W"))
        .and_then(|o| o.as_f64())
        .unwrap_or(1.0);

    let content = match subtype {
        b"Highlight" => highlight_appearance(&rect, &color, annot_dict),
        b"Underline" => underline_appearance(&rect, &color, border_width, annot_dict),
        b"StrikeOut" => strikeout_appearance(&rect, &color, border_width, annot_dict),
        b"Squiggly" => squiggly_appearance(&rect, &color, border_width, annot_dict),
        b"Square" => square_appearance(&rect, &color, border_width, annot_dict),
        b"Circle" => circle_appearance(&rect, &color, border_width, annot_dict),
        b"Line" => line_appearance(&rect, &color, border_width, annot_dict),
        b"Ink" => ink_appearance(&rect, &color, border_width, annot_dict),
        b"Text" => text_note_appearance(&rect),
        b"Stamp" => stamp_appearance(&rect, annot_dict),
        b"Redact" => redact_appearance(&rect, annot_dict),
        _ => return Ok(None),
    };

    let content = match content {
        Some(c) => c,
        None => return Ok(None),
    };

    let w = rect.width();
    let h = rect.height();

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
    form_dict.insert(
        b"Matrix".to_vec(),
        PdfObject::Array(vec![
            PdfObject::Real(1.0),
            PdfObject::Real(0.0),
            PdfObject::Real(0.0),
            PdfObject::Real(1.0),
            PdfObject::Real(-rect.llx),
            PdfObject::Real(-rect.lly),
        ]),
    );

    let form_xobj = PdfObject::Stream {
        dict: form_dict,
        data: stream_data,
    };
    let ap_ref = modifier.add_object(form_xobj);
    Ok(Some(ap_ref))
}

fn set_stroke_color(buf: &mut String, color: &Option<AnnotColor>) {
    match color {
        Some(AnnotColor::Gray(g)) => { let _ = writeln!(buf, "{} G", Number(*g)); }
        Some(AnnotColor::Rgb(r, g, b)) => { let _ = writeln!(buf, "{} {} {} RG", Number(*r), Number(*g), Number(*b)); }
        Some(AnnotColor::Cmyk(c, m, y, k)) => { let _ = writeln!(buf, "{} {} {} {} K", Number(*c), Number(*m), Number(*y), Number(*k)); }
        None => { buf.push_str("0 G\n"); }
    }
}

fn set_fill_color(buf: &mut String, color: &Option<AnnotColor>) {
    match color {
        Some(AnnotColor::Gray(g)) => { let _ = writeln!(buf, "{} g", Number(*g)); }
        Some(AnnotColor::Rgb(r, g, b)) => { let _ = writeln!(buf, "{} {} {} rg", Number(*r), Number(*g), Number(*b)); }
        Some(AnnotColor::Cmyk(c, m, y, k)) => { let _ = writeln!(buf, "{} {} {} {} k", Number(*c), Number(*m), Number(*y), Number(*k)); }
        None => { buf.push_str("0 g\n"); }
    }
}

fn highlight_appearance(
    rect: &Rect,
    color: &Option<AnnotColor>,
    _dict: &PdfDict,
) -> Option<String> {
    let mut buf = String::new();
    set_fill_color(&mut buf, color);
    let _ = write!(
        buf,
        "{} {} {} {} re\nf\n",
        Number(rect.llx), Number(rect.lly), Number(rect.width()), Number(rect.height())
    );
    Some(buf)
}

fn underline_appearance(
    rect: &Rect,
    color: &Option<AnnotColor>,
    width: f64,
    _dict: &PdfDict,
) -> Option<String> {
    let mut buf = String::new();
    set_stroke_color(&mut buf, color);
    let _ = write!(buf, "{} w\n", Number(width));
    let _ = write!(
        buf,
        "{} {} m\n{} {} l\nS\n",
        Number(rect.llx),
        Number(rect.lly),
        Number(rect.urx),
        Number(rect.lly)
    );
    Some(buf)
}

fn strikeout_appearance(
    rect: &Rect,
    color: &Option<AnnotColor>,
    width: f64,
    _dict: &PdfDict,
) -> Option<String> {
    let mut buf = String::new();
    set_stroke_color(&mut buf, color);
    let _ = write!(buf, "{} w\n", Number(width));
    let mid_y = (rect.lly + rect.ury) / 2.0;
    let _ = write!(
        buf,
        "{} {} m\n{} {} l\nS\n",
        Number(rect.llx),
        Number(mid_y),
        Number(rect.urx),
        Number(mid_y)
    );
    Some(buf)
}

fn squiggly_appearance(
    rect: &Rect,
    color: &Option<AnnotColor>,
    width: f64,
    _dict: &PdfDict,
) -> Option<String> {
    let mut buf = String::new();
    set_stroke_color(&mut buf, color);
    let _ = write!(buf, "{} w\n", Number(width));

    // Simple zigzag at bottom
    let step = 4.0;
    let amp = 2.0;
    let y_base = rect.lly;
    let mut x = rect.llx;
    let _ = write!(buf, "{} {} m\n", Number(x), Number(y_base));
    let mut up = true;
    while x < rect.urx {
        x += step;
        let y = if up { y_base + amp } else { y_base };
        let _ = write!(buf, "{} {} l\n", Number(x), Number(y));
        up = !up;
    }
    buf.push_str("S\n");
    Some(buf)
}

fn square_appearance(
    rect: &Rect,
    color: &Option<AnnotColor>,
    width: f64,
    dict: &PdfDict,
) -> Option<String> {
    let mut buf = String::new();
    set_stroke_color(&mut buf, color);
    let _ = write!(buf, "{} w\n", Number(width));

    let ic = dict.get_array(b"IC").and_then(AnnotColor::from_array);
    let _ = write!(
        buf,
        "{} {} {} {} re\n",
        Number(rect.llx), Number(rect.lly), Number(rect.width()), Number(rect.height())
    );
    if ic.is_some() {
        set_fill_color(&mut buf, &ic);
        buf.push_str("B\n");
    } else {
        buf.push_str("S\n");
    }
    Some(buf)
}

fn circle_appearance(
    rect: &Rect,
    color: &Option<AnnotColor>,
    width: f64,
    dict: &PdfDict,
) -> Option<String> {
    let mut buf = String::new();
    set_stroke_color(&mut buf, color);
    let _ = write!(buf, "{} w\n", Number(width));

    let ic = dict.get_array(b"IC").and_then(AnnotColor::from_array);

    // Approximate ellipse with 4 bezier curves
    let cx = (rect.llx + rect.urx) / 2.0;
    let cy = (rect.lly + rect.ury) / 2.0;
    let rx = rect.width() / 2.0;
    let ry = rect.height() / 2.0;
    let k = 0.5522847498; // magic number for bezier circle approximation

    let _ = write!(buf, "{} {} m\n", Number(cx + rx), Number(cy));
    let _ = write!(
        buf,
        "{} {} {} {} {} {} c\n",
        Number(cx + rx), Number(cy + ry * k),
        Number(cx + rx * k), Number(cy + ry),
        Number(cx), Number(cy + ry)
    );
    let _ = write!(
        buf,
        "{} {} {} {} {} {} c\n",
        Number(cx - rx * k), Number(cy + ry),
        Number(cx - rx), Number(cy + ry * k),
        Number(cx - rx), Number(cy)
    );
    let _ = write!(
        buf,
        "{} {} {} {} {} {} c\n",
        Number(cx - rx), Number(cy - ry * k),
        Number(cx - rx * k), Number(cy - ry),
        Number(cx), Number(cy - ry)
    );
    let _ = write!(
        buf,
        "{} {} {} {} {} {} c\n",
        Number(cx + rx * k), Number(cy - ry),
        Number(cx + rx), Number(cy - ry * k),
        Number(cx + rx), Number(cy)
    );

    if ic.is_some() {
        set_fill_color(&mut buf, &ic);
        buf.push_str("B\n");
    } else {
        buf.push_str("S\n");
    }
    Some(buf)
}

fn line_appearance(
    _rect: &Rect,
    color: &Option<AnnotColor>,
    width: f64,
    dict: &PdfDict,
) -> Option<String> {
    let l = dict
        .get_array(b"L")
        .map(|arr| arr.iter().filter_map(|o| o.as_f64()).collect::<Vec<_>>())?;
    if l.len() < 4 {
        return None;
    }

    let mut buf = String::new();
    set_stroke_color(&mut buf, color);
    let _ = write!(buf, "{} w\n", Number(width));
    let _ = write!(
        buf,
        "{} {} m\n{} {} l\nS\n",
        Number(l[0]),
        Number(l[1]),
        Number(l[2]),
        Number(l[3])
    );
    Some(buf)
}

fn ink_appearance(
    _rect: &Rect,
    color: &Option<AnnotColor>,
    width: f64,
    dict: &PdfDict,
) -> Option<String> {
    let ink_list = dict.get_array(b"InkList")?;

    let mut buf = String::new();
    set_stroke_color(&mut buf, color);
    let _ = write!(buf, "{} w\n1 J\n", Number(width)); // round cap

    for stroke in ink_list {
        if let Some(coords) = stroke.as_array() {
            let points: Vec<f64> = coords.iter().filter_map(|o| o.as_f64()).collect();
            if points.len() >= 2 {
                let _ = write!(buf, "{} {} m\n", Number(points[0]), Number(points[1]));
                let mut i = 2;
                while i + 1 < points.len() {
                    let _ = write!(buf, "{} {} l\n", Number(points[i]), Number(points[i + 1]));
                    i += 2;
                }
                buf.push_str("S\n");
            }
        }
    }
    Some(buf)
}

fn text_note_appearance(rect: &Rect) -> Option<String> {
    // Simple note icon: yellow rectangle with fold
    let w = rect.width();
    let h = rect.height();
    let fold = (w.min(h) * 0.3).max(4.0);

    let mut buf = String::new();
    buf.push_str("1 1 0 rg\n"); // yellow fill
    let _ = write!(buf, "0 0 {} {} re\nf\n", Number(w), Number(h));
    buf.push_str("0 G\n0.5 w\n");
    let _ = write!(buf, "0 0 {} {} re\nS\n", Number(w), Number(h));
    // Fold triangle
    let _ = write!(
        buf,
        "{} {} m\n{} {} l\n{} {} l\nS\n",
        Number(w - fold),
        Number(h),
        Number(w),
        Number(h - fold),
        Number(w - fold),
        Number(h - fold)
    );
    Some(buf)
}

fn stamp_appearance(rect: &Rect, dict: &PdfDict) -> Option<String> {
    let icon_name = dict
        .get_name(b"Name")
        .map(|n| String::from_utf8_lossy(n).into_owned())
        .unwrap_or_else(|| "Draft".to_string());

    let w = rect.width();
    let h = rect.height();

    let mut buf = String::new();
    // Red border
    buf.push_str("1 0 0 RG\n2 w\n");
    let _ = write!(buf, "2 2 {} {} re\nS\n", Number(w - 4.0), Number(h - 4.0));
    // Red text (centered approximately)
    buf.push_str("1 0 0 rg\n");
    buf.push_str("BT\n/Helvetica 14 Tf\n");
    let _ = write!(buf, "8 {} Td\n", Number(h / 2.0 - 5.0));
    let _ = write!(buf, "{} Tj\nET\n", string_syntax(icon_name.as_bytes()));
    Some(buf)
}

fn redact_appearance(rect: &Rect, dict: &PdfDict) -> Option<String> {
    let ic = dict
        .get_array(b"IC")
        .and_then(AnnotColor::from_array)
        .unwrap_or(AnnotColor::Rgb(0.0, 0.0, 0.0));

    let mut buf = String::new();
    set_fill_color(&mut buf, &Some(ic));
    let _ = write!(
        buf,
        "{} {} {} {} re\nf\n",
        Number(rect.llx), Number(rect.lly), Number(rect.width()), Number(rect.height())
    );
    Some(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_non_finite_numbers_are_written_as_numbers() {
        let rect = Rect { llx: f64::NAN, lly: f64::INFINITY, urx: 10.0, ury: f64::NEG_INFINITY };
        let color = Some(AnnotColor::Rgb(f64::NAN, f64::INFINITY, 0.5));
        let w = f64::NAN;
        let nan = PdfObject::Real(f64::NAN);
        let inf = PdfObject::Real(f64::INFINITY);
        let mut dict = PdfDict::new();
        dict.insert(b"L".to_vec(), PdfObject::Array(vec![nan.clone(), inf.clone(), nan.clone(), inf.clone()]));
        dict.insert(
            b"InkList".to_vec(),
            PdfObject::Array(vec![PdfObject::Array(vec![nan.clone(), inf.clone(), nan, inf])]),
        );
        dict.insert(b"IC".to_vec(), PdfObject::Array(vec![PdfObject::Real(f64::NAN)]));
        let contents = [
            highlight_appearance(&rect, &color, &dict),
            underline_appearance(&rect, &color, w, &dict),
            strikeout_appearance(&rect, &color, w, &dict),
            squiggly_appearance(&rect, &color, w, &dict),
            square_appearance(&rect, &color, w, &dict),
            circle_appearance(&rect, &color, w, &dict),
            line_appearance(&rect, &color, w, &dict),
            ink_appearance(&rect, &color, w, &dict),
            text_note_appearance(&rect),
            stamp_appearance(&rect, &dict),
            redact_appearance(&rect, &dict),
        ];
        for (i, content) in contents.iter().enumerate() {
            let content = content.as_ref().unwrap();
            assert_eq!(non_finite_operators(content.as_bytes()), Vec::<Vec<u8>>::new(), "generator {i}: {content}");
        }
    }

    #[test]
    fn test_stamp_name_reads_back_unchanged() {
        let rect = Rect {
            llx: 0.0,
            lly: 0.0,
            urx: 100.0,
            ury: 40.0,
        };
        let mut dict = PdfDict::new();
        dict.insert(b"Name".to_vec(), PdfObject::Name(b"Draft)\r\\".to_vec()));
        let content = stamp_appearance(&rect, &dict).unwrap();

        let ops = crate::content::parse_content_stream(content.as_bytes()).unwrap();
        let tj = ops.iter().find(|op| op.operator == b"Tj").unwrap();
        assert_eq!(
            tj.operands,
            vec![crate::content::Operand::String(b"Draft)\r\\".to_vec())]
        );
    }

    #[test]
    fn test_highlight_content() {
        let rect = Rect {
            llx: 100.0,
            lly: 200.0,
            urx: 300.0,
            ury: 220.0,
        };
        let color = Some(AnnotColor::Rgb(1.0, 1.0, 0.0));
        let dict = PdfDict::new();
        let content = highlight_appearance(&rect, &color, &dict).unwrap();
        assert!(content.contains("1 1 0 rg"));
        assert!(content.contains("re"));
        assert!(content.contains("f"));
    }

    #[test]
    fn test_circle_content() {
        let rect = Rect {
            llx: 100.0,
            lly: 100.0,
            urx: 200.0,
            ury: 200.0,
        };
        let color = Some(AnnotColor::Rgb(1.0, 0.0, 0.0));
        let dict = PdfDict::new();
        let content = circle_appearance(&rect, &color, 1.0, &dict).unwrap();
        assert!(content.contains("c")); // bezier curves
        assert!(content.contains("S")); // stroke
    }

    #[test]
    fn test_ink_content() {
        let rect = Rect {
            llx: 0.0,
            lly: 0.0,
            urx: 100.0,
            ury: 100.0,
        };
        let mut dict = PdfDict::new();
        dict.insert(
            b"InkList".to_vec(),
            PdfObject::Array(vec![PdfObject::Array(vec![
                PdfObject::Real(10.0),
                PdfObject::Real(20.0),
                PdfObject::Real(30.0),
                PdfObject::Real(40.0),
            ])]),
        );
        let color = Some(AnnotColor::Rgb(0.0, 0.0, 1.0));
        let content = ink_appearance(&rect, &color, 2.0, &dict).unwrap();
        assert!(content.contains("10 20 m"));
        assert!(content.contains("30 40 l"));
    }
}
