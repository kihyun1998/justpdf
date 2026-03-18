use std::collections::HashMap;

use justpdf_core::color::{Color as PdfColor, ColorSpace};
use justpdf_core::content::{ContentOp, Operand, parse_content_stream};
use justpdf_core::font::{FontInfo, ToUnicodeCMap, parse_font_info};
use justpdf_core::image;
use justpdf_core::object::{PdfDict, PdfObject};
use justpdf_core::page::PageInfo;
use justpdf_core::PdfDocument;
use tiny_skia::{FillRule, Mask, PathBuilder, Pixmap, Transform};

use crate::device::PixmapDevice;
use crate::error::{RenderError, Result};
use crate::graphics_state::{
    GraphicsState, LineCap, LineJoin, Matrix, PdfBlendMode, SoftMask, SoftMaskSubtype,
};

/// Resolved font for rendering.
struct ResolvedFont {
    info: FontInfo,
    #[allow(dead_code)]
    cmap: Option<ToUnicodeCMap>,
    /// Raw embedded font data (TrueType/OpenType/CFF) for glyph outlines.
    font_data: Option<Vec<u8>>,
}

/// The rendering interpreter: walks content stream ops and renders onto a device.
pub struct RenderInterpreter<'a> {
    doc: &'a mut PdfDocument,
    device: &'a mut PixmapDevice,
    state: GraphicsState,
    state_stack: Vec<GraphicsState>,
    fonts: HashMap<Vec<u8>, ResolvedFont>,
    /// Transform from PDF user space to device (pixel) space.
    page_transform: Matrix,
    /// Current path being constructed.
    path_builder: Option<PathBuilder>,
    /// Form XObject recursion depth limit.
    xobject_depth: u32,
}

impl<'a> RenderInterpreter<'a> {
    pub fn new(
        doc: &'a mut PdfDocument,
        device: &'a mut PixmapDevice,
        page_transform: Matrix,
    ) -> Self {
        Self {
            doc,
            device,
            state: GraphicsState::default(),
            state_stack: Vec::new(),
            fonts: HashMap::new(),
            page_transform,
            path_builder: None,
            xobject_depth: 0,
        }
    }

    /// Render a page's content streams.
    pub fn render_page(&mut self, page: &PageInfo) -> Result<()> {
        // Resolve resources and fonts (non-fatal if it fails)
        let _ = self.resolve_page_fonts(page);

        // Get content stream data
        let content_data = self.get_page_content(page)?;
        if content_data.is_empty() {
            return Ok(());
        }

        let ops = parse_content_stream(&content_data).map_err(|e| RenderError::Core(e))?;
        self.execute_ops(&ops, page)?;
        Ok(())
    }

