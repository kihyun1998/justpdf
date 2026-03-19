//! BBox Device: a lightweight device that tracks the bounding box of all drawing operations
//! without actually rendering pixels.

use justpdf_core::page::{Rect, collect_pages};
use justpdf_core::PdfDocument;
use justpdf_core::content::{ContentOp, Operand, parse_content_stream};
use justpdf_core::object::PdfObject;
use justpdf_core::page::PageInfo;

use crate::error::{RenderError, Result};
use crate::graphics_state::Matrix;

/// A bounding box tracker that records the extent of all drawing operations.
pub struct BBoxDevice {
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
    has_content: bool,
    /// Transform from PDF user space to page space.
    #[allow(dead_code)]
    page_transform: Matrix,
    /// Current transformation matrix.
    ctm: Matrix,
    ctm_stack: Vec<Matrix>,
}

impl BBoxDevice {
    pub fn new(page_transform: Matrix) -> Self {
        Self {
            min_x: f64::MAX,
            min_y: f64::MAX,
            max_x: f64::MIN,
            max_y: f64::MIN,
            has_content: false,
            page_transform,
            ctm: Matrix::identity(),
            ctm_stack: Vec::new(),
        }
    }

    /// Get the computed bounding box, or None if no content was drawn.
    pub fn bbox(&self) -> Option<Rect> {
        if !self.has_content {
            return None;
        }
        Some(Rect {
            llx: self.min_x,
            lly: self.min_y,
            urx: self.max_x,
            ury: self.max_y,
        })
    }

    /// Extend the bounding box with a point in PDF user space.
    fn extend_point(&mut self, x: f64, y: f64) {
        // Transform point through CTM (but NOT page_transform — we want PDF coordinates)
        let (tx, ty) = self.ctm.transform_point(x, y);
        self.min_x = self.min_x.min(tx);
        self.min_y = self.min_y.min(ty);
        self.max_x = self.max_x.max(tx);
        self.max_y = self.max_y.max(ty);
        self.has_content = true;
    }

    /// Extend the bounding box with a rectangle in PDF user space.
    pub fn extend_rect(&mut self, x: f64, y: f64, w: f64, h: f64) {
        self.extend_point(x, y);
        self.extend_point(x + w, y);
        self.extend_point(x + w, y + h);
        self.extend_point(x, y + h);
    }

    /// Process content stream operations to compute bounding box.
    pub fn process_ops(&mut self, ops: &[ContentOp]) {
        let mut path_points: Vec<(f64, f64)> = Vec::new();
        let mut text_matrix = Matrix::identity();
        let mut text_line_matrix = Matrix::identity();
        let mut font_size = 12.0_f64;
        let mut text_rise = 0.0_f64;

        for op in ops {
            let operator = op.operator_str();
            let operands = &op.operands;

            match operator {
                "q" => self.ctm_stack.push(self.ctm),
                "Q" => {
                    if let Some(m) = self.ctm_stack.pop() {
                        self.ctm = m;
                    }
                }
                "cm" => {
                    if operands.len() >= 6 {
                        let m = Matrix {
                            a: f(operands, 0),
                            b: f(operands, 1),
                            c: f(operands, 2),
                            d: f(operands, 3),
                            e: f(operands, 4),
                            f: f(operands, 5),
                        };
                        self.ctm = m.concat(&self.ctm);
                    }
                }

                // Path construction
                "m" | "l" => {
                    if operands.len() >= 2 {
                        path_points.push((f(operands, 0), f(operands, 1)));
                    }
                }
                "c" => {
                    if operands.len() >= 6 {
                        path_points.push((f(operands, 0), f(operands, 1)));
                        path_points.push((f(operands, 2), f(operands, 3)));
                        path_points.push((f(operands, 4), f(operands, 5)));
                    }
                }
                "v" => {
                    if operands.len() >= 4 {
                        path_points.push((f(operands, 0), f(operands, 1)));
                        path_points.push((f(operands, 2), f(operands, 3)));
                    }
                }
                "y" => {
                    if operands.len() >= 4 {
                        path_points.push((f(operands, 0), f(operands, 1)));
                        path_points.push((f(operands, 2), f(operands, 3)));
                    }
                }
                "re" => {
                    if operands.len() >= 4 {
                        let x = f(operands, 0);
                        let y = f(operands, 1);
                        let w = f(operands, 2);
                        let h = f(operands, 3);
                        path_points.push((x, y));
                        path_points.push((x + w, y));
                        path_points.push((x + w, y + h));
                        path_points.push((x, y + h));
                    }
                }

                // Path painting — flush points to bbox
                "S" | "s" | "f" | "F" | "f*" | "B" | "B*" | "b" | "b*" => {
                    for &(x, y) in &path_points {
                        self.extend_point(x, y);
                    }
                    path_points.clear();
                }
                "n" => {
                    path_points.clear();
                }

                // Text
                "BT" => {
                    text_matrix = Matrix::identity();
                    text_line_matrix = Matrix::identity();
                }
                "Tf" => {
                    if operands.len() > 1 {
                        font_size = f(operands, 1);
                    }
                }
                "Ts" => {
                    text_rise = f(operands, 0);
                }
                "Td" | "TD" => {
                    let tx = f(operands, 0);
                    let ty = f(operands, 1);
                    let t = Matrix::translate(tx, ty);
                    text_line_matrix = t.concat(&text_line_matrix);
                    text_matrix = text_line_matrix;
                }
                "Tm" => {
                    if operands.len() >= 6 {
                        let m = Matrix {
                            a: f(operands, 0),
                            b: f(operands, 1),
                            c: f(operands, 2),
                            d: f(operands, 3),
                            e: f(operands, 4),
                            f: f(operands, 5),
                        };
                        text_matrix = m;
                        text_line_matrix = m;
                    }
                }
                "Tj" | "'" => {
                    // Approximate text bbox: a rectangle at text position
                    let trm = text_matrix.concat(&self.ctm);
                    let (tx, ty) = (trm.e, trm.f + text_rise);
                    self.extend_point_raw(tx, ty);
                    self.extend_point_raw(tx, ty + font_size);
                }
                "TJ" => {
                    let trm = text_matrix.concat(&self.ctm);
                    let (tx, ty) = (trm.e, trm.f + text_rise);
                    self.extend_point_raw(tx, ty);
                    self.extend_point_raw(tx, ty + font_size);
                }

                // Image XObject
                "Do" => {
                    // Image/Form XObject: bbox is the CTM-transformed unit square
                    self.extend_point(0.0, 0.0);
                    self.extend_point(1.0, 0.0);
                    self.extend_point(1.0, 1.0);
                    self.extend_point(0.0, 1.0);
                }

                _ => {}
            }
        }
    }

