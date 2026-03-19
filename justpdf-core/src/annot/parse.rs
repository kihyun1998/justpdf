use crate::error::Result;
use crate::object::{IndirectRef, PdfDict, PdfObject};
use crate::page::{collect_pages, PageInfo, Rect};
use crate::parser::PdfDocument;

use super::types::*;

/// Get all annotations from a specific page.
pub fn get_annotations(doc: &PdfDocument, page: &PageInfo) -> Result<Vec<Annotation>> {
    let page_obj = doc.resolve(&page.page_ref)?;
    let page_dict = match page_obj.as_dict() {
        Some(d) => d,
        None => return Ok(Vec::new()),
    };

    let annots_arr = match page_dict.get(b"Annots") {
        Some(PdfObject::Array(arr)) => arr.clone(),
        Some(PdfObject::Reference(r)) => {
            let resolved = doc.resolve(r)?;
            match resolved.as_array() {
                Some(arr) => arr.to_vec(),
                None => return Ok(Vec::new()),
            }
        }
        _ => return Ok(Vec::new()),
    };

    let mut annotations = Vec::new();
    for item in &annots_arr {
        let (annot_dict, obj_ref) = match item {
            PdfObject::Reference(r) => {
                let resolved = doc.resolve(r)?;
                match resolved {
                    PdfObject::Dict(d) => (d, Some(r.clone())),
                    _ => continue,
                }
            }
            PdfObject::Dict(d) => (d.clone(), None),
            _ => continue,
        };
        if let Some(annot) = parse_annotation_dict(&annot_dict, obj_ref.as_ref()) {
            annotations.push(annot);
        }
    }
    Ok(annotations)
}

/// Get all annotations from all pages.
pub fn get_all_annotations(doc: &PdfDocument) -> Result<Vec<(usize, Vec<Annotation>)>> {
    let pages = collect_pages(doc)?;
    let mut result = Vec::new();
    for page in &pages {
        let annots = get_annotations(doc, page)?;
        if !annots.is_empty() {
            result.push((page.index, annots));
        }
    }
    Ok(result)
}

/// Parse a single annotation dictionary.
fn parse_annotation_dict(dict: &PdfDict, obj_ref: Option<&IndirectRef>) -> Option<Annotation> {
    let subtype = dict.get_name(b"Subtype")?;
    let annot_type = AnnotationType::from_name(subtype);

    let rect_arr = dict.get_array(b"Rect")?;
    let rect = Rect::from_pdf_array(rect_arr)?;

    let contents = dict
        .get(b"Contents")
        .and_then(|o| o.as_str())
        .map(|b| String::from_utf8_lossy(b).into_owned());

    let name = dict
        .get(b"NM")
        .and_then(|o| o.as_str())
        .map(|b| String::from_utf8_lossy(b).into_owned());

    let modified = dict
        .get(b"M")
        .and_then(|o| o.as_str())
        .map(|b| String::from_utf8_lossy(b).into_owned());

    let flags = AnnotationFlags(
        dict.get_i64(b"F").unwrap_or(0) as u32,
    );

    let color = dict
        .get_array(b"C")
        .and_then(AnnotColor::from_array);

    let border = parse_border_style(dict);

    let appearance_ref = dict
        .get_dict(b"AP")
        .and_then(|ap| ap.get_ref(b"N"))
        .cloned();

    let popup_ref = dict.get_ref(b"Popup").cloned();

    let data = parse_annotation_data(dict, &annot_type);

    Some(Annotation {
        annot_type,
        rect,
        contents,
        name,
        modified,
        flags,
        color,
        border,
        appearance_ref,
        popup_ref,
        obj_ref: obj_ref.cloned(),
        data,
    })
}

