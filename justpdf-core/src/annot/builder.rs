use crate::error::{JustPdfError, Result};
use crate::object::{IndirectRef, PdfDict, PdfObject};
use crate::page::Rect;
use crate::writer::modify::DocumentModifier;

use super::types::*;

/// Builder for creating PDF annotations.
pub struct AnnotationBuilder {
    annot_type: AnnotationType,
    rect: Rect,
    contents: Option<String>,
    color: Option<AnnotColor>,
    border: Option<BorderStyle>,
    flags: AnnotationFlags,
    data: AnnotationData,
}

impl AnnotationBuilder {
    fn new(annot_type: AnnotationType, rect: Rect, data: AnnotationData) -> Self {
        Self {
            annot_type,
            rect,
            contents: None,
            color: None,
            border: None,
            flags: AnnotationFlags(AnnotationFlags::PRINT),
            data,
        }
    }

    // --- Constructors for specific annotation types ---

    pub fn highlight(rect: Rect, quad_points: Vec<f64>, color: AnnotColor) -> Self {
        let mut b = Self::new(
            AnnotationType::Highlight,
            rect,
            AnnotationData::Markup { quad_points },
        );
        b.color = Some(color);
        b
    }

    pub fn underline(rect: Rect, quad_points: Vec<f64>, color: AnnotColor) -> Self {
        let mut b = Self::new(
            AnnotationType::Underline,
            rect,
            AnnotationData::Markup { quad_points },
        );
        b.color = Some(color);
        b
    }

    pub fn strike_out(rect: Rect, quad_points: Vec<f64>, color: AnnotColor) -> Self {
        let mut b = Self::new(
            AnnotationType::StrikeOut,
            rect,
            AnnotationData::Markup { quad_points },
        );
        b.color = Some(color);
        b
    }

    pub fn squiggly(rect: Rect, quad_points: Vec<f64>, color: AnnotColor) -> Self {
        let mut b = Self::new(
            AnnotationType::Squiggly,
            rect,
            AnnotationData::Markup { quad_points },
        );
        b.color = Some(color);
        b
    }

    pub fn text(rect: Rect, contents: &str) -> Self {
        let mut b = Self::new(AnnotationType::Text, rect, AnnotationData::None);
        b.contents = Some(contents.to_string());
        b
    }

    pub fn free_text(rect: Rect, text: &str, da: &str) -> Self {
        let mut b = Self::new(
            AnnotationType::FreeText,
            rect,
            AnnotationData::FreeText {
                da: da.to_string(),
                justification: 0,
            },
        );
        b.contents = Some(text.to_string());
        b
    }

    pub fn line(start: (f64, f64), end: (f64, f64)) -> Self {
        let rect = Rect {
            llx: start.0.min(end.0),
            lly: start.1.min(end.1),
            urx: start.0.max(end.0),
            ury: start.1.max(end.1),
        };
        Self::new(
            AnnotationType::Line,
            rect,
            AnnotationData::Line {
                start,
                end,
                line_endings: (LineEndingStyle::None, LineEndingStyle::None),
                leader_line_length: 0.0,
                leader_line_extension: 0.0,
                caption: false,
                interior_color: None,
            },
        )
    }

    pub fn square(rect: Rect) -> Self {
        Self::new(
            AnnotationType::Square,
            rect,
            AnnotationData::Shape {
                vertices: Vec::new(),
                interior_color: None,
            },
        )
    }

    pub fn circle(rect: Rect) -> Self {
        Self::new(
            AnnotationType::Circle,
            rect,
            AnnotationData::Shape {
                vertices: Vec::new(),
                interior_color: None,
            },
        )
    }

    pub fn polygon(rect: Rect, vertices: Vec<(f64, f64)>) -> Self {
        Self::new(
            AnnotationType::Polygon,
            rect,
            AnnotationData::Shape {
                vertices,
                interior_color: None,
            },
        )
    }