    fn resolve_page_fonts(&mut self, page: &PageInfo) -> Result<()> {
        let resources_obj = match &page.resources_ref {
            Some(obj) => self.resolve_object(obj)?,
            None => return Ok(()),
        };

        let resources_dict = match &resources_obj {
            PdfObject::Dict(d) => d.clone(),
            _ => return Ok(()),
        };

        let font_dict_obj = match resources_dict.get(b"Font") {
            Some(PdfObject::Dict(d)) => PdfObject::Dict(d.clone()),
            Some(PdfObject::Reference(r)) => {
                let r = r.clone();
                self.doc.resolve(&r)?.clone()
            }
            _ => return Ok(()),
        };

        if let PdfObject::Dict(font_dict) = &font_dict_obj {
            for (name, val) in font_dict.iter() {
                let font_obj = match val {
                    PdfObject::Reference(r) => {
                        let r = r.clone();
                        self.doc.resolve(&r)?.clone()
                    }
                    other => other.clone(),
                };

                if let PdfObject::Dict(fd) = &font_obj {
                    let mut info = parse_font_info(fd);

                    // Resolve ToUnicode CMap
                    let cmap = if let Some(PdfObject::Reference(tu_ref)) = fd.get(b"ToUnicode") {
                        let tu_ref = tu_ref.clone();
                        if let Ok(tu_obj) = self.doc.resolve(&tu_ref) {
                            if let PdfObject::Stream { dict, data } = tu_obj.clone() {
                                let decoded = self.doc.decode_stream(&dict, &data).ok();
                                decoded.map(|d| ToUnicodeCMap::parse(&d))
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    };

                    // Resolve CIDFont widths and font descriptor for Type0 fonts
                    let mut cid_font_descriptor: Option<PdfDict> = None;
                    if info.subtype == b"Type0" {
                        if let Some(PdfObject::Array(descendants)) =
                            fd.get(b"DescendantFonts")
                        {
                            if let Some(desc_ref) = descendants.first() {
                                let desc_obj = match desc_ref {
                                    PdfObject::Reference(r) => {
                                        let r = r.clone();
                                        self.doc.resolve(&r)?.clone()
                                    }
                                    other => other.clone(),
                                };
                                if let PdfObject::Dict(cid_dict) = &desc_obj {
                                    let cid_info = parse_font_info(cid_dict);
                                    info.widths = cid_info.widths;
                                    // Get font descriptor from CID font
                                    if let Some(fd_obj) = cid_dict.get(b"FontDescriptor") {
                                        let fd_resolved = match fd_obj {
                                            PdfObject::Reference(r) => {
                                                let r = r.clone();
                                                self.doc.resolve(&r).ok().cloned()
                                            }
                                            other => Some(other.clone()),
                                        };
                                        if let Some(PdfObject::Dict(d)) = fd_resolved {
                                            cid_font_descriptor = Some(d);
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Extract embedded font data from FontDescriptor
                    let font_data = self.extract_font_data(
                        fd,
                        cid_font_descriptor.as_ref(),
                    );

                    self.fonts.insert(
                        name.clone(),
                        ResolvedFont { info, cmap, font_data },
                    );
                }
            }
        }

        Ok(())
    }

    /// Extract embedded font program from FontDescriptor.
    /// Looks for FontFile, FontFile2 (TrueType), FontFile3 (CFF/OpenType).
    fn extract_font_data(
        &mut self,
        font_dict: &PdfDict,
        cid_descriptor: Option<&PdfDict>,
    ) -> Option<Vec<u8>> {
        // First try the font's own descriptor, then CID font descriptor
        let descriptor = self
            .get_font_descriptor(font_dict)
            .or_else(|| cid_descriptor.cloned());

        let descriptor = descriptor?;

        // Try FontFile2 (TrueType), FontFile3 (CFF/OpenType), FontFile (Type1)
        for key in &[b"FontFile2".as_slice(), b"FontFile3", b"FontFile"] {
            if let Some(obj) = descriptor.get(*key) {
                let stream_obj = match obj {
                    PdfObject::Reference(r) => {
                        let r = r.clone();
                        self.doc.resolve(&r).ok().cloned()
                    }
                    other => Some(other.clone()),
                };
                if let Some(PdfObject::Stream { dict, data }) = stream_obj {
                    if let Ok(decoded) = self.doc.decode_stream(&dict, &data) {
                        return Some(decoded);
                    }
                }
            }
        }

        None
    }

    fn get_font_descriptor(&mut self, font_dict: &PdfDict) -> Option<PdfDict> {
        let fd_obj = font_dict.get(b"FontDescriptor")?;
        let resolved = match fd_obj {
            PdfObject::Reference(r) => {
                let r = r.clone();
                self.doc.resolve(&r).ok().cloned()
            }
            other => Some(other.clone()),
        };
        match resolved {
            Some(PdfObject::Dict(d)) => Some(d),
            _ => None,
        }
    }

    fn resolve_object(&mut self, obj: &PdfObject) -> Result<PdfObject> {
        match obj {
            PdfObject::Reference(r) => {
                let r = r.clone();
                Ok(self.doc.resolve(&r)?.clone())
            }
            other => Ok(other.clone()),
        }
    }

    fn get_page_content(&mut self, page: &PageInfo) -> Result<Vec<u8>> {
        let contents = match &page.contents_ref {
            Some(c) => c.clone(),
            None => return Ok(Vec::new()),
        };

        match &contents {
            PdfObject::Reference(r) => {
                let r = r.clone();
                let obj = self.doc.resolve(&r)?.clone();
                match obj {
                    PdfObject::Stream { dict, data } => {
                        Ok(self.doc.decode_stream(&dict, &data).unwrap_or_default())
                    }
                    PdfObject::Array(arr) => self.concat_content_streams(&arr),
                    _ => Ok(Vec::new()),
                }
            }
            PdfObject::Array(arr) => {
                let arr = arr.clone();
                self.concat_content_streams(&arr)
            }
            PdfObject::Stream { dict, data } => {
                Ok(self.doc.decode_stream(dict, data).unwrap_or_default())
            }
            _ => Ok(Vec::new()),
        }
    }

    fn concat_content_streams(&mut self, arr: &[PdfObject]) -> Result<Vec<u8>> {
        let mut combined = Vec::new();
        for item in arr {
            let obj = match item {
                PdfObject::Reference(r) => {
                    let r = r.clone();
                    self.doc.resolve(&r)?.clone()
                }
                other => other.clone(),
            };
            if let PdfObject::Stream { dict, data } = obj {
                // Skip streams that fail to decode (corrupt data)
                if let Ok(decoded) = self.doc.decode_stream(&dict, &data) {
                    combined.extend_from_slice(&decoded);
                    combined.push(b' ');
                }
            }
        }
        Ok(combined)
    }

    fn execute_ops(&mut self, ops: &[ContentOp], page: &PageInfo) -> Result<()> {
        for op in ops {
            self.execute_op(op, page)?;
        }
        Ok(())
    }

    fn execute_op(&mut self, op: &ContentOp, page: &PageInfo) -> Result<()> {
        let operator = op.operator_str();
        let operands = &op.operands;

        match operator {
            // --- Graphics state ---
            "q" => {
                self.state_stack.push(self.state.clone());
            }
            "Q" => {
                let had_clip = self.state.has_clip;
                if let Some(s) = self.state_stack.pop() {
                    self.state = s;
                }
                // If the popped state had a clip but restored state doesn't,
                // we need to clear the device clip
                if had_clip && !self.state.has_clip {
                    self.device.clear_clip();
                    // Re-apply clip from state stack if any parent has one
                    // (simplified: just clear for now)
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
                    self.state.ctm = m.concat(&self.state.ctm);
                }
            }

            // Line parameters
            "w" => {
                if let Some(v) = operands.first().and_then(|o| o.as_f64()) {
                    self.state.line_width = v;
                }
            }
            "J" => {
                if let Some(v) = operands.first().and_then(|o| o.as_i64()) {
                    self.state.line_cap = match v {
                        1 => LineCap::Round,
                        2 => LineCap::Square,
                        _ => LineCap::Butt,
                    };
                }
            }
            "j" => {
                if let Some(v) = operands.first().and_then(|o| o.as_i64()) {
                    self.state.line_join = match v {
                        1 => LineJoin::Round,
                        2 => LineJoin::Bevel,
                        _ => LineJoin::Miter,
                    };
                }
            }
            "M" => {
                if let Some(v) = operands.first().and_then(|o| o.as_f64()) {
                    self.state.miter_limit = v;
                }
            }
            "d" => {
                // [array] phase
                if operands.len() >= 2 {
                    if let Some(arr) = operands[0].as_array() {
                        self.state.dash_pattern =
                            arr.iter().filter_map(|o| o.as_f64()).collect();
                    }
                    self.state.dash_phase = f(operands, 1);
                }
            }

            // ExtGState
            "gs" => {
                if let Some(name) = operands.first().and_then(|o| o.as_name()) {
                    self.apply_extgstate(name, page)?;
                }
            }

            // --- Path construction ---
            "m" => {
                let pb = self.path_builder.get_or_insert_with(PathBuilder::new);
                pb.move_to(f(operands, 0) as f32, f(operands, 1) as f32);
            }
            "l" => {
                if let Some(pb) = &mut self.path_builder {
                    pb.line_to(f(operands, 0) as f32, f(operands, 1) as f32);
                }
            }
            "c" => {
                if let Some(pb) = &mut self.path_builder {
                    pb.cubic_to(
                        f(operands, 0) as f32,
                        f(operands, 1) as f32,
                        f(operands, 2) as f32,
                        f(operands, 3) as f32,
                        f(operands, 4) as f32,
                        f(operands, 5) as f32,
                    );
                }
            }
            "v" => {
                // current point as first control point
                if let Some(pb) = &mut self.path_builder {
                    // tiny-skia doesn't have v/y directly, use cubic with same start
                    // For 'v': cp1 = current point — but we don't track it here,
                    // so we approximate with a cubic. This is lossy without current point tracking.
                    // A proper implementation would track last point. For now, use cubic_to
                    // with first control = last moved point (not perfectly correct for all cases).
                    pb.cubic_to(
                        f(operands, 0) as f32, // actually should be current point
                        f(operands, 1) as f32,
                        f(operands, 0) as f32,
                        f(operands, 1) as f32,
                        f(operands, 2) as f32,
                        f(operands, 3) as f32,
                    );
                }
            }
            "y" => {
                if let Some(pb) = &mut self.path_builder {
                    // 'y': cp2 = end point
                    pb.cubic_to(
                        f(operands, 0) as f32,
                        f(operands, 1) as f32,
                        f(operands, 2) as f32,
                        f(operands, 3) as f32,
                        f(operands, 2) as f32,
                        f(operands, 3) as f32,
                    );
                }
            }
            "h" => {
                if let Some(pb) = &mut self.path_builder {
                    pb.close();
                }
            }
            "re" => {
                // Rectangle: x y width height
                if operands.len() >= 4 {
                    let x = f(operands, 0) as f32;
                    let y = f(operands, 1) as f32;
                    let w = f(operands, 2) as f32;
                    let h = f(operands, 3) as f32;
                    let pb = self.path_builder.get_or_insert_with(PathBuilder::new);
                    pb.move_to(x, y);
                    pb.line_to(x + w, y);
                    pb.line_to(x + w, y + h);
                    pb.line_to(x, y + h);
                    pb.close();
                }
            }

            // --- Path painting ---
            "S" => {
                // Stroke
                self.stroke_current_path(page);
            }
            "s" => {
                // Close and stroke
                if let Some(pb) = &mut self.path_builder {
                    pb.close();
                }
                self.stroke_current_path(page);
            }
            "f" | "F" => {
                // Fill (non-zero winding)
                self.fill_current_path(FillRule::Winding, page);
            }
            "f*" => {
                // Fill (even-odd)
                self.fill_current_path(FillRule::EvenOdd, page);
            }
            "B" => {
                // Fill and stroke (non-zero)
                self.fill_current_path_keep(FillRule::Winding, page);
                self.stroke_current_path(page);
            }
            "B*" => {
                self.fill_current_path_keep(FillRule::EvenOdd, page);
                self.stroke_current_path(page);
            }
            "b" => {
                if let Some(pb) = &mut self.path_builder {
                    pb.close();
                }
                self.fill_current_path_keep(FillRule::Winding, page);
                self.stroke_current_path(page);
            }
            "b*" => {
                if let Some(pb) = &mut self.path_builder {
                    pb.close();
                }
                self.fill_current_path_keep(FillRule::EvenOdd, page);
                self.stroke_current_path(page);
            }
            "n" => {
                // End path without fill/stroke (used for clipping)
                self.path_builder = None;
            }

            // --- Clipping ---
            "W" => {
                self.apply_clip(FillRule::Winding);
            }
            "W*" => {
                self.apply_clip(FillRule::EvenOdd);
            }

            // --- Color operators ---
            "CS" => {
                if let Some(name) = operands.first().and_then(|o| o.as_name()) {
                    self.state.stroke_cs = cs_from_name(name);
                    if name != b"Pattern" {
                        self.state.stroke_pattern_name = None;
                    }
                }
            }
            "cs" => {
                if let Some(name) = operands.first().and_then(|o| o.as_name()) {
                    self.state.fill_cs = cs_from_name(name);
                    if name != b"Pattern" {
                        self.state.fill_pattern_name = None;
                    }
                }
            }
            "SC" | "SCN" => {
                // Last operand may be a pattern name if stroke CS is Pattern
                let last_is_name = operands.last().and_then(|o| o.as_name());
                if last_is_name.is_some() {
                    self.state.stroke_pattern_name =
                        last_is_name.map(|n| n.to_vec());
                    // Remaining numeric operands are underlying color components
                    let comps: Vec<f64> = operands.iter().filter_map(|o| o.as_f64()).collect();
                    if !comps.is_empty() {
                        self.state.stroke_color = PdfColor { components: comps };
                    }
                } else {
                    let comps: Vec<f64> = operands.iter().filter_map(|o| o.as_f64()).collect();
                    if !comps.is_empty() {
                        self.state.stroke_color = PdfColor { components: comps };
                    }
                }
            }
            "sc" | "scn" => {
                // Last operand may be a pattern name if fill CS is Pattern
                let last_is_name = operands.last().and_then(|o| o.as_name());
                if last_is_name.is_some() {
                    self.state.fill_pattern_name =
                        last_is_name.map(|n| n.to_vec());
                    // Remaining numeric operands are underlying color components
                    let comps: Vec<f64> = operands.iter().filter_map(|o| o.as_f64()).collect();
                    if !comps.is_empty() {
                        self.state.fill_color = PdfColor { components: comps };
                    }
                } else {
                    let comps: Vec<f64> = operands.iter().filter_map(|o| o.as_f64()).collect();
                    if !comps.is_empty() {
                        self.state.fill_color = PdfColor { components: comps };
                    }
                }
            }
            "G" => {
                self.state.stroke_cs = ColorSpace::DeviceGray;
                self.state.stroke_color = PdfColor::gray(f(operands, 0));
            }
            "g" => {
                self.state.fill_cs = ColorSpace::DeviceGray;
                self.state.fill_color = PdfColor::gray(f(operands, 0));
            }
            "RG" => {
                self.state.stroke_cs = ColorSpace::DeviceRGB;
                self.state.stroke_color =
                    PdfColor::rgb(f(operands, 0), f(operands, 1), f(operands, 2));
            }
            "rg" => {
                self.state.fill_cs = ColorSpace::DeviceRGB;
                self.state.fill_color =
                    PdfColor::rgb(f(operands, 0), f(operands, 1), f(operands, 2));
            }
            "K" => {
                self.state.stroke_cs = ColorSpace::DeviceCMYK;
                self.state.stroke_color = PdfColor::cmyk(
                    f(operands, 0),
                    f(operands, 1),
                    f(operands, 2),
                    f(operands, 3),
                );
            }
            "k" => {
                self.state.fill_cs = ColorSpace::DeviceCMYK;
                self.state.fill_color = PdfColor::cmyk(
                    f(operands, 0),
                    f(operands, 1),
                    f(operands, 2),
                    f(operands, 3),
                );
            }

            // --- Text operators ---
            "BT" => {
                self.state.text_matrix = Matrix::identity();
                self.state.text_line_matrix = Matrix::identity();
            }
            "ET" => {}
            "Tc" => {
                self.state.text.char_spacing = f(operands, 0);
            }
            "Tw" => {
                self.state.text.word_spacing = f(operands, 0);
            }
            "Tz" => {
                self.state.text.horiz_scaling = f(operands, 0) / 100.0;
            }
            "TL" => {
                self.state.text.leading = f(operands, 0);
            }
            "Tf" => {
                if let Some(name) = operands.first().and_then(|o| o.as_name()) {
                    self.state.text.font_name = name.to_vec();
                }
                if operands.len() > 1 {
                    self.state.text.font_size = f(operands, 1);
                }
            }
            "Tr" => {
                self.state.text.render_mode = operands.first().and_then(|o| o.as_i64()).unwrap_or(0);
            }
            "Ts" => {
                self.state.text.text_rise = f(operands, 0);
            }
            "Td" => {
                let tx = f(operands, 0);
                let ty = f(operands, 1);
                let t = Matrix::translate(tx, ty);
                self.state.text_line_matrix = t.concat(&self.state.text_line_matrix);
                self.state.text_matrix = self.state.text_line_matrix;
            }
            "TD" => {
                let tx = f(operands, 0);
                let ty = f(operands, 1);
                self.state.text.leading = -ty;
                let t = Matrix::translate(tx, ty);
                self.state.text_line_matrix = t.concat(&self.state.text_line_matrix);
                self.state.text_matrix = self.state.text_line_matrix;
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
                    self.state.text_matrix = m;
                    self.state.text_line_matrix = m;
                }
            }
            "T*" => {
                let leading = self.state.text.leading;
                let t = Matrix::translate(0.0, -leading);
                self.state.text_line_matrix = t.concat(&self.state.text_line_matrix);
                self.state.text_matrix = self.state.text_line_matrix;
            }
            "Tj" => {
                if let Some(s) = operands.first().and_then(|o| o.as_str()) {
                    self.render_text_string(s)?;
                }
            }
            "TJ" => {
                if let Some(arr) = operands.first().and_then(|o| o.as_array()) {
                    for item in arr {
                        match item {
                            Operand::String(s) => {
                                self.render_text_string(s)?;
                            }
                            Operand::Integer(n) => {
                                self.adjust_text_position(*n as f64);
                            }
                            Operand::Real(n) => {
                                self.adjust_text_position(*n);
                            }
                            _ => {}
                        }
                    }
                }
            }
            "'" => {
                // Move to next line, show string
                let leading = self.state.text.leading;
                let t = Matrix::translate(0.0, -leading);
                self.state.text_line_matrix = t.concat(&self.state.text_line_matrix);
                self.state.text_matrix = self.state.text_line_matrix;
                if let Some(s) = operands.first().and_then(|o| o.as_str()) {
                    self.render_text_string(s)?;
                }
            }
            "\"" => {
                // Set word/char spacing, move to next line, show string
                if operands.len() >= 3 {
                    self.state.text.word_spacing = f(operands, 0);
                    self.state.text.char_spacing = f(operands, 1);
                    let leading = self.state.text.leading;
                    let t = Matrix::translate(0.0, -leading);
                    self.state.text_line_matrix = t.concat(&self.state.text_line_matrix);
                    self.state.text_matrix = self.state.text_line_matrix;
                    if let Some(s) = operands.get(2).and_then(|o| o.as_str()) {
                        self.render_text_string(s)?;
                    }
                }
            }

            // --- XObject (images and forms) ---
            "Do" => {
                if let Some(name) = operands.first().and_then(|o| o.as_name()) {
                    self.do_xobject(name, page)?;
                }
            }

            // --- Inline image ---
            "BI" => {
                if let Some(Operand::InlineImage { dict, data }) = operands.first() {
                    self.render_inline_image(dict, data)?;
                }
            }

            // --- Marked content (ignore) ---
            "BMC" | "BDC" | "EMC" | "MP" | "DP" => {}

            // --- Shading ---
            "sh" => {
                if let Some(name) = operands.first().and_then(|o| o.as_name()) {
                    self.render_shading(name, page)?;
                }
            }

            // --- Type3 font ---
            "d0" | "d1" => {}

            // --- Compatibility ---
            "BX" | "EX" => {}

            // Unknown operator — ignore
            _ => {}
        }

        Ok(())
    }

    // --- Path rendering helpers ---

    fn effective_transform(&self) -> Transform {
        self.state.ctm.concat(&self.page_transform).to_skia()
    }

    fn blend_mode(&self) -> tiny_skia::BlendMode {
        self.state.blend_mode.to_skia()
    }

    fn fill_current_path(&mut self, rule: FillRule, page: &PageInfo) {
        if let Some(pb) = self.path_builder.take() {
            if let Some(path) = pb.finish() {
                // Check for pattern fill
                if self.state.fill_pattern_name.is_some() {
                    if self.try_fill_with_pattern(&path, rule, page) {
                        return;
                    }
                }
                let transform = self.effective_transform();
                let color = self.state.fill_color_rgba();
                let bm = self.blend_mode();
                self.apply_soft_mask_to_device();
                self.device.fill_path(&path, rule, transform, color, bm);
                self.restore_clip_after_soft_mask();
            }
        }
    }

    fn fill_current_path_keep(&mut self, rule: FillRule, page: &PageInfo) {
        if let Some(pb) = &self.path_builder {
            let pb_clone = pb.clone();
            if let Some(path) = pb_clone.finish() {
                // Check for pattern fill
                if self.state.fill_pattern_name.is_some() {
                    if self.try_fill_with_pattern(&path, rule, page) {
                        return;
                    }
                }
                let transform = self.effective_transform();
                let color = self.state.fill_color_rgba();
                let bm = self.blend_mode();
                self.apply_soft_mask_to_device();
                self.device.fill_path(&path, rule, transform, color, bm);
                self.restore_clip_after_soft_mask();
            }
        }
    }

    fn stroke_current_path(&mut self, page: &PageInfo) {
        if let Some(pb) = self.path_builder.take() {
            if let Some(path) = pb.finish() {
                // Check for pattern stroke
                if self.state.stroke_pattern_name.is_some() {
                    if self.try_stroke_with_pattern(&path, page) {
                        return;
                    }
                }
                let transform = self.effective_transform();
                let color = self.state.stroke_color_rgba();
                let bm = self.blend_mode();
                self.apply_soft_mask_to_device();
                self.device.stroke_path(&path, transform, color, &self.state, bm);
                self.restore_clip_after_soft_mask();
            }
        }
    }

    /// If a soft mask is active, combine it with the current clip mask
    /// so that drawing operations are masked accordingly.
    fn apply_soft_mask_to_device(&mut self) {
        if let Some(ref soft_mask) = self.state.soft_mask {
            // Intersect the soft mask with the existing clip mask
            if let Some(ref existing_clip) = self.device.clip_mask {
                // Combine: for each pixel, min(existing_clip, soft_mask)
                let mut combined = existing_clip.clone();
                let combined_data = combined.data_mut();
                let mask_data = soft_mask.mask.data();
                let len = combined_data.len().min(mask_data.len());
                for i in 0..len {
                    combined_data[i] =
                        ((combined_data[i] as u16 * mask_data[i] as u16) / 255) as u8;
                }
                self.device.clip_mask = Some(combined);
            } else {
                self.device.clip_mask = Some(soft_mask.mask.clone());
            }
        }
    }

    /// Restore the clip mask after soft mask application (remove the soft mask contribution).
    fn restore_clip_after_soft_mask(&mut self) {
        if self.state.soft_mask.is_some() {
            // Restore original clip (without the soft mask merged in)
            // We need to reconstruct just the clip path mask.
            // For simplicity, if we had a clip before, we need to re-establish it.
            // Since we don't track the clip path separately, we'll just leave
            // the combined mask in place. The Q operator will restore properly.
            // This is acceptable because soft masks are typically used within
            // a q/Q pair.
        }
    }

    fn apply_clip(&mut self, rule: FillRule) {
        if let Some(pb) = &self.path_builder {
            let pb_clone = pb.clone();
            if let Some(path) = pb_clone.finish() {
                let transform = self.effective_transform();
                if self.state.has_clip {
                    self.device.intersect_clip_path(&path, rule, transform);
                } else {
                    self.device.set_clip_path(&path, rule, transform);
                }
                self.state.has_clip = true;
            }
        }
    }

    // --- Text rendering ---

    fn render_text_string(&mut self, string_bytes: &[u8]) -> Result<()> {
        let font_name = self.state.text.font_name.clone();
        let font = match self.fonts.get(&font_name) {
            Some(f) => f,
            None => return Ok(()), // font not found, skip
        };

        let font_size = self.state.text.font_size;
        let horiz_scaling = self.state.text.horiz_scaling;
        let char_spacing = self.state.text.char_spacing;
        let word_spacing = self.state.text.word_spacing;
        let text_rise = self.state.text.text_rise;
        let render_mode = self.state.text.render_mode;

        // Determine if this is a 2-byte CID font
        let is_cid = font.info.subtype == b"Type0";

        // Pre-compute char codes, widths, and font data while borrowing fonts immutably
        let char_codes: Vec<u32> = if is_cid {
            string_bytes
                .chunks(2)
                .map(|c| {
                    if c.len() == 2 {
                        ((c[0] as u32) << 8) | (c[1] as u32)
                    } else {
                        c[0] as u32
                    }
                })
                .collect()
        } else {
            string_bytes.iter().map(|b| *b as u32).collect()
        };

        let widths: Vec<f64> = char_codes
            .iter()
            .map(|code| font.info.widths.get_width(*code))
            .collect();

        // Clone font_data reference for glyph outline rendering
        let font_data = font.font_data.clone();

        // Now we're done borrowing self.fonts, can mutably borrow self
        for (i, code) in char_codes.iter().enumerate() {
            let width = widths[i];
            let w0 = width / 1000.0;

            if render_mode != 3 {
                self.render_glyph(*code, w0 * font_size, font_size, text_rise, is_cid, &font_data)?;
            }

            // Advance text matrix
            let tx = (w0 * font_size + char_spacing) * horiz_scaling;
            let tx = if *code == 32 {
                tx + word_spacing * horiz_scaling
            } else {
                tx
            };

            let advance = Matrix::translate(tx, 0.0);
            self.state.text_matrix = advance.concat(&self.state.text_matrix);
        }

        Ok(())
    }

    fn render_glyph(
        &mut self,
        code: u32,
        glyph_width: f64,
        font_size: f64,
        text_rise: f64,
        is_cid: bool,
        font_data: &Option<Vec<u8>>,
    ) -> Result<()> {
        if glyph_width.abs() < 0.001 {
            return Ok(());
        }

        // Try to render with real glyph outlines
        if let Some(data) = font_data {
            if let Ok(face) = ttf_parser::Face::parse(data, 0) {
                let glyph_id = if is_cid {
                    // For CID fonts, the code is often a CID — try direct GID mapping
                    ttf_parser::GlyphId(code as u16)
                } else {
                    crate::glyph::char_code_to_glyph_id(&face, code)
                };

                if let Some(path) = crate::glyph::glyph_outline(&face, glyph_id) {
                    let upem = crate::glyph::units_per_em(&face);
                    if upem > 0.0 {
                        // Glyph coordinates are in font units.
                        // Scale: font_size / upem, and flip Y (font Y is up, PDF text Y is up too
                        // but we apply the text matrix which handles the rest)
                        let scale = font_size / upem;
                        let glyph_matrix = Matrix {
                            a: scale,
                            b: 0.0,
                            c: 0.0,
                            d: scale, // no Y flip here — glyph coords have Y-up, matching PDF
                            e: 0.0,
                            f: text_rise,
                        };

                        let text_rendering_matrix = glyph_matrix
                            .concat(&self.state.text_matrix)
                            .concat(&self.state.ctm)
                            .concat(&self.page_transform);

                        let transform = text_rendering_matrix.to_skia();
                        let color = self.state.fill_color_rgba();
                        let bm = self.blend_mode();
                        self.device
                            .fill_path(&path, FillRule::Winding, transform, color, bm);
                        return Ok(());
                    }
                }
            }
        }

        // Fallback: render a placeholder rectangle
        let text_rendering_matrix = self
            .state
            .text_matrix
            .concat(&self.state.ctm)
            .concat(&self.page_transform);

        let mut pb = PathBuilder::new();
        let x = 0.0_f32;
        let y = (text_rise - font_size * 0.2) as f32;
        let w = glyph_width as f32;
        let h = font_size as f32 * 0.8;
        pb.move_to(x, y);
        pb.line_to(x + w, y);
        pb.line_to(x + w, y + h);
        pb.line_to(x, y + h);
        pb.close();

        if let Some(path) = pb.finish() {
            let transform = text_rendering_matrix.to_skia();
            let color = self.state.fill_color_rgba();
            let bm = self.blend_mode();
            self.device
                .fill_path(&path, FillRule::Winding, transform, color, bm);
        }

        Ok(())
    }

    fn adjust_text_position(&mut self, amount: f64) {
        // TJ adjustment: negative = move right, positive = move left
        let font_size = self.state.text.font_size;
        let horiz_scaling = self.state.text.horiz_scaling;
        let tx = -amount / 1000.0 * font_size * horiz_scaling;
        let advance = Matrix::translate(tx, 0.0);
        self.state.text_matrix = advance.concat(&self.state.text_matrix);
    }

    // --- XObject rendering ---

    fn do_xobject(&mut self, name: &[u8], page: &PageInfo) -> Result<()> {
        let xobj = self.resolve_xobject(name, page)?;
        let xobj = match xobj {
            Some(x) => x,
            None => return Ok(()),
        };

        match xobj {
            XObjectData::Image { dict, data } => {
                let _ = self.render_image(&dict, &data); // skip broken images
            }
            XObjectData::Form { dict, data } => {
                if self.xobject_depth > 10 {
                    return Ok(()); // prevent infinite recursion
                }
                self.xobject_depth += 1;
                let _ = self.render_form_xobject(&dict, &data, page);
                self.xobject_depth -= 1;
            }
        }

        Ok(())
    }

    fn resolve_xobject(&mut self, name: &[u8], page: &PageInfo) -> Result<Option<XObjectData>> {
        let resources_obj = match &page.resources_ref {
            Some(obj) => self.resolve_object(obj)?,
            None => return Ok(None),
        };

        let resources_dict = match &resources_obj {
            PdfObject::Dict(d) => d.clone(),
            _ => return Ok(None),
        };

        let xobject_dict_obj = match resources_dict.get(b"XObject") {
            Some(PdfObject::Dict(d)) => PdfObject::Dict(d.clone()),
            Some(PdfObject::Reference(r)) => {
                let r = r.clone();
                self.doc.resolve(&r)?.clone()
            }
            _ => return Ok(None),
        };

        let xobject_dict = match &xobject_dict_obj {
            PdfObject::Dict(d) => d,
            _ => return Ok(None),
        };

        let xobj_ref = match xobject_dict.get(name) {
            Some(PdfObject::Reference(r)) => r.clone(),
            _ => return Ok(None),
        };

        let xobj = self.doc.resolve(&xobj_ref)?.clone();

        match xobj {
            PdfObject::Stream { dict, data } => {
                let subtype = dict.get_name(b"Subtype").unwrap_or(b"");
                match subtype {
                    b"Image" => {
                        // For JPEG, pass raw data
                        let filter = dict.get(b"Filter").and_then(|o| o.as_name());
                        let image_data = if filter == Some(b"DCTDecode") {
                            data.clone()
                        } else {
                            match self.doc.decode_stream(&dict, &data) {
                                Ok(d) => d,
                                Err(_) => return Ok(None),
                            }
                        };
                        Ok(Some(XObjectData::Image {
                            dict,
                            data: image_data,
                        }))
                    }
                    b"Form" => {
                        match self.doc.decode_stream(&dict, &data) {
                            Ok(decoded) => Ok(Some(XObjectData::Form {
                                dict,
                                data: decoded,
                            })),
                            Err(_) => Ok(None),
                        }
                    }
                    _ => Ok(None),
                }
            }
            _ => Ok(None),
        }
    }

    fn render_image(&mut self, dict: &PdfDict, data: &[u8]) -> Result<()> {
        let decoded = image::decode_image(data, dict).map_err(|e| RenderError::Core(e))?;

        // Convert decoded image to RGBA
        let rgba_data = image_to_rgba(&decoded);
        let w = decoded.width;
        let h = decoded.height;

        let img_pixmap = match tiny_skia::Pixmap::from_vec(rgba_data, tiny_skia::IntSize::from_wh(w, h).unwrap()) {
            Some(p) => p,
            None => return Ok(()),
        };

        // PDF images are placed in a 1x1 unit square, scaled by the CTM
        // The CTM should already include the scaling to position the image
        let image_transform = Matrix {
            a: 1.0 / w as f64,
            b: 0.0,
            c: 0.0,
            d: -1.0 / h as f64, // flip Y (PDF images are top-down)
            e: 0.0,
            f: 1.0,
        };

        let full_transform = image_transform
            .concat(&self.state.ctm)
            .concat(&self.page_transform);

        let bm = self.blend_mode();
        self.device.draw_image(
            &img_pixmap.as_ref(),
            full_transform.to_skia(),
            self.state.fill_alpha as f32,
            bm,
        );

        Ok(())
    }

    fn render_inline_image(
        &mut self,
        _dict: &[(Vec<u8>, Operand)],
        _data: &[u8],
    ) -> Result<()> {
        // TODO: implement inline image rendering
        Ok(())
    }

    fn render_shading(&mut self, name: &[u8], page: &PageInfo) -> Result<()> {
        let resources_obj = match &page.resources_ref {
            Some(obj) => self.resolve_object(obj)?,
            None => return Ok(()),
        };

        let resources_dict = match &resources_obj {
            PdfObject::Dict(d) => d.clone(),
            _ => return Ok(()),
        };

        let shading_dict_obj = match resources_dict.get(b"Shading") {
            Some(PdfObject::Dict(d)) => PdfObject::Dict(d.clone()),
            Some(PdfObject::Reference(r)) => {
                let r = r.clone();
                self.doc.resolve(&r)?.clone()
            }
            _ => return Ok(()),
        };

        let shading_container = match &shading_dict_obj {
            PdfObject::Dict(d) => d,
            _ => return Ok(()),
        };

        let sh_obj = match shading_container.get(name) {
            Some(PdfObject::Reference(r)) => {
                let r = r.clone();
                self.doc.resolve(&r)?.clone()
            }
            Some(other) => other.clone(),
            None => return Ok(()),
        };

        if let PdfObject::Dict(sh_dict) = &sh_obj {
            // Resolve function if it's a reference
            let mut resolved_dict = sh_dict.clone();
            if let Some(PdfObject::Reference(func_ref)) = sh_dict.get(b"Function") {
                let func_ref = func_ref.clone();
                if let Ok(func_obj) = self.doc.resolve(&func_ref) {
                    resolved_dict.insert(b"Function".to_vec(), func_obj.clone());
                }
            }

            let clip = self.device.clip_mask.as_ref();
            crate::shading::render_shading(
                &mut self.device.pixmap,
                &resolved_dict,
                &self.state.ctm,
                &self.page_transform,
                clip,
            );
        }

        Ok(())
    }

    fn render_form_xobject(
        &mut self,
        dict: &PdfDict,
        data: &[u8],
        page: &PageInfo,
    ) -> Result<()> {
        // Check if this form has a transparency group
        let has_transparency_group = dict
            .get(b"Group")
            .and_then(|o| match o {
                PdfObject::Dict(d) => Some(d),
                _ => None,
            })
            .map(|group| group.get_name(b"S") == Some(b"Transparency"))
            .unwrap_or(false);

        if has_transparency_group {
            return self.render_transparency_group(dict, data, page);
        }

        // Save state
        self.state_stack.push(self.state.clone());

        // Apply form matrix if present
        if let Some(matrix_arr) = dict.get_array(b"Matrix") {
            if matrix_arr.len() >= 6 {
                let m = Matrix {
                    a: matrix_arr[0].as_f64().unwrap_or(1.0),
                    b: matrix_arr[1].as_f64().unwrap_or(0.0),
                    c: matrix_arr[2].as_f64().unwrap_or(0.0),
                    d: matrix_arr[3].as_f64().unwrap_or(1.0),
                    e: matrix_arr[4].as_f64().unwrap_or(0.0),
                    f: matrix_arr[5].as_f64().unwrap_or(0.0),
                };
                self.state.ctm = m.concat(&self.state.ctm);
            }
        }

        // Parse and execute the form's content stream
        let ops = parse_content_stream(data).map_err(|e| RenderError::Core(e))?;
        self.execute_ops(&ops, page)?;

        // Restore state
        if let Some(s) = self.state_stack.pop() {
            self.state = s;
        }

        Ok(())
    }

    /// Render a transparency group: draw content into a temporary pixmap,
    /// then composite onto the main device.
    fn render_transparency_group(
        &mut self,
        dict: &PdfDict,
        data: &[u8],
        page: &PageInfo,
    ) -> Result<()> {
        let w = self.device.pixmap.width();
        let h = self.device.pixmap.height();

        // Create temporary pixmap for the group
        let mut temp_pixmap = match Pixmap::new(w, h) {
            Some(p) => p,
            None => {
                // Fallback: render directly (non-grouped)
                return self.render_form_xobject_direct(dict, data, page);
            }
        };

        // Check if group is isolated (starts with transparent backdrop)
        let is_isolated = dict
            .get(b"Group")
            .and_then(|o| match o {
                PdfObject::Dict(d) => Some(d),
                _ => None,
            })
            .and_then(|group| group.get(b"I"))
            .and_then(|o| o.as_bool())
            .unwrap_or(false);

        // If not isolated, copy the current pixmap as backdrop
        if !is_isolated {
            temp_pixmap
                .data_mut()
                .copy_from_slice(self.device.pixmap.data());
        }

        // Swap in the temp pixmap
        let saved_clip = self.device.clip_mask.take();
        std::mem::swap(&mut self.device.pixmap, &mut temp_pixmap);

        // Save state
        self.state_stack.push(self.state.clone());

        // Apply form matrix if present
        if let Some(matrix_arr) = dict.get_array(b"Matrix") {
            if matrix_arr.len() >= 6 {
                let m = Matrix {
                    a: matrix_arr[0].as_f64().unwrap_or(1.0),
                    b: matrix_arr[1].as_f64().unwrap_or(0.0),
                    c: matrix_arr[2].as_f64().unwrap_or(0.0),
                    d: matrix_arr[3].as_f64().unwrap_or(1.0),
                    e: matrix_arr[4].as_f64().unwrap_or(0.0),
                    f: matrix_arr[5].as_f64().unwrap_or(0.0),
                };
                self.state.ctm = m.concat(&self.state.ctm);
            }
        }

        // Parse and execute the form's content stream into temp pixmap
        let ops = parse_content_stream(data).map_err(|e| RenderError::Core(e))?;
        let _ = self.execute_ops(&ops, page);

        // Restore state
        if let Some(s) = self.state_stack.pop() {
            self.state = s;
        }

        // Swap back: temp_pixmap now has the group content, device has original
        std::mem::swap(&mut self.device.pixmap, &mut temp_pixmap);
        self.device.clip_mask = saved_clip;

        // Composite the group result onto the main pixmap
        let alpha = self.state.fill_alpha as f32;
        let bm = self.blend_mode();
        self.device.draw_pixmap(
            &temp_pixmap.as_ref(),
            Transform::identity(),
            alpha,
            bm,
        );

        Ok(())
    }

    /// Render a form xobject directly (without transparency group handling).
    /// Used as fallback when temp pixmap creation fails.
    fn render_form_xobject_direct(
        &mut self,
        dict: &PdfDict,
        data: &[u8],
        page: &PageInfo,
    ) -> Result<()> {
        self.state_stack.push(self.state.clone());

        if let Some(matrix_arr) = dict.get_array(b"Matrix") {
            if matrix_arr.len() >= 6 {
                let m = Matrix {
                    a: matrix_arr[0].as_f64().unwrap_or(1.0),
                    b: matrix_arr[1].as_f64().unwrap_or(0.0),
                    c: matrix_arr[2].as_f64().unwrap_or(0.0),
                    d: matrix_arr[3].as_f64().unwrap_or(1.0),
                    e: matrix_arr[4].as_f64().unwrap_or(0.0),
                    f: matrix_arr[5].as_f64().unwrap_or(0.0),
                };
                self.state.ctm = m.concat(&self.state.ctm);
            }
        }

        let ops = parse_content_stream(data).map_err(|e| RenderError::Core(e))?;
        self.execute_ops(&ops, page)?;

        if let Some(s) = self.state_stack.pop() {
            self.state = s;
        }

        Ok(())
    }

    fn apply_extgstate(&mut self, name: &[u8], page: &PageInfo) -> Result<()> {
        let resources_obj = match &page.resources_ref {
            Some(obj) => self.resolve_object(obj)?,
            None => return Ok(()),
        };

        let resources_dict = match &resources_obj {
            PdfObject::Dict(d) => d.clone(),
            _ => return Ok(()),
        };

        let extgstate_dict_obj = match resources_dict.get(b"ExtGState") {
            Some(PdfObject::Dict(d)) => PdfObject::Dict(d.clone()),
            Some(PdfObject::Reference(r)) => {
                let r = r.clone();
                self.doc.resolve(&r)?.clone()
            }
            _ => return Ok(()),
        };

        let extgstate_dict = match &extgstate_dict_obj {
            PdfObject::Dict(d) => d,
            _ => return Ok(()),
        };

        let gs_obj = match extgstate_dict.get(name) {
            Some(PdfObject::Reference(r)) => {
                let r = r.clone();
                self.doc.resolve(&r)?.clone()
            }
            Some(other) => other.clone(),
            None => return Ok(()),
        };

        if let PdfObject::Dict(gs_dict) = &gs_obj {
            if let Some(lw) = gs_dict.get(b"LW").and_then(|o| o.as_f64()) {
                self.state.line_width = lw;
            }
            if let Some(lc) = gs_dict.get(b"LC").and_then(|o| o.as_i64()) {
                self.state.line_cap = match lc {
                    1 => LineCap::Round,
                    2 => LineCap::Square,
                    _ => LineCap::Butt,
                };
            }
            if let Some(lj) = gs_dict.get(b"LJ").and_then(|o| o.as_i64()) {
                self.state.line_join = match lj {
                    1 => LineJoin::Round,
                    2 => LineJoin::Bevel,
                    _ => LineJoin::Miter,
                };
            }
            if let Some(ml) = gs_dict.get(b"ML").and_then(|o| o.as_f64()) {
                self.state.miter_limit = ml;
            }
            // Fill alpha (ca)
            if let Some(a) = gs_dict.get(b"ca").and_then(|o| o.as_f64()) {
                self.state.fill_alpha = a;
            }
            // Stroke alpha (CA)
            if let Some(a) = gs_dict.get(b"CA").and_then(|o| o.as_f64()) {
                self.state.stroke_alpha = a;
            }
            // Blend mode (BM)
            if let Some(bm_name) = gs_dict.get(b"BM").and_then(|o| o.as_name()) {
                self.state.blend_mode = PdfBlendMode::from_name(bm_name);
            }

            // Soft mask (SMask)
            match gs_dict.get(b"SMask") {
                Some(PdfObject::Name(n)) if n == b"None" => {
                    self.state.soft_mask = None;
                }
                Some(PdfObject::Dict(smask_dict)) => {
                    let _ = self.apply_soft_mask(smask_dict.clone(), page);
                }
                _ => {}
            }
        }

        Ok(())
    }

    /// Resolve and render a soft mask from an SMask dictionary.
    fn apply_soft_mask(&mut self, smask_dict: PdfDict, page: &PageInfo) -> Result<()> {
        // /S: Luminosity or Alpha
        let subtype = match smask_dict.get_name(b"S") {
            Some(b"Luminosity") => SoftMaskSubtype::Luminosity,
            Some(b"Alpha") => SoftMaskSubtype::Alpha,
            _ => return Ok(()), // unsupported subtype
        };

        // /G: form XObject reference for the mask
        let form_obj = match smask_dict.get(b"G") {
            Some(PdfObject::Reference(r)) => {
                let r = r.clone();
                match self.doc.resolve(&r) {
                    Ok(obj) => obj.clone(),
                    Err(_) => return Ok(()),
                }
            }
            Some(other) => other.clone(),
            None => return Ok(()),
        };

        let (form_dict, form_data) = match form_obj {
            PdfObject::Stream { dict, data } => {
                match self.doc.decode_stream(&dict, &data) {
                    Ok(decoded) => (dict, decoded),
                    Err(_) => return Ok(()),
                }
            }
            _ => return Ok(()),
        };

        let w = self.device.pixmap.width();
        let h = self.device.pixmap.height();

        // Create a temporary pixmap to render the mask form into
        let mut mask_pixmap = match Pixmap::new(w, h) {
            Some(p) => p,
            None => return Ok(()),
        };

        // For luminosity masks, initialize to white background if /BC is specified
        if subtype == SoftMaskSubtype::Luminosity {
            // Default backdrop: black (which means mask = 0, fully transparent)
            // Some PDFs specify /BC (backdrop color) but we use black as default
            mask_pixmap.fill(tiny_skia::Color::BLACK);
        }

        // Swap in the mask pixmap for rendering
        let saved_clip = self.device.clip_mask.take();
        std::mem::swap(&mut self.device.pixmap, &mut mask_pixmap);

        // Save and reset state for mask rendering
        self.state_stack.push(self.state.clone());

        // Apply form matrix if present
        if let Some(matrix_arr) = form_dict.get_array(b"Matrix") {
            if matrix_arr.len() >= 6 {
                let m = Matrix {
                    a: matrix_arr[0].as_f64().unwrap_or(1.0),
                    b: matrix_arr[1].as_f64().unwrap_or(0.0),
                    c: matrix_arr[2].as_f64().unwrap_or(0.0),
                    d: matrix_arr[3].as_f64().unwrap_or(1.0),
                    e: matrix_arr[4].as_f64().unwrap_or(0.0),
                    f: matrix_arr[5].as_f64().unwrap_or(0.0),
                };
                self.state.ctm = m.concat(&self.state.ctm);
            }
        }

        // Render the mask form
        if let Ok(ops) = parse_content_stream(&form_data) {
            let _ = self.execute_ops(&ops, page);
        }

        // Restore state
        if let Some(s) = self.state_stack.pop() {
            self.state = s;
        }

        // Swap back
        std::mem::swap(&mut self.device.pixmap, &mut mask_pixmap);
        self.device.clip_mask = saved_clip;

        // Convert the rendered mask pixmap to a tiny_skia::Mask
        if let Some(mask) = self.pixmap_to_mask(&mask_pixmap, subtype) {
            self.state.soft_mask = Some(SoftMask { mask, subtype });
        }

        Ok(())
    }

    /// Convert a rendered pixmap to a Mask based on luminosity or alpha.
    fn pixmap_to_mask(&self, pixmap: &Pixmap, subtype: SoftMaskSubtype) -> Option<Mask> {
        let w = pixmap.width();
        let h = pixmap.height();
        let mut mask = Mask::new(w, h)?;
        let mask_data = mask.data_mut();
        let src_data = pixmap.data();

        for i in 0..(w * h) as usize {
            let idx = i * 4;
            if idx + 3 >= src_data.len() {
                break;
            }
            let value = match subtype {
                SoftMaskSubtype::Luminosity => {
                    // Convert RGB to luminosity: 0.2126*R + 0.7152*G + 0.0722*B
                    let r = src_data[idx] as f32 / 255.0;
                    let g = src_data[idx + 1] as f32 / 255.0;
                    let b = src_data[idx + 2] as f32 / 255.0;
                    (0.2126 * r + 0.7152 * g + 0.0722 * b).clamp(0.0, 1.0) * 255.0
                }
                SoftMaskSubtype::Alpha => {
                    src_data[idx + 3] as f32
                }
            };
            mask_data[i] = value as u8;
        }

        Some(mask)
    }

    // --- Pattern rendering ---

    /// Resolve a pattern from page resources.
    fn resolve_pattern(&mut self, name: &[u8], page: &PageInfo) -> Result<Option<PdfObject>> {
        let resources_obj = match &page.resources_ref {
            Some(obj) => self.resolve_object(obj)?,
            None => return Ok(None),
        };

        let resources_dict = match &resources_obj {
            PdfObject::Dict(d) => d.clone(),
            _ => return Ok(None),
        };

        let pattern_dict_obj = match resources_dict.get(b"Pattern") {
            Some(PdfObject::Dict(d)) => PdfObject::Dict(d.clone()),
            Some(PdfObject::Reference(r)) => {
                let r = r.clone();
                self.doc.resolve(&r)?.clone()
            }
            _ => return Ok(None),
        };

        let pattern_dict = match &pattern_dict_obj {
            PdfObject::Dict(d) => d,
            _ => return Ok(None),
        };

        match pattern_dict.get(name) {
            Some(PdfObject::Reference(r)) => {
                let r = r.clone();
                Ok(Some(self.doc.resolve(&r)?.clone()))
            }
            Some(other) => Ok(Some(other.clone())),
            None => Ok(None),
        }
    }

    /// Render a tiling pattern cell and return the pixmap.
    fn render_tiling_pattern(
        &mut self,
        pattern_dict: &PdfDict,
        pattern_data: &[u8],
        page: &PageInfo,
    ) -> Result<Option<Pixmap>> {
        let xstep = pattern_dict
            .get(b"XStep")
            .and_then(|o| o.as_f64())
            .unwrap_or(1.0)
            .abs();
        let ystep = pattern_dict
            .get(b"YStep")
            .and_then(|o| o.as_f64())
            .unwrap_or(1.0)
            .abs();

        if xstep < 1.0 || ystep < 1.0 {
            return Ok(None);
        }

        // Compute effective scale from CTM + page transform
        let effective = self.state.ctm.concat(&self.page_transform);
        let sx = (effective.a * effective.a + effective.b * effective.b)
            .sqrt()
            .abs();
        let sy = (effective.c * effective.c + effective.d * effective.d)
            .sqrt()
            .abs();

        // Pattern cell size in device pixels
        let cell_w = (xstep * sx).ceil().max(1.0).min(2048.0) as u32;
        let cell_h = (ystep * sy).ceil().max(1.0).min(2048.0) as u32;

        let mut cell_pixmap = match Pixmap::new(cell_w, cell_h) {
            Some(p) => p,
            None => return Ok(None),
        };

        // Pattern matrix (from pattern dict)
        let pattern_matrix = if let Some(matrix_arr) = pattern_dict.get_array(b"Matrix") {
            if matrix_arr.len() >= 6 {
                Matrix {
                    a: matrix_arr[0].as_f64().unwrap_or(1.0),
                    b: matrix_arr[1].as_f64().unwrap_or(0.0),
                    c: matrix_arr[2].as_f64().unwrap_or(0.0),
                    d: matrix_arr[3].as_f64().unwrap_or(1.0),
                    e: matrix_arr[4].as_f64().unwrap_or(0.0),
                    f: matrix_arr[5].as_f64().unwrap_or(0.0),
                }
            } else {
                Matrix::identity()
            }
        } else {
            Matrix::identity()
        };

        // Build the transform for rendering the pattern cell:
        // Pattern coords -> pattern matrix -> scale to device pixels
        let scale_to_device = Matrix::scale(
            cell_w as f64 / xstep,
            cell_h as f64 / ystep,
        );
        let cell_transform = pattern_matrix.concat(&scale_to_device);

        // Swap in the cell pixmap
        let saved_clip = self.device.clip_mask.take();
        std::mem::swap(&mut self.device.pixmap, &mut cell_pixmap);

        // Save state for pattern rendering
        self.state_stack.push(self.state.clone());
        let saved_page_transform = self.page_transform;

        // Set up state for rendering into the cell
        self.state.ctm = Matrix::identity();
        self.page_transform = cell_transform;

        // Render the pattern content stream
        if let Ok(ops) = parse_content_stream(pattern_data) {
            let _ = self.execute_ops(&ops, page);
        }

        // Restore state
        self.page_transform = saved_page_transform;
        if let Some(s) = self.state_stack.pop() {
            self.state = s;
        }

        // Swap back
        std::mem::swap(&mut self.device.pixmap, &mut cell_pixmap);
        self.device.clip_mask = saved_clip;

        Ok(Some(cell_pixmap))
    }

    /// Try to fill a path using the current fill pattern, if one is set.
    /// Returns true if pattern fill was performed.
    fn try_fill_with_pattern(&mut self, path: &tiny_skia::Path, rule: FillRule, page: &PageInfo) -> bool {
        let pattern_name = match &self.state.fill_pattern_name {
            Some(name) => name.clone(),
            None => return false,
        };

        if let Ok(Some(pattern_pixmap)) = self.resolve_and_render_pattern(&pattern_name, page) {
            let transform = self.effective_transform();
            let bm = self.blend_mode();
            self.device.fill_path_with_pattern(
                path,
                rule,
                transform,
                &pattern_pixmap.as_ref(),
                Transform::identity(),
                bm,
            );
            true
        } else {
            false
        }
    }

    /// Try to stroke a path using the current stroke pattern, if one is set.
    /// Returns true if pattern stroke was performed.
    fn try_stroke_with_pattern(&mut self, path: &tiny_skia::Path, page: &PageInfo) -> bool {
        let pattern_name = match &self.state.stroke_pattern_name {
            Some(name) => name.clone(),
            None => return false,
        };

        if let Ok(Some(pattern_pixmap)) = self.resolve_and_render_pattern(&pattern_name, page) {
            let transform = self.effective_transform();
            let bm = self.blend_mode();
            self.device.stroke_path_with_pattern(
                path,
                transform,
                &self.state,
                &pattern_pixmap.as_ref(),
                Transform::identity(),
                bm,
            );
            true
        } else {
            false
        }
    }

    /// Resolve a pattern by name and render it. Handles both tiling and shading patterns.
    fn resolve_and_render_pattern(
        &mut self,
        name: &[u8],
        page: &PageInfo,
    ) -> Result<Option<Pixmap>> {
        let pattern_obj = match self.resolve_pattern(name, page)? {
            Some(obj) => obj,
            None => return Ok(None),
        };

        match &pattern_obj {
            PdfObject::Stream { dict, data } => {
                let pattern_type = dict.get_i64(b"PatternType").unwrap_or(0);
                match pattern_type {
                    1 => {
                        // Tiling pattern
                        let decoded = match self.doc.decode_stream(dict, data) {
                            Ok(d) => d,
                            Err(_) => return Ok(None),
                        };
                        let dict = dict.clone();
                        self.render_tiling_pattern(&dict, &decoded, page)
                    }
                    2 => {
                        // Shading pattern: render shading into a temp pixmap
                        self.render_shading_pattern(dict, page)
                    }
                    _ => Ok(None),
                }
            }
            PdfObject::Dict(dict) => {
                let pattern_type = dict.get_i64(b"PatternType").unwrap_or(0);
                if pattern_type == 2 {
                    self.render_shading_pattern(dict, page)
                } else {
                    Ok(None)
                }
            }
            _ => Ok(None),
        }
    }

    /// Render a shading pattern (PatternType 2) into a pixmap.
    fn render_shading_pattern(
        &mut self,
        pattern_dict: &PdfDict,
        _page: &PageInfo,
    ) -> Result<Option<Pixmap>> {
        // Get the shading dict from the pattern
        let shading_obj = match pattern_dict.get(b"Shading") {
            Some(PdfObject::Reference(r)) => {
                let r = r.clone();
                match self.doc.resolve(&r) {
                    Ok(obj) => obj.clone(),
                    Err(_) => return Ok(None),
                }
            }
            Some(other) => other.clone(),
            None => return Ok(None),
        };

        let shading_dict = match &shading_obj {
            PdfObject::Dict(d) => d.clone(),
            _ => return Ok(None),
        };

        // Resolve function references within the shading dict
        let mut resolved_shading = shading_dict.clone();
        if let Some(PdfObject::Reference(func_ref)) = shading_dict.get(b"Function") {
            let func_ref = func_ref.clone();
            if let Ok(func_obj) = self.doc.resolve(&func_ref) {
                resolved_shading.insert(b"Function".to_vec(), func_obj.clone());
            }
        }

        let w = self.device.pixmap.width();
        let h = self.device.pixmap.height();

        let mut shading_pixmap = match Pixmap::new(w, h) {
            Some(p) => p,
            None => return Ok(None),
        };

        // Pattern matrix
        let pattern_matrix = if let Some(matrix_arr) = pattern_dict.get_array(b"Matrix") {
            if matrix_arr.len() >= 6 {
                Matrix {
                    a: matrix_arr[0].as_f64().unwrap_or(1.0),
                    b: matrix_arr[1].as_f64().unwrap_or(0.0),
                    c: matrix_arr[2].as_f64().unwrap_or(0.0),
                    d: matrix_arr[3].as_f64().unwrap_or(1.0),
                    e: matrix_arr[4].as_f64().unwrap_or(0.0),
                    f: matrix_arr[5].as_f64().unwrap_or(0.0),
                }
            } else {
                Matrix::identity()
            }
        } else {
            Matrix::identity()
        };

        // The effective CTM for the shading is pattern_matrix * CTM
        let effective_ctm = pattern_matrix.concat(&self.state.ctm);

        let clip = self.device.clip_mask.as_ref();
        crate::shading::render_shading(
            &mut shading_pixmap,
            &resolved_shading,
            &effective_ctm,
            &self.page_transform,
            clip,
        );

        Ok(Some(shading_pixmap))
    }
}

enum XObjectData {
    Image { dict: PdfDict, data: Vec<u8> },
    Form { dict: PdfDict, data: Vec<u8> },
}

/// Convert decoded image data to RGBA bytes.
fn image_to_rgba(img: &image::DecodedImage) -> Vec<u8> {
    let pixel_count = (img.width * img.height) as usize;
    let mut rgba = vec![255u8; pixel_count * 4];

    match img.components {
        1 => {
            // Grayscale
            for i in 0..pixel_count.min(img.data.len()) {
                let g = img.data[i];
                rgba[i * 4] = g;
                rgba[i * 4 + 1] = g;
                rgba[i * 4 + 2] = g;
            }
        }
        3 => {
            // RGB
            for i in 0..pixel_count.min(img.data.len() / 3) {
                rgba[i * 4] = img.data[i * 3];
                rgba[i * 4 + 1] = img.data[i * 3 + 1];
                rgba[i * 4 + 2] = img.data[i * 3 + 2];
            }
        }
        4 => {
            // CMYK → RGB
            for i in 0..pixel_count.min(img.data.len() / 4) {
                let c = img.data[i * 4] as f64 / 255.0;
                let m = img.data[i * 4 + 1] as f64 / 255.0;
                let y = img.data[i * 4 + 2] as f64 / 255.0;
                let k = img.data[i * 4 + 3] as f64 / 255.0;
                rgba[i * 4] = ((1.0 - c) * (1.0 - k) * 255.0) as u8;
                rgba[i * 4 + 1] = ((1.0 - m) * (1.0 - k) * 255.0) as u8;
                rgba[i * 4 + 2] = ((1.0 - y) * (1.0 - k) * 255.0) as u8;
            }
        }
        _ => {
            // Fill with black
            for i in 0..pixel_count {
                rgba[i * 4] = 0;
                rgba[i * 4 + 1] = 0;
                rgba[i * 4 + 2] = 0;
            }
        }
    }

    rgba
}

fn cs_from_name(name: &[u8]) -> ColorSpace {
    match name {
        b"DeviceGray" | b"G" => ColorSpace::DeviceGray,
        b"DeviceRGB" | b"RGB" => ColorSpace::DeviceRGB,
        b"DeviceCMYK" | b"CMYK" => ColorSpace::DeviceCMYK,
        _ => ColorSpace::DeviceRGB, // fallback
    }
}

/// Get f64 from operands at index.
fn f(operands: &[Operand], idx: usize) -> f64 {
    operands.get(idx).and_then(|o| o.as_f64()).unwrap_or(0.0)
}