/// Parse border style from /BS dict or /Border array.
fn parse_border_style(dict: &PdfDict) -> Option<BorderStyle> {
    if let Some(bs) = dict.get_dict(b"BS") {
        let width = bs
            .get(b"W")
            .and_then(|o| o.as_f64())
            .unwrap_or(1.0);
        let style = bs
            .get_name(b"S")
            .map(BorderStyleType::from_name)
            .unwrap_or(BorderStyleType::Solid);
        let dash_pattern = bs
            .get_array(b"D")
            .map(|arr| arr.iter().filter_map(|o| o.as_f64()).collect())
            .unwrap_or_default();
        return Some(BorderStyle {
            width,
            style,
            dash_pattern,
        });
    }

    if let Some(border_arr) = dict.get_array(b"Border")
        && border_arr.len() >= 3
    {
        let width = border_arr[2].as_f64().unwrap_or(1.0);
        let dash_pattern = if border_arr.len() > 3 {
            border_arr[3]
                .as_array()
                .map(|arr| arr.iter().filter_map(|o| o.as_f64()).collect())
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        return Some(BorderStyle {
            width,
            style: if dash_pattern.is_empty() {
                BorderStyleType::Solid
            } else {
                BorderStyleType::Dashed
            },
            dash_pattern,
        });
    }

    None
}

/// Parse type-specific annotation data.
fn parse_annotation_data(dict: &PdfDict, annot_type: &AnnotationType) -> AnnotationData {
    match annot_type {
        AnnotationType::Highlight
        | AnnotationType::Underline
        | AnnotationType::StrikeOut
        | AnnotationType::Squiggly => {
            let quad_points = dict
                .get_array(b"QuadPoints")
                .map(|arr| arr.iter().filter_map(|o| o.as_f64()).collect())
                .unwrap_or_default();
            AnnotationData::Markup { quad_points }
        }

        AnnotationType::Line => {
            let l = dict
                .get_array(b"L")
                .map(|arr| arr.iter().filter_map(|o| o.as_f64()).collect::<Vec<_>>())
                .unwrap_or_default();
            let start = if l.len() >= 2 { (l[0], l[1]) } else { (0.0, 0.0) };
            let end = if l.len() >= 4 { (l[2], l[3]) } else { (0.0, 0.0) };

            let line_endings = dict
                .get_array(b"LE")
                .map(|arr| {
                    let s = arr
                        .first()
                        .and_then(|o| o.as_name())
                        .map(LineEndingStyle::from_name)
                        .unwrap_or(LineEndingStyle::None);
                    let e = arr
                        .get(1)
                        .and_then(|o| o.as_name())
                        .map(LineEndingStyle::from_name)
                        .unwrap_or(LineEndingStyle::None);
                    (s, e)
                })
                .unwrap_or((LineEndingStyle::None, LineEndingStyle::None));

            let leader_line_length = dict
                .get(b"LL")
                .and_then(|o| o.as_f64())
                .unwrap_or(0.0);
            let leader_line_extension = dict
                .get(b"LLE")
                .and_then(|o| o.as_f64())
                .unwrap_or(0.0);
            let caption = dict
                .get(b"Cap")
                .and_then(|o| o.as_bool())
                .unwrap_or(false);
            let interior_color = dict
                .get_array(b"IC")
                .and_then(AnnotColor::from_array);

            AnnotationData::Line {
                start,
                end,
                line_endings,
                leader_line_length,
                leader_line_extension,
                caption,
                interior_color,
            }
        }

        AnnotationType::Ink => {
            let ink_list = dict
                .get_array(b"InkList")
                .map(|arr| {
                    arr.iter()
                        .filter_map(|item| {
                            item.as_array().map(|coords| {
                                coords
                                    .chunks(2)
                                    .filter_map(|pair| {
                                        if pair.len() == 2 {
                                            Some((pair[0].as_f64()?, pair[1].as_f64()?))
                                        } else {
                                            None
                                        }
                                    })
                                    .collect()
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();
            AnnotationData::Ink { ink_list }
        }

        AnnotationType::Link => {
            let uri = dict
                .get_dict(b"A")
                .and_then(|a| a.get(b"URI"))
                .and_then(|o| o.as_str())
                .map(|b| String::from_utf8_lossy(b).into_owned());
            let dest = dict.get(b"Dest").cloned();
            AnnotationData::Link { uri, dest }
        }

        AnnotationType::FreeText => {
            let da = dict
                .get(b"DA")
                .and_then(|o| o.as_str())
                .map(|b| String::from_utf8_lossy(b).into_owned())
                .unwrap_or_default();
            let justification = dict.get_i64(b"Q").unwrap_or(0);
            AnnotationData::FreeText { da, justification }
        }

        AnnotationType::FileAttachment => {
            let fs_ref = dict.get_ref(b"FS").cloned();
            let icon_name = dict
                .get_name(b"Name")
                .map(|n| String::from_utf8_lossy(n).into_owned())
                .unwrap_or_else(|| "PushPin".to_string());
            AnnotationData::FileAttachment { fs_ref, icon_name }
        }

        AnnotationType::Stamp => {
            let icon_name = dict
                .get_name(b"Name")
                .map(|n| String::from_utf8_lossy(n).into_owned())
                .unwrap_or_else(|| "Draft".to_string());
            AnnotationData::Stamp { icon_name }
        }

        AnnotationType::Square
        | AnnotationType::Circle
        | AnnotationType::Polygon
        | AnnotationType::PolyLine => {
            let vertices = dict
                .get_array(b"Vertices")
                .map(|arr| {
                    arr.chunks(2)
                        .filter_map(|pair| {
                            if pair.len() == 2 {
                                Some((pair[0].as_f64()?, pair[1].as_f64()?))
                            } else {
                                None
                            }
                        })
                        .collect()
                })
                .unwrap_or_default();
            let interior_color = dict
                .get_array(b"IC")
                .and_then(AnnotColor::from_array);
            AnnotationData::Shape {
                vertices,
                interior_color,
            }
        }

        AnnotationType::Redact => {
            let overlay_text = dict
                .get(b"OverlayText")
                .and_then(|o| o.as_str())
                .map(|b| String::from_utf8_lossy(b).into_owned());
            let repeat = dict
                .get(b"Repeat")
                .and_then(|o| o.as_bool())
                .unwrap_or(false);
            let interior_color = dict
                .get_array(b"IC")
                .and_then(AnnotColor::from_array);
            AnnotationData::Redact {
                overlay_text,
                repeat,
                interior_color,
            }
        }

        _ => AnnotationData::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_highlight_annotation() {
        let mut dict = PdfDict::new();
        dict.insert(b"Type".to_vec(), PdfObject::Name(b"Annot".to_vec()));
        dict.insert(b"Subtype".to_vec(), PdfObject::Name(b"Highlight".to_vec()));
        dict.insert(
            b"Rect".to_vec(),
            PdfObject::Array(vec![
                PdfObject::Real(100.0),
                PdfObject::Real(200.0),
                PdfObject::Real(300.0),
                PdfObject::Real(220.0),
            ]),
        );
        dict.insert(
            b"C".to_vec(),
            PdfObject::Array(vec![
                PdfObject::Real(1.0),
                PdfObject::Real(1.0),
                PdfObject::Real(0.0),
            ]),
        );
        dict.insert(
            b"QuadPoints".to_vec(),
            PdfObject::Array(vec![
                PdfObject::Real(100.0),
                PdfObject::Real(220.0),
                PdfObject::Real(300.0),
                PdfObject::Real(220.0),
                PdfObject::Real(100.0),
                PdfObject::Real(200.0),
                PdfObject::Real(300.0),
                PdfObject::Real(200.0),
            ]),
        );
        dict.insert(
            b"Contents".to_vec(),
            PdfObject::String(b"Test highlight".to_vec()),
        );

        let annot = parse_annotation_dict(&dict, None).unwrap();
        assert_eq!(annot.annot_type, AnnotationType::Highlight);
        assert_eq!(annot.rect.llx, 100.0);
        assert_eq!(annot.contents, Some("Test highlight".to_string()));
        assert_eq!(annot.color, Some(AnnotColor::Rgb(1.0, 1.0, 0.0)));
        if let AnnotationData::Markup { quad_points } = &annot.data {
            assert_eq!(quad_points.len(), 8);
        } else {
            panic!("expected Markup data");
        }
    }

    #[test]
    fn test_parse_link_annotation() {
        let mut dict = PdfDict::new();
        dict.insert(b"Subtype".to_vec(), PdfObject::Name(b"Link".to_vec()));
        dict.insert(
            b"Rect".to_vec(),
            PdfObject::Array(vec![
                PdfObject::Integer(72),
                PdfObject::Integer(700),
                PdfObject::Integer(200),
                PdfObject::Integer(720),
            ]),
        );
        let mut action = PdfDict::new();
        action.insert(b"S".to_vec(), PdfObject::Name(b"URI".to_vec()));
        action.insert(
            b"URI".to_vec(),
            PdfObject::String(b"https://example.com".to_vec()),
        );
        dict.insert(b"A".to_vec(), PdfObject::Dict(action));

        let annot = parse_annotation_dict(&dict, None).unwrap();
        assert_eq!(annot.annot_type, AnnotationType::Link);
        if let AnnotationData::Link { uri, .. } = &annot.data {
            assert_eq!(uri.as_deref(), Some("https://example.com"));
        } else {
            panic!("expected Link data");
        }
    }

    #[test]
    fn test_parse_ink_annotation() {
        let mut dict = PdfDict::new();
        dict.insert(b"Subtype".to_vec(), PdfObject::Name(b"Ink".to_vec()));
        dict.insert(
            b"Rect".to_vec(),
            PdfObject::Array(vec![
                PdfObject::Integer(0),
                PdfObject::Integer(0),
                PdfObject::Integer(100),
                PdfObject::Integer(100),
            ]),
        );
        dict.insert(
            b"InkList".to_vec(),
            PdfObject::Array(vec![PdfObject::Array(vec![
                PdfObject::Real(10.0),
                PdfObject::Real(20.0),
                PdfObject::Real(30.0),
                PdfObject::Real(40.0),
                PdfObject::Real(50.0),
                PdfObject::Real(60.0),
            ])]),
        );

        let annot = parse_annotation_dict(&dict, None).unwrap();
        if let AnnotationData::Ink { ink_list } = &annot.data {
            assert_eq!(ink_list.len(), 1);
            assert_eq!(ink_list[0].len(), 3);
            assert_eq!(ink_list[0][0], (10.0, 20.0));
            assert_eq!(ink_list[0][2], (50.0, 60.0));
        } else {
            panic!("expected Ink data");
        }
    }

    #[test]
    fn test_parse_line_annotation() {
        let mut dict = PdfDict::new();
        dict.insert(b"Subtype".to_vec(), PdfObject::Name(b"Line".to_vec()));
        dict.insert(
            b"Rect".to_vec(),
            PdfObject::Array(vec![
                PdfObject::Integer(100),
                PdfObject::Integer(100),
                PdfObject::Integer(300),
                PdfObject::Integer(300),
            ]),
        );
        dict.insert(
            b"L".to_vec(),
            PdfObject::Array(vec![
                PdfObject::Real(100.0),
                PdfObject::Real(100.0),
                PdfObject::Real(300.0),
                PdfObject::Real(300.0),
            ]),
        );
        dict.insert(
            b"LE".to_vec(),
            PdfObject::Array(vec![
                PdfObject::Name(b"OpenArrow".to_vec()),
                PdfObject::Name(b"ClosedArrow".to_vec()),
            ]),
        );
        dict.insert(b"LL".to_vec(), PdfObject::Real(10.0));

        let annot = parse_annotation_dict(&dict, None).unwrap();
        if let AnnotationData::Line {
            start,
            end,
            line_endings,
            leader_line_length,
            ..
        } = &annot.data
        {
            assert_eq!(*start, (100.0, 100.0));
            assert_eq!(*end, (300.0, 300.0));
            assert_eq!(line_endings.0, LineEndingStyle::OpenArrow);
            assert_eq!(line_endings.1, LineEndingStyle::ClosedArrow);
            assert_eq!(*leader_line_length, 10.0);
        } else {
            panic!("expected Line data");
        }
    }

    #[test]
    fn test_parse_missing_subtype() {
        let mut dict = PdfDict::new();
        dict.insert(
            b"Rect".to_vec(),
            PdfObject::Array(vec![
                PdfObject::Integer(0),
                PdfObject::Integer(0),
                PdfObject::Integer(100),
                PdfObject::Integer(100),
            ]),
        );
        assert!(parse_annotation_dict(&dict, None).is_none());
    }

    #[test]
    fn test_parse_missing_rect() {
        let mut dict = PdfDict::new();
        dict.insert(b"Subtype".to_vec(), PdfObject::Name(b"Text".to_vec()));
        assert!(parse_annotation_dict(&dict, None).is_none());
    }

    #[test]
    fn test_parse_border_style() {
        let mut dict = PdfDict::new();
        dict.insert(b"Subtype".to_vec(), PdfObject::Name(b"Text".to_vec()));
        dict.insert(
            b"Rect".to_vec(),
            PdfObject::Array(vec![
                PdfObject::Integer(0),
                PdfObject::Integer(0),
                PdfObject::Integer(100),
                PdfObject::Integer(100),
            ]),
        );
        let mut bs = PdfDict::new();
        bs.insert(b"W".to_vec(), PdfObject::Real(2.0));
        bs.insert(b"S".to_vec(), PdfObject::Name(b"D".to_vec()));
        bs.insert(
            b"D".to_vec(),
            PdfObject::Array(vec![PdfObject::Integer(3), PdfObject::Integer(1)]),
        );
        dict.insert(b"BS".to_vec(), PdfObject::Dict(bs));

        let annot = parse_annotation_dict(&dict, None).unwrap();
        let border = annot.border.unwrap();
        assert_eq!(border.width, 2.0);
        assert_eq!(border.style, BorderStyleType::Dashed);
        assert_eq!(border.dash_pattern, vec![3.0, 1.0]);
    }
}