    pub fn polyline(rect: Rect, vertices: Vec<(f64, f64)>) -> Self {
        Self::new(
            AnnotationType::PolyLine,
            rect,
            AnnotationData::Shape {
                vertices,
                interior_color: None,
            },
        )
    }

    pub fn ink(rect: Rect, ink_list: Vec<Vec<(f64, f64)>>) -> Self {
        Self::new(AnnotationType::Ink, rect, AnnotationData::Ink { ink_list })
    }

    pub fn stamp(rect: Rect, stamp_name: &str) -> Self {
        Self::new(
            AnnotationType::Stamp,
            rect,
            AnnotationData::Stamp {
                icon_name: stamp_name.to_string(),
            },
        )
    }

    pub fn link_uri(rect: Rect, uri: &str) -> Self {
        Self::new(
            AnnotationType::Link,
            rect,
            AnnotationData::Link {
                uri: Some(uri.to_string()),
                dest: None,
            },
        )
    }

    pub fn link_goto(rect: Rect, dest: PdfObject) -> Self {
        Self::new(
            AnnotationType::Link,
            rect,
            AnnotationData::Link {
                uri: None,
                dest: Some(dest),
            },
        )
    }

    pub fn file_attachment(rect: Rect, fs_ref: IndirectRef, icon_name: &str) -> Self {
        Self::new(
            AnnotationType::FileAttachment,
            rect,
            AnnotationData::FileAttachment {
                fs_ref: Some(fs_ref),
                icon_name: icon_name.to_string(),
            },
        )
    }

    pub fn redact(rect: Rect) -> Self {
        Self::new(
            AnnotationType::Redact,
            rect,
            AnnotationData::Redact {
                overlay_text: None,
                repeat: false,
                interior_color: None,
            },
        )
    }

    // --- Setters ---

    pub fn contents(mut self, contents: &str) -> Self {
        self.contents = Some(contents.to_string());
        self
    }

    pub fn color(mut self, color: AnnotColor) -> Self {
        self.color = Some(color);
        self
    }

    pub fn border(mut self, border: BorderStyle) -> Self {
        self.border = Some(border);
        self
    }

    pub fn flags(mut self, flags: AnnotationFlags) -> Self {
        self.flags = flags;
        self
    }

    pub fn line_endings(mut self, start: LineEndingStyle, end: LineEndingStyle) -> Self {
        if let AnnotationData::Line {
            ref mut line_endings,
            ..
        } = self.data
        {
            *line_endings = (start, end);
        }
        self
    }

    pub fn interior_color(mut self, color: AnnotColor) -> Self {
        match &mut self.data {
            AnnotationData::Line {
                interior_color,
                ..
            }
            | AnnotationData::Shape {
                interior_color,
                ..
            }
            | AnnotationData::Redact {
                interior_color,
                ..
            } => {
                *interior_color = Some(color);
            }
            _ => {}
        }
        self
    }

    pub fn overlay_text(mut self, text: &str) -> Self {
        if let AnnotationData::Redact {
            overlay_text,
            ..
        } = &mut self.data
        {
            *overlay_text = Some(text.to_string());
        }
        self
    }

    pub fn justification(mut self, q: i64) -> Self {
        if let AnnotationData::FreeText {
            justification,
            ..
        } = &mut self.data
        {
            *justification = q;
        }
        self
    }

