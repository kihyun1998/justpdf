use std::fmt::Write;

use crate::content::{parse_content_stream, ContentOp, Operand};
use crate::error::{JustPdfError, Result};
use crate::object::{PdfDict, PdfObject};
use crate::page::{collect_pages, Rect};
use crate::parser::PdfDocument;
use crate::stream;
use crate::writer::encode::make_stream;
use crate::writer::modify::DocumentModifier;

use super::types::AnnotColor;

/// Apply all redaction annotations on a given page.
///
/// This:
/// 1. Finds all Redact annotations on the page
/// 2. Removes text/image content that falls within redaction rects
/// 3. Draws overlay rectangles (black by default, or IC color)
/// 4. Removes the Redact annotations
pub fn apply_redactions(
    modifier: &mut DocumentModifier,
    doc: &PdfDocument,
    page_index: usize,
) -> Result<()> {
    let pages = collect_pages(doc)?;
    let page = pages.get(page_index).ok_or_else(|| JustPdfError::AnnotationError {
        detail: format!("page index {page_index} out of range"),
    })?;

    let page_obj = doc.resolve(&page.page_ref)?;
    let page_dict = match page_obj.as_dict() {
        Some(d) => d.clone(),
        None => return Ok(()),
    };

    // Collect redaction rects and their properties
    let annots_arr = match page_dict.get(b"Annots") {
        Some(PdfObject::Array(arr)) => arr.clone(),
        Some(PdfObject::Reference(r)) => {
            let r = r.clone();
            match doc.resolve(&r)? {
                PdfObject::Array(arr) => arr,
                _ => return Ok(()),
            }
        }
        _ => return Ok(()),
    };

    let mut redact_rects: Vec<RedactInfo> = Vec::new();
    let mut remaining_annots: Vec<PdfObject> = Vec::new();

    for item in &annots_arr {
        let annot_dict = match item {
            PdfObject::Reference(r) => {
                let r = r.clone();
                match doc.resolve(&r)? {
                    PdfObject::Dict(d) => d,
                    _ => {
                        remaining_annots.push(item.clone());
                        continue;
                    }
                }
            }
            PdfObject::Dict(d) => d.clone(),
            _ => {
                remaining_annots.push(item.clone());
                continue;
            }
        };

        let subtype = annot_dict.get_name(b"Subtype").unwrap_or(b"");
        if subtype != b"Redact" {
            remaining_annots.push(item.clone());
            continue;
        }

        let rect = match annot_dict.get_array(b"Rect").and_then(Rect::from_pdf_array) {
            Some(r) => r,
            None => continue,
        };

        let color = annot_dict
            .get_array(b"IC")
            .and_then(AnnotColor::from_array)
            .unwrap_or(AnnotColor::Rgb(0.0, 0.0, 0.0));

        let overlay_text = annot_dict
            .get(b"OverlayText")
            .and_then(|o| o.as_str())
            .map(|b| String::from_utf8_lossy(b).into_owned());

        redact_rects.push(RedactInfo {
            rect,
            color,
            overlay_text,
        });
    }

    if redact_rects.is_empty() {
        return Ok(());
    }

    // Get and filter the content stream
    let content_data = get_page_content_data(doc, &page_dict)?;
    if !content_data.is_empty() {
        let ops = parse_content_stream(&content_data)
            .map_err(|e| JustPdfError::AnnotationError {
                detail: format!("failed to parse content stream: {e}"),
            })?;

        let filtered_ops = filter_content_ops(&ops, &redact_rects);
        let mut new_content = String::new();
        for op in &filtered_ops {
            let _ = writeln!(new_content, "{op}");
        }

        // Add overlay rectangles
        new_content.push_str("q\n");
        for info in &redact_rects {
            // Fill color
            match &info.color {
                AnnotColor::Gray(g) => { let _ = writeln!(new_content, "{g} g"); }
                AnnotColor::Rgb(r, g, b) => { let _ = writeln!(new_content, "{r} {g} {b} rg"); }
                AnnotColor::Cmyk(c, m, y, k) => { let _ = writeln!(new_content, "{c} {m} {y} {k} k"); }
            }
            let _ = writeln!(
                new_content,
                "{} {} {} {} re\nf",
                info.rect.llx, info.rect.lly,
                info.rect.width(), info.rect.height()
            );
        }
        new_content.push_str("Q\n");

        // Replace page content stream
        let (stream_dict, stream_data) = make_stream(new_content.as_bytes(), true);
        let content_ref = modifier.add_object(PdfObject::Stream {
            dict: stream_dict,
            data: stream_data,
        });

        let mut updated_page = page_dict.clone();
        updated_page.insert(
            b"Contents".to_vec(),
            PdfObject::Reference(content_ref),
        );

        // Update annotations (remove redact annots)
        if remaining_annots.is_empty() {
            updated_page.remove(b"Annots");
        } else {
            updated_page.insert(b"Annots".to_vec(), PdfObject::Array(remaining_annots));
        }

        modifier.set_object(page.page_ref.obj_num, PdfObject::Dict(updated_page));
    }

    Ok(())
}