    /// Extend bbox with a point already in page coordinates (not through CTM).
    fn extend_point_raw(&mut self, x: f64, y: f64) {
        self.min_x = self.min_x.min(x);
        self.min_y = self.min_y.min(y);
        self.max_x = self.max_x.max(x);
        self.max_y = self.max_y.max(y);
        self.has_content = true;
    }
}

/// Compute the content bounding box for a page (in PDF user space coordinates).
pub fn compute_page_bbox(doc: &mut PdfDocument, page_index: usize) -> Result<Option<Rect>> {
    let pages = collect_pages(doc)?;
    let page = pages
        .get(page_index)
        .ok_or_else(|| RenderError::InvalidDimensions {
            detail: format!("page index {page_index} out of range"),
        })?
        .clone();

    let media_box = page.crop_box.unwrap_or(page.media_box);
    let page_transform = crate::render::compute_page_transform(&media_box, 1.0, page.rotate);

    let mut bbox = BBoxDevice::new(page_transform);

    // Get content stream
    let content_data = get_page_content(doc, &page)?;
    if content_data.is_empty() {
        return Ok(None);
    }

    let ops = parse_content_stream(&content_data).map_err(RenderError::Core)?;
    bbox.process_ops(&ops);

    Ok(bbox.bbox())
}

/// Helper to get page content data (simplified version).
fn get_page_content(doc: &mut PdfDocument, page: &PageInfo) -> Result<Vec<u8>> {
    let contents = match &page.contents_ref {
        Some(c) => c.clone(),
        None => return Ok(Vec::new()),
    };

    match &contents {
        PdfObject::Reference(r) => {
            let r = r.clone();
            let obj = doc.resolve(&r)?.clone();
            match obj {
                PdfObject::Stream { dict, data } => {
                    Ok(doc.decode_stream(&dict, &data).unwrap_or_default())
                }
                PdfObject::Array(arr) => concat_streams(doc, &arr),
                _ => Ok(Vec::new()),
            }
        }
        PdfObject::Stream { dict, data } => {
            Ok(doc.decode_stream(dict, data).unwrap_or_default())
        }
        PdfObject::Array(arr) => {
            let arr = arr.clone();
            concat_streams(doc, &arr)
        }
        _ => Ok(Vec::new()),
    }
}

fn concat_streams(doc: &mut PdfDocument, arr: &[PdfObject]) -> Result<Vec<u8>> {
    let mut combined = Vec::new();
    for item in arr {
        let obj = match item {
            PdfObject::Reference(r) => {
                let r = r.clone();
                doc.resolve(&r)?.clone()
            }
            other => other.clone(),
        };
        if let PdfObject::Stream { dict, data } = obj
            && let Ok(decoded) = doc.decode_stream(&dict, &data)
        {
            combined.extend_from_slice(&decoded);
            combined.push(b' ');
        }
    }
    Ok(combined)
}

fn f(operands: &[Operand], idx: usize) -> f64 {
    operands.get(idx).and_then(|o| o.as_f64()).unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bbox_no_content() {
        let bbox = BBoxDevice::new(Matrix::identity());
        assert!(bbox.bbox().is_none());
    }

    #[test]
    fn test_bbox_single_point() {
        let mut bbox = BBoxDevice::new(Matrix::identity());
        bbox.extend_point(10.0, 20.0);
        let r = bbox.bbox().unwrap();
        assert!((r.llx - 10.0).abs() < 0.001);
        assert!((r.lly - 20.0).abs() < 0.001);
    }

    #[test]
    fn test_bbox_rectangle() {
        let mut bbox = BBoxDevice::new(Matrix::identity());
        bbox.extend_rect(10.0, 20.0, 100.0, 50.0);
        let r = bbox.bbox().unwrap();
        assert!((r.llx - 10.0).abs() < 0.001);
        assert!((r.lly - 20.0).abs() < 0.001);
        assert!((r.urx - 110.0).abs() < 0.001);
        assert!((r.ury - 70.0).abs() < 0.001);
    }
}