    /// Build the annotation dictionary.
    pub fn build_dict(&self) -> PdfDict {
        let mut dict = PdfDict::new();
        dict.insert(b"Type".to_vec(), PdfObject::Name(b"Annot".to_vec()));
        dict.insert(
            b"Subtype".to_vec(),
            PdfObject::Name(self.annot_type.to_name().to_vec()),
        );
        dict.insert(
            b"Rect".to_vec(),
            PdfObject::Array(vec![
                PdfObject::Real(self.rect.llx),
                PdfObject::Real(self.rect.lly),
                PdfObject::Real(self.rect.urx),
                PdfObject::Real(self.rect.ury),
            ]),
        );

        if self.flags.0 != 0 {
            dict.insert(b"F".to_vec(), PdfObject::Integer(self.flags.0 as i64));
        }

        if let Some(ref contents) = self.contents {
            dict.insert(
                b"Contents".to_vec(),
                PdfObject::String(contents.as_bytes().to_vec()),
            );
        }

        if let Some(ref color) = self.color {
            dict.insert(b"C".to_vec(), PdfObject::Array(color.to_pdf_array()));
        }

        if let Some(ref border) = self.border {
            let mut bs = PdfDict::new();
            bs.insert(b"W".to_vec(), PdfObject::Real(border.width));
            bs.insert(
                b"S".to_vec(),
                PdfObject::Name(border.style.to_name().to_vec()),
            );
            if !border.dash_pattern.is_empty() {
                bs.insert(
                    b"D".to_vec(),
                    PdfObject::Array(
                        border
                            .dash_pattern
                            .iter()
                            .map(|&v| PdfObject::Real(v))
                            .collect(),
                    ),
                );
            }
            dict.insert(b"BS".to_vec(), PdfObject::Dict(bs));
        }

        // Type-specific data
        match &self.data {
            AnnotationData::Markup { quad_points } => {
                if !quad_points.is_empty() {
                    dict.insert(
                        b"QuadPoints".to_vec(),
                        PdfObject::Array(
                            quad_points.iter().map(|&v| PdfObject::Real(v)).collect(),
                        ),
                    );
                }
            }
            AnnotationData::Line {
                start,
                end,
                line_endings,
                leader_line_length,
                leader_line_extension,
                caption,
                interior_color,
            } => {
                dict.insert(
                    b"L".to_vec(),
                    PdfObject::Array(vec![
                        PdfObject::Real(start.0),
                        PdfObject::Real(start.1),
                        PdfObject::Real(end.0),
                        PdfObject::Real(end.1),
                    ]),
                );
                dict.insert(
                    b"LE".to_vec(),
                    PdfObject::Array(vec![
                        PdfObject::Name(line_endings.0.to_name().to_vec()),
                        PdfObject::Name(line_endings.1.to_name().to_vec()),
                    ]),
                );
                if *leader_line_length != 0.0 {
                    dict.insert(b"LL".to_vec(), PdfObject::Real(*leader_line_length));
                }
                if *leader_line_extension != 0.0 {
                    dict.insert(b"LLE".to_vec(), PdfObject::Real(*leader_line_extension));
                }
                if *caption {
                    dict.insert(b"Cap".to_vec(), PdfObject::Bool(true));
                }
                if let Some(ic) = interior_color {
                    dict.insert(b"IC".to_vec(), PdfObject::Array(ic.to_pdf_array()));
                }
            }
            AnnotationData::Ink { ink_list } => {
                let ink_arr: Vec<PdfObject> = ink_list
                    .iter()
                    .map(|stroke| {
                        let coords: Vec<PdfObject> = stroke
                            .iter()
                            .flat_map(|&(x, y)| vec![PdfObject::Real(x), PdfObject::Real(y)])
                            .collect();
                        PdfObject::Array(coords)
                    })
                    .collect();
                dict.insert(b"InkList".to_vec(), PdfObject::Array(ink_arr));
            }
            AnnotationData::Link { uri, dest } => {
                if let Some(uri) = uri {
                    let mut action = PdfDict::new();
                    action.insert(b"S".to_vec(), PdfObject::Name(b"URI".to_vec()));
                    action.insert(
                        b"URI".to_vec(),
                        PdfObject::String(uri.as_bytes().to_vec()),
                    );
                    dict.insert(b"A".to_vec(), PdfObject::Dict(action));
                } else if let Some(dest) = dest {
                    dict.insert(b"Dest".to_vec(), dest.clone());
                }
            }
            AnnotationData::FreeText { da, justification } => {
                dict.insert(b"DA".to_vec(), PdfObject::String(da.as_bytes().to_vec()));
                if *justification != 0 {
                    dict.insert(b"Q".to_vec(), PdfObject::Integer(*justification));
                }
            }
            AnnotationData::FileAttachment { fs_ref, icon_name } => {
                if let Some(r) = fs_ref {
                    dict.insert(b"FS".to_vec(), PdfObject::Reference(r.clone()));
                }
                dict.insert(
                    b"Name".to_vec(),
                    PdfObject::Name(icon_name.as_bytes().to_vec()),
                );
            }
            AnnotationData::Stamp { icon_name } => {
                dict.insert(
                    b"Name".to_vec(),
                    PdfObject::Name(icon_name.as_bytes().to_vec()),
                );
            }
            AnnotationData::Shape {
                vertices,
                interior_color,
            } => {
                if !vertices.is_empty() {
                    let coords: Vec<PdfObject> = vertices
                        .iter()
                        .flat_map(|&(x, y)| vec![PdfObject::Real(x), PdfObject::Real(y)])
                        .collect();
                    dict.insert(b"Vertices".to_vec(), PdfObject::Array(coords));
                }
                if let Some(ic) = interior_color {
                    dict.insert(b"IC".to_vec(), PdfObject::Array(ic.to_pdf_array()));
                }
            }
            AnnotationData::Redact {
                overlay_text,
                repeat,
                interior_color,
            } => {
                if let Some(text) = overlay_text {
                    dict.insert(
                        b"OverlayText".to_vec(),
                        PdfObject::String(text.as_bytes().to_vec()),
                    );
                }
                if *repeat {
                    dict.insert(b"Repeat".to_vec(), PdfObject::Bool(true));
                }
                if let Some(ic) = interior_color {
                    dict.insert(b"IC".to_vec(), PdfObject::Array(ic.to_pdf_array()));
                }
            }
            AnnotationData::None => {}
        }

        dict
    }
}