struct RedactInfo {
    rect: Rect,
    color: AnnotColor,
    #[allow(dead_code)] // Used for future overlay text rendering
    overlay_text: Option<String>,
}

/// Filter content stream ops, removing text that falls within redaction rects.
fn filter_content_ops(ops: &[ContentOp], redact_rects: &[RedactInfo]) -> Vec<ContentOp> {
    let mut result = Vec::new();
    let mut in_text = false;
    // Track text position via Tm, Td, TD, T* operators
    let mut text_x: f64 = 0.0;
    let mut text_y: f64 = 0.0;
    let mut text_matrix_set = false;
    // Track CTM for image positioning
    let mut ctm_e: f64 = 0.0;
    let mut ctm_f: f64 = 0.0;
    let mut ctm_a: f64 = 1.0;
    let mut ctm_d: f64 = 1.0;
    let mut skip_text_block = false;

    for op in ops {
        let op_name = op.operator.as_slice();

        match op_name {
            b"BT" => {
                in_text = true;
                text_x = 0.0;
                text_y = 0.0;
                text_matrix_set = false;
                skip_text_block = false;
            }
            b"ET" => {
                if !skip_text_block {
                    result.push(op.clone());
                }
                in_text = false;
                skip_text_block = false;
                continue;
            }
            b"cm" => {
                // Track CTM for image positioning
                if op.operands.len() >= 6 {
                    ctm_a = operand_f64(&op.operands[0]);
                    ctm_d = operand_f64(&op.operands[3]);
                    ctm_e = operand_f64(&op.operands[4]);
                    ctm_f = operand_f64(&op.operands[5]);
                }
            }
            b"Tm" if in_text => {
                // Text matrix: a b c d e f
                if op.operands.len() >= 6 {
                    text_x = operand_f64(&op.operands[4]);
                    text_y = operand_f64(&op.operands[5]);
                    text_matrix_set = true;
                }
            }
            b"Td" | b"TD" if in_text => {
                if op.operands.len() >= 2 {
                    text_x += operand_f64(&op.operands[0]);
                    text_y += operand_f64(&op.operands[1]);
                    text_matrix_set = true;
                }
            }
            b"T*" if in_text => {
                // Move to next line (approximate: decrease y by leading)
                text_y -= 12.0; // approximate leading
                text_matrix_set = true;
            }
            _ => {}
        }

        if in_text {
            let is_text_showing = matches!(
                op_name,
                b"Tj" | b"TJ" | b"'" | b"\""
            );

            if is_text_showing && text_matrix_set {
                let in_redact = redact_rects.iter().any(|info| {
                    point_in_rect(text_x, text_y, &info.rect)
                });
                if in_redact {
                    // Skip this entire text block
                    skip_text_block = true;
                    // Remove previously added BT and text positioning ops
                    // by draining back to the last BT
                    while let Some(last) = result.last() {
                        if last.operator == b"BT" {
                            result.pop();
                            break;
                        }
                        result.pop();
                    }
                    continue;
                }
            }

            if skip_text_block {
                continue;
            }
        }

        // Check image Do operations
        if op_name == b"Do" {
            let img_rect = Rect {
                llx: ctm_e,
                lly: ctm_f,
                urx: ctm_e + ctm_a,
                ury: ctm_f + ctm_d,
            };
            let in_redact = redact_rects.iter().any(|info| {
                rects_overlap(&img_rect, &info.rect)
            });
            if in_redact {
                continue; // Skip this image
            }
        }

        result.push(op.clone());
    }

    result
}

fn operand_f64(op: &Operand) -> f64 {
    match op {
        Operand::Real(v) => *v,
        Operand::Integer(v) => *v as f64,
        _ => 0.0,
    }
}

fn point_in_rect(x: f64, y: f64, rect: &Rect) -> bool {
    let (min_x, max_x) = if rect.llx <= rect.urx {
        (rect.llx, rect.urx)
    } else {
        (rect.urx, rect.llx)
    };
    let (min_y, max_y) = if rect.lly <= rect.ury {
        (rect.lly, rect.ury)
    } else {
        (rect.ury, rect.lly)
    };
    x >= min_x && x <= max_x && y >= min_y && y <= max_y
}

fn rects_overlap(a: &Rect, b: &Rect) -> bool {
    let a_min_x = a.llx.min(a.urx);
    let a_max_x = a.llx.max(a.urx);
    let a_min_y = a.lly.min(a.ury);
    let a_max_y = a.lly.max(a.ury);
    let b_min_x = b.llx.min(b.urx);
    let b_max_x = b.llx.max(b.urx);
    let b_min_y = b.lly.min(b.ury);
    let b_max_y = b.lly.max(b.ury);

    a_min_x < b_max_x && a_max_x > b_min_x && a_min_y < b_max_y && a_max_y > b_min_y
}