/// Add an annotation to a page.
pub fn add_annotation(
    modifier: &mut DocumentModifier,
    page_obj_num: u32,
    builder: AnnotationBuilder,
) -> Result<IndirectRef> {
    let annot_dict = builder.build_dict();

    // Generate appearance stream
    let ap_ref = super::appearance::generate_appearance(&annot_dict, modifier)?;

    let mut final_dict = annot_dict;
    if let Some(ap_ref) = ap_ref {
        let mut ap_dict = PdfDict::new();
        ap_dict.insert(b"N".to_vec(), PdfObject::Reference(ap_ref));
        final_dict.insert(b"AP".to_vec(), PdfObject::Dict(ap_dict));
    }

    let annot_ref = modifier.add_object(PdfObject::Dict(final_dict));

    // Add to page /Annots array
    let page_obj = modifier
        .find_object_pub(page_obj_num)
        .cloned()
        .unwrap_or(PdfObject::Null);

    if let PdfObject::Dict(mut page_dict) = page_obj {
        let mut annots = match page_dict.remove(b"Annots") {
            Some(PdfObject::Array(arr)) => arr,
            _ => Vec::new(),
        };
        annots.push(PdfObject::Reference(annot_ref.clone()));
        page_dict.insert(b"Annots".to_vec(), PdfObject::Array(annots));
        modifier.set_object(page_obj_num, PdfObject::Dict(page_dict));
    }

    Ok(annot_ref)
}