/// Get raw content stream data from page dict.
fn get_page_content_data(doc: &PdfDocument, page_dict: &PdfDict) -> Result<Vec<u8>> {
    let contents = match page_dict.get(b"Contents") {
        Some(obj) => obj.clone(),
        None => return Ok(Vec::new()),
    };

    match contents {
        PdfObject::Reference(r) => {
            let resolved = doc.resolve(&r)?;
            match resolved {
                PdfObject::Stream { dict, data } => {
                    stream::decode_stream(&data, &dict)
                }
                _ => Ok(Vec::new()),
            }
        }
        PdfObject::Array(arr) => {
            let mut all_data = Vec::new();
            for item in &arr {
                if let PdfObject::Reference(r) = item {
                    let resolved = doc.resolve(r)?;
                    if let PdfObject::Stream { dict, data } = resolved {
                        let decoded = stream::decode_stream(&data, &dict)?;
                        all_data.extend_from_slice(&decoded);
                        all_data.push(b'\n');
                    }
                }
            }
            Ok(all_data)
        }
        PdfObject::Stream { dict, data } => {
            stream::decode_stream(&data, &dict)
        }
        _ => Ok(Vec::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::Operand;

    #[test]
    fn test_point_in_rect() {
        let rect = Rect { llx: 100.0, lly: 200.0, urx: 300.0, ury: 220.0 };
        assert!(point_in_rect(150.0, 210.0, &rect));
        assert!(point_in_rect(100.0, 200.0, &rect)); // edge
        assert!(!point_in_rect(50.0, 210.0, &rect));
        assert!(!point_in_rect(150.0, 250.0, &rect));
    }

    #[test]
    fn test_rects_overlap() {
        let a = Rect { llx: 0.0, lly: 0.0, urx: 100.0, ury: 100.0 };
        let b = Rect { llx: 50.0, lly: 50.0, urx: 150.0, ury: 150.0 };
        assert!(rects_overlap(&a, &b));

        let c = Rect { llx: 200.0, lly: 200.0, urx: 300.0, ury: 300.0 };
        assert!(!rects_overlap(&a, &c));
    }

    #[test]
    fn test_filter_removes_text_in_redact_rect() {
        let ops = vec![
            ContentOp { operator: b"BT".to_vec(), operands: vec![] },
            ContentOp {
                operator: b"Td".to_vec(),
                operands: vec![Operand::Real(150.0), Operand::Real(710.0)],
            },
            ContentOp {
                operator: b"Tj".to_vec(),
                operands: vec![Operand::String(b"Secret".to_vec())],
            },
            ContentOp { operator: b"ET".to_vec(), operands: vec![] },
        ];

        let redact = vec![RedactInfo {
            rect: Rect { llx: 100.0, lly: 700.0, urx: 300.0, ury: 720.0 },
            color: AnnotColor::Rgb(0.0, 0.0, 0.0),
            overlay_text: None,
        }];

        let filtered = filter_content_ops(&ops, &redact);
        // BT/Td/Tj/ET should all be removed
        assert!(
            filtered.iter().all(|op| op.operator != b"Tj"),
            "Tj should be removed"
        );
        assert!(
            filtered.iter().all(|op| op.operator != b"BT"),
            "BT should be removed"
        );
    }

    #[test]
    fn test_filter_keeps_text_outside_redact_rect() {
        let ops = vec![
            ContentOp { operator: b"BT".to_vec(), operands: vec![] },
            ContentOp {
                operator: b"Td".to_vec(),
                operands: vec![Operand::Real(150.0), Operand::Real(500.0)],
            },
            ContentOp {
                operator: b"Tj".to_vec(),
                operands: vec![Operand::String(b"Public".to_vec())],
            },
            ContentOp { operator: b"ET".to_vec(), operands: vec![] },
        ];

        let redact = vec![RedactInfo {
            rect: Rect { llx: 100.0, lly: 700.0, urx: 300.0, ury: 720.0 },
            color: AnnotColor::Rgb(0.0, 0.0, 0.0),
            overlay_text: None,
        }];

        let filtered = filter_content_ops(&ops, &redact);
        assert_eq!(filtered.len(), 4); // All ops kept
    }

    #[test]
    fn test_filter_removes_image_in_redact_rect() {
        let ops = vec![
            ContentOp { operator: b"q".to_vec(), operands: vec![] },
            ContentOp {
                operator: b"cm".to_vec(),
                operands: vec![
                    Operand::Real(200.0), Operand::Real(0.0),
                    Operand::Real(0.0), Operand::Real(100.0),
                    Operand::Real(150.0), Operand::Real(705.0),
                ],
            },
            ContentOp {
                operator: b"Do".to_vec(),
                operands: vec![Operand::Name(b"Im1".to_vec())],
            },
            ContentOp { operator: b"Q".to_vec(), operands: vec![] },
        ];

        let redact = vec![RedactInfo {
            rect: Rect { llx: 100.0, lly: 700.0, urx: 400.0, ury: 820.0 },
            color: AnnotColor::Rgb(0.0, 0.0, 0.0),
            overlay_text: None,
        }];

        let filtered = filter_content_ops(&ops, &redact);
        // Do should be removed
        assert!(
            filtered.iter().all(|op| op.operator != b"Do"),
            "Do should be removed"
        );
    }
}