/// Delete an annotation from a page by index.
pub fn delete_annotation(
    modifier: &mut DocumentModifier,
    page_obj_num: u32,
    annot_index: usize,
) -> Result<()> {
    let page_obj = modifier
        .find_object_pub(page_obj_num)
        .cloned()
        .unwrap_or(PdfObject::Null);

    if let PdfObject::Dict(mut page_dict) = page_obj {
        if let Some(PdfObject::Array(mut annots)) = page_dict.remove(b"Annots") {
            if annot_index >= annots.len() {
                return Err(JustPdfError::AnnotationError {
                    detail: format!(
                        "annotation index {annot_index} out of range ({})",
                        annots.len()
                    ),
                });
            }
            annots.remove(annot_index);
            if !annots.is_empty() {
                page_dict.insert(b"Annots".to_vec(), PdfObject::Array(annots));
            }
            modifier.set_object(page_obj_num, PdfObject::Dict(page_dict));
            Ok(())
        } else {
            Err(JustPdfError::AnnotationError {
                detail: "page has no annotations".into(),
            })
        }
    } else {
        Err(JustPdfError::AnnotationError {
            detail: "invalid page object".into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_highlight_dict() {
        let rect = Rect {
            llx: 100.0,
            lly: 200.0,
            urx: 300.0,
            ury: 220.0,
        };
        let qp = vec![100.0, 220.0, 300.0, 220.0, 100.0, 200.0, 300.0, 200.0];
        let builder =
            AnnotationBuilder::highlight(rect, qp, AnnotColor::Rgb(1.0, 1.0, 0.0));
        let dict = builder.build_dict();

        assert_eq!(dict.get_name(b"Subtype"), Some(b"Highlight".as_slice()));
        assert!(dict.get_array(b"Rect").is_some());
        assert!(dict.get_array(b"QuadPoints").is_some());
        assert!(dict.get_array(b"C").is_some());
    }

    #[test]
    fn test_build_ink_dict() {
        let rect = Rect {
            llx: 0.0,
            lly: 0.0,
            urx: 100.0,
            ury: 100.0,
        };
        let ink_list = vec![vec![(10.0, 20.0), (30.0, 40.0), (50.0, 60.0)]];
        let builder = AnnotationBuilder::ink(rect, ink_list)
            .color(AnnotColor::Rgb(1.0, 0.0, 0.0))
            .contents("Test ink");
        let dict = builder.build_dict();

        assert_eq!(dict.get_name(b"Subtype"), Some(b"Ink".as_slice()));
        assert!(dict.get_array(b"InkList").is_some());
        assert!(dict.get_array(b"C").is_some());
        assert!(dict.get(b"Contents").is_some());
    }

    #[test]
    fn test_build_line_dict() {
        let builder = AnnotationBuilder::line((100.0, 100.0), (300.0, 300.0))
            .line_endings(LineEndingStyle::OpenArrow, LineEndingStyle::ClosedArrow)
            .color(AnnotColor::Rgb(0.0, 0.0, 1.0));
        let dict = builder.build_dict();

        assert_eq!(dict.get_name(b"Subtype"), Some(b"Line".as_slice()));
        assert!(dict.get_array(b"L").is_some());
        assert!(dict.get_array(b"LE").is_some());
    }

    #[test]
    fn test_build_link_uri_dict() {
        let rect = Rect {
            llx: 72.0,
            lly: 700.0,
            urx: 200.0,
            ury: 720.0,
        };
        let builder = AnnotationBuilder::link_uri(rect, "https://example.com");
        let dict = builder.build_dict();

        assert_eq!(dict.get_name(b"Subtype"), Some(b"Link".as_slice()));
        let action = dict.get_dict(b"A").unwrap();
        assert_eq!(action.get_name(b"S"), Some(b"URI".as_slice()));
    }

    #[test]
    fn test_build_redact_dict() {
        let rect = Rect {
            llx: 100.0,
            lly: 200.0,
            urx: 400.0,
            ury: 220.0,
        };
        let builder = AnnotationBuilder::redact(rect)
            .overlay_text("REDACTED")
            .interior_color(AnnotColor::Rgb(0.0, 0.0, 0.0));
        let dict = builder.build_dict();

        assert_eq!(dict.get_name(b"Subtype"), Some(b"Redact".as_slice()));
        assert!(dict.get(b"OverlayText").is_some());
        assert!(dict.get_array(b"IC").is_some());
    }

    #[test]
    fn test_build_stamp_dict() {
        let rect = Rect {
            llx: 100.0,
            lly: 600.0,
            urx: 250.0,
            ury: 650.0,
        };
        let builder = AnnotationBuilder::stamp(rect, "Approved");
        let dict = builder.build_dict();

        assert_eq!(dict.get_name(b"Subtype"), Some(b"Stamp".as_slice()));
        assert_eq!(dict.get_name(b"Name"), Some(b"Approved".as_slice()));
    }
}
