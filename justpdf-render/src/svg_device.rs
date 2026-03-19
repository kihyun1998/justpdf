//! SVG output device — converts PDF content stream operations into SVG XML.

use std::collections::HashMap;
use std::fmt::Write as FmtWrite;

use justpdf_core::color::{Color as PdfColor, ColorSpace};
use justpdf_core::content::{ContentOp, Operand, parse_content_stream};
use justpdf_core::font::{FontInfo, ToUnicodeCMap, parse_font_info};
use justpdf_core::image;
use justpdf_core::object::{PdfDict, PdfObject};
use justpdf_core::page::PageInfo;
use justpdf_core::PdfDocument;

use crate::error::{RenderError, Result};
use crate::graphics_state::{
    GraphicsState, LineCap, LineJoin, Matrix, PdfBlendMode,
};

/// Resolved font for SVG rendering.
struct ResolvedFont {
    info: FontInfo,
    cmap: Option<ToUnicodeCMap>,
    #[allow(dead_code)]
    font_data: Option<Vec<u8>>,
}

/// SVG rendering interpreter: walks content stream ops and builds SVG XML.
pub struct SvgRenderer<'a> {
    doc: &'a PdfDocument,
    state: GraphicsState,
    state_stack: Vec<GraphicsState>,
    fonts: HashMap<Vec<u8>, ResolvedFont>,
    /// Transform from PDF user space to SVG space.
    page_transform: Matrix,
    /// Current path being constructed (SVG path data string).
    path_data: Option<String>,
    /// Collected SVG elements (body content).
    elements: Vec<String>,
    /// Collected SVG defs (clip paths, gradients, etc.).
    defs: Vec<String>,
    /// Counter for unique IDs (clip paths, etc.).
    id_counter: u32,
    /// Active clip path ID for the current graphics state.
    active_clip_id: Option<String>,
    /// Stack of clip path IDs.
    clip_id_stack: Vec<Option<String>>,
    /// Form XObject recursion depth limit.
    xobject_depth: u32,
    /// Page dimensions in points.
    page_width: f64,
    page_height: f64,
}

impl<'a> SvgRenderer<'a> {
    pub fn new(
        doc: &'a PdfDocument,
        page_transform: Matrix,
        page_width: f64,
        page_height: f64,
    ) -> Self {
        Self {
            doc,
            state: GraphicsState::default(),
            state_stack: Vec::new(),
            fonts: HashMap::new(),
            page_transform,
            path_data: None,
            elements: Vec::new(),
            defs: Vec::new(),
            id_counter: 0,
            active_clip_id: None,
            clip_id_stack: Vec::new(),
            xobject_depth: 0,
            page_width,
            page_height,
        }
    }

    fn next_id(&mut self, prefix: &str) -> String {
        self.id_counter += 1;
        format!("{}{}", prefix, self.id_counter)
    }

    /// Render a page's content streams and return the SVG XML string.
    pub fn render_page(mut self, page: &PageInfo) -> Result<String> {
        let _ = self.resolve_page_fonts(page);

        let content_data = self.get_page_content(page)?;
        if !content_data.is_empty() {
            let ops = parse_content_stream(&content_data).map_err(RenderError::Core)?;
            self.execute_ops(&ops, page)?;
        }

        Ok(self.build_svg())
    }

    fn build_svg(&self) -> String {
        let mut svg = String::new();
        let _ = write!(
            svg,
            r#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" viewBox="0 0 {} {}" width="{}" height="{}">"#,
            self.page_width, self.page_height, self.page_width, self.page_height
        );

        // White background
        let _ = write!(
            svg,
            r#"
<rect width="{}" height="{}" fill="white"/>"#,
            self.page_width, self.page_height
        );

        if !self.defs.is_empty() {
            svg.push_str("\n<defs>");
            for d in &self.defs {
                svg.push('\n');
                svg.push_str(d);
            }
            svg.push_str("\n</defs>");
        }

        for el in &self.elements {
            svg.push('\n');
            svg.push_str(el);
        }

        svg.push_str("\n</svg>\n");
        svg
    }

    // -----------------------------------------------------------------------
    // Font resolution (mirrors RenderInterpreter)
    // -----------------------------------------------------------------------

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
                self.doc.resolve(&r)?
            }
            _ => return Ok(()),
        };

        if let PdfObject::Dict(font_dict) = &font_dict_obj {
            for (name, val) in font_dict.iter() {
                let font_obj = match val {
                    PdfObject::Reference(r) => {
                        let r = r.clone();
                        self.doc.resolve(&r)?
                    }
                    other => other.clone(),
                };

                if let PdfObject::Dict(fd) = &font_obj {
                    let mut info = parse_font_info(fd);

                    let cmap = if let Some(PdfObject::Reference(tu_ref)) = fd.get(b"ToUnicode") {
                        let tu_ref = tu_ref.clone();
                        if let Ok(tu_obj) = self.doc.resolve(&tu_ref) {
                            if let PdfObject::Stream { dict, data } = tu_obj {
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

                    // Resolve CID font widths for Type0
                    if info.subtype == b"Type0" {
                        if let Some(PdfObject::Array(descendants)) = fd.get(b"DescendantFonts") {
                            if let Some(desc_ref) = descendants.first() {
                                let desc_obj = match desc_ref {
                                    PdfObject::Reference(r) => {
                                        let r = r.clone();
                                        self.doc.resolve(&r)?
                                    }
                                    other => other.clone(),
                                };
                                if let PdfObject::Dict(cid_dict) = &desc_obj {
                                    let cid_info = parse_font_info(cid_dict);
                                    info.widths = cid_info.widths;
                                }
                            }
                        }
                    }

                    self.fonts.insert(
                        name.clone(),
                        ResolvedFont {
                            info,
                            cmap,
                            font_data: None, // SVG uses <text>, not glyph outlines
                        },
                    );
                }
            }
        }

        Ok(())
    }

    fn resolve_object(&mut self, obj: &PdfObject) -> Result<PdfObject> {
        match obj {
            PdfObject::Reference(r) => {
                let r = r.clone();
                Ok(self.doc.resolve(&r)?)
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
                let obj = self.doc.resolve(&r)?;
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
                    self.doc.resolve(&r)?
                }
                other => other.clone(),
            };
            if let PdfObject::Stream { dict, data } = obj {
                if let Ok(decoded) = self.doc.decode_stream(&dict, &data) {
                    combined.extend_from_slice(&decoded);
                    combined.push(b' ');
                }
            }
        }
        Ok(combined)
    }

    // -----------------------------------------------------------------------
    // Operator dispatch
    // -----------------------------------------------------------------------

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
                self.clip_id_stack.push(self.active_clip_id.clone());
            }
            "Q" => {
                if let Some(s) = self.state_stack.pop() {
                    self.state = s;
                }
                if let Some(clip_id) = self.clip_id_stack.pop() {
                    self.active_clip_id = clip_id;
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
                    let _ = self.apply_extgstate(name, page);
                }
            }

            // --- Path construction ---
            "m" => {
                let pd = self.path_data.get_or_insert_with(String::new);
                let _ = write!(pd, "M{} {} ", fmt_f(f(operands, 0)), fmt_f(f(operands, 1)));
            }
            "l" => {
                if let Some(pd) = &mut self.path_data {
                    let _ = write!(pd, "L{} {} ", fmt_f(f(operands, 0)), fmt_f(f(operands, 1)));
                }
            }
            "c" => {
                if let Some(pd) = &mut self.path_data {
                    let _ = write!(
                        pd,
                        "C{} {} {} {} {} {} ",
                        fmt_f(f(operands, 0)),
                        fmt_f(f(operands, 1)),
                        fmt_f(f(operands, 2)),
                        fmt_f(f(operands, 3)),
                        fmt_f(f(operands, 4)),
                        fmt_f(f(operands, 5)),
                    );
                }
            }
            "v" => {
                if let Some(pd) = &mut self.path_data {
                    // 'v': first control point = current point (approximate with same coords)
                    let _ = write!(
                        pd,
                        "C{} {} {} {} {} {} ",
                        fmt_f(f(operands, 0)),
                        fmt_f(f(operands, 1)),
                        fmt_f(f(operands, 0)),
                        fmt_f(f(operands, 1)),
                        fmt_f(f(operands, 2)),
                        fmt_f(f(operands, 3)),
                    );
                }
            }
            "y" => {
                if let Some(pd) = &mut self.path_data {
                    // 'y': second control point = endpoint
                    let _ = write!(
                        pd,
                        "C{} {} {} {} {} {} ",
                        fmt_f(f(operands, 0)),
                        fmt_f(f(operands, 1)),
                        fmt_f(f(operands, 2)),
                        fmt_f(f(operands, 3)),
                        fmt_f(f(operands, 2)),
                        fmt_f(f(operands, 3)),
                    );
                }
            }
            "h" => {
                if let Some(pd) = &mut self.path_data {
                    pd.push_str("Z ");
                }
            }
            "re" => {
                if operands.len() >= 4 {
                    let x = f(operands, 0);
                    let y = f(operands, 1);
                    let w = f(operands, 2);
                    let h = f(operands, 3);
                    let pd = self.path_data.get_or_insert_with(String::new);
                    let _ = write!(
                        pd,
                        "M{} {} L{} {} L{} {} L{} {} Z ",
                        fmt_f(x), fmt_f(y),
                        fmt_f(x + w), fmt_f(y),
                        fmt_f(x + w), fmt_f(y + h),
                        fmt_f(x), fmt_f(y + h),
                    );
                }
            }

            // --- Path painting ---
            "S" => {
                self.stroke_current_path();
            }
            "s" => {
                if let Some(pd) = &mut self.path_data {
                    pd.push_str("Z ");
                }
                self.stroke_current_path();
            }
            "f" | "F" => {
                self.fill_current_path("nonzero");
            }
            "f*" => {
                self.fill_current_path("evenodd");
            }
            "B" => {
                self.fill_and_stroke_path("nonzero");
            }
            "B*" => {
                self.fill_and_stroke_path("evenodd");
            }
            "b" => {
                if let Some(pd) = &mut self.path_data {
                    pd.push_str("Z ");
                }
                self.fill_and_stroke_path("nonzero");
            }
            "b*" => {
                if let Some(pd) = &mut self.path_data {
                    pd.push_str("Z ");
                }
                self.fill_and_stroke_path("evenodd");
            }
            "n" => {
                self.path_data = None;
            }

            // --- Clipping ---
            "W" => {
                self.apply_clip("nonzero");
            }
            "W*" => {
                self.apply_clip("evenodd");
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
                let last_is_name = operands.last().and_then(|o| o.as_name());
                if last_is_name.is_some() {
                    self.state.stroke_pattern_name = last_is_name.map(|n| n.to_vec());
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
                let last_is_name = operands.last().and_then(|o| o.as_name());
                if last_is_name.is_some() {
                    self.state.fill_pattern_name = last_is_name.map(|n| n.to_vec());
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
                self.state.text.render_mode =
                    operands.first().and_then(|o| o.as_i64()).unwrap_or(0);
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
                    self.render_text_string(s);
                }
            }
            "TJ" => {
                if let Some(arr) = operands.first().and_then(|o| o.as_array()) {
                    for item in arr {
                        match item {
                            Operand::String(s) => {
                                self.render_text_string(s);
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
                let leading = self.state.text.leading;
                let t = Matrix::translate(0.0, -leading);
                self.state.text_line_matrix = t.concat(&self.state.text_line_matrix);
                self.state.text_matrix = self.state.text_line_matrix;
                if let Some(s) = operands.first().and_then(|o| o.as_str()) {
                    self.render_text_string(s);
                }
            }
            "\"" => {
                if operands.len() >= 3 {
                    self.state.text.word_spacing = f(operands, 0);
                    self.state.text.char_spacing = f(operands, 1);
                    let leading = self.state.text.leading;
                    let t = Matrix::translate(0.0, -leading);
                    self.state.text_line_matrix = t.concat(&self.state.text_line_matrix);
                    self.state.text_matrix = self.state.text_line_matrix;
                    if let Some(s) = operands.get(2).and_then(|o| o.as_str()) {
                        self.render_text_string(s);
                    }
                }
            }

            // --- XObject (images and forms) ---
            "Do" => {
                if let Some(name) = operands.first().and_then(|o| o.as_name()) {
                    let _ = self.do_xobject(name, page);
                }
            }

            // --- Inline image (skip for SVG) ---
            "BI" => {}

            // --- Marked content (ignore) ---
            "BMC" | "BDC" | "EMC" | "MP" | "DP" => {}

            // --- Shading (skip for now) ---
            "sh" => {}

            // --- Type3 font ---
            "d0" | "d1" => {}

            // --- Compatibility ---
            "BX" | "EX" => {}

            _ => {}
        }

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Path rendering helpers
    // -----------------------------------------------------------------------

    fn effective_transform(&self) -> Matrix {
        self.state.ctm.concat(&self.page_transform)
    }

    fn svg_transform_attr(&self) -> String {
        let m = self.effective_transform();
        format!(
            "transform=\"matrix({},{},{},{},{},{})\"",
            fmt_f(m.a), fmt_f(m.b), fmt_f(m.c), fmt_f(m.d), fmt_f(m.e), fmt_f(m.f)
        )
    }

    fn fill_color_svg(&self) -> String {
        let rgb = self.state.fill_color.to_rgb(&self.state.fill_cs);
        format!(
            "rgb({},{},{})",
            (rgb[0].clamp(0.0, 1.0) * 255.0).round() as u8,
            (rgb[1].clamp(0.0, 1.0) * 255.0).round() as u8,
            (rgb[2].clamp(0.0, 1.0) * 255.0).round() as u8,
        )
    }

    fn stroke_color_svg(&self) -> String {
        let rgb = self.state.stroke_color.to_rgb(&self.state.stroke_cs);
        format!(
            "rgb({},{},{})",
            (rgb[0].clamp(0.0, 1.0) * 255.0).round() as u8,
            (rgb[1].clamp(0.0, 1.0) * 255.0).round() as u8,
            (rgb[2].clamp(0.0, 1.0) * 255.0).round() as u8,
        )
    }

    fn opacity_attrs(&self, for_fill: bool, for_stroke: bool) -> String {
        let mut attrs = String::new();
        if for_fill && self.state.fill_alpha < 1.0 {
            let _ = write!(attrs, " fill-opacity=\"{}\"", fmt_f(self.state.fill_alpha));
        }
        if for_stroke && self.state.stroke_alpha < 1.0 {
            let _ = write!(attrs, " stroke-opacity=\"{}\"", fmt_f(self.state.stroke_alpha));
        }
        attrs
    }

    fn blend_mode_attr(&self) -> String {
        let bm = match self.state.blend_mode {
            PdfBlendMode::Normal => return String::new(),
            PdfBlendMode::Multiply => "multiply",
            PdfBlendMode::Screen => "screen",
            PdfBlendMode::Overlay => "overlay",
            PdfBlendMode::Darken => "darken",
            PdfBlendMode::Lighten => "lighten",
            PdfBlendMode::ColorDodge => "color-dodge",
            PdfBlendMode::ColorBurn => "color-burn",
            PdfBlendMode::HardLight => "hard-light",
            PdfBlendMode::SoftLight => "soft-light",
            PdfBlendMode::Difference => "difference",
            PdfBlendMode::Exclusion => "exclusion",
            PdfBlendMode::Hue => "hue",
            PdfBlendMode::Saturation => "saturation",
            PdfBlendMode::Color => "color",
            PdfBlendMode::Luminosity => "luminosity",
        };
        format!(" style=\"mix-blend-mode:{}\"", bm)
    }

    fn clip_attr(&self) -> String {
        match &self.active_clip_id {
            Some(id) => format!(" clip-path=\"url(#{})\"", id),
            None => String::new(),
        }
    }

    fn stroke_attrs(&self) -> String {
        let mut attrs = String::new();
        let _ = write!(attrs, " stroke-width=\"{}\"", fmt_f(self.state.line_width));
        match self.state.line_cap {
            LineCap::Butt => {}
            LineCap::Round => attrs.push_str(" stroke-linecap=\"round\""),
            LineCap::Square => attrs.push_str(" stroke-linecap=\"square\""),
        }
        match self.state.line_join {
            LineJoin::Miter => {}
            LineJoin::Round => attrs.push_str(" stroke-linejoin=\"round\""),
            LineJoin::Bevel => attrs.push_str(" stroke-linejoin=\"bevel\""),
        }
        if self.state.miter_limit != 4.0 {
            let _ = write!(attrs, " stroke-miterlimit=\"{}\"", fmt_f(self.state.miter_limit));
        }
        if !self.state.dash_pattern.is_empty() {
            let dashes: Vec<String> = self.state.dash_pattern.iter().map(|d| fmt_f(*d)).collect();
            let _ = write!(attrs, " stroke-dasharray=\"{}\"", dashes.join(","));
            if self.state.dash_phase != 0.0 {
                let _ = write!(attrs, " stroke-dashoffset=\"{}\"", fmt_f(self.state.dash_phase));
            }
        }
        attrs
    }

    fn fill_current_path(&mut self, fill_rule: &str) {
        if let Some(pd) = self.path_data.take() {
            let transform = self.svg_transform_attr();
            let fill = self.fill_color_svg();
            let opacity = self.opacity_attrs(true, false);
            let clip = self.clip_attr();
            let bm = self.blend_mode_attr();
            let rule = if fill_rule == "evenodd" {
                " fill-rule=\"evenodd\""
            } else {
                ""
            };
            self.elements.push(format!(
                "<path d=\"{}\" fill=\"{}\"{} stroke=\"none\" {}{}{}{}/>",
                pd.trim(), fill, rule, transform, opacity, clip, bm,
            ));
        }
    }

    fn stroke_current_path(&mut self) {
        if let Some(pd) = self.path_data.take() {
            let transform = self.svg_transform_attr();
            let stroke = self.stroke_color_svg();
            let opacity = self.opacity_attrs(false, true);
            let clip = self.clip_attr();
            let bm = self.blend_mode_attr();
            let stroke_attrs = self.stroke_attrs();
            self.elements.push(format!(
                "<path d=\"{}\" fill=\"none\" stroke=\"{}\"{}{}{}{}{}/>",
                pd.trim(), stroke, stroke_attrs, transform, opacity, clip, bm,
            ));
        }
    }

    fn fill_and_stroke_path(&mut self, fill_rule: &str) {
        if let Some(pd) = self.path_data.take() {
            let transform = self.svg_transform_attr();
            let fill = self.fill_color_svg();
            let stroke = self.stroke_color_svg();
            let opacity = self.opacity_attrs(true, true);
            let clip = self.clip_attr();
            let bm = self.blend_mode_attr();
            let stroke_attrs = self.stroke_attrs();
            let rule = if fill_rule == "evenodd" {
                " fill-rule=\"evenodd\""
            } else {
                ""
            };
            self.elements.push(format!(
                "<path d=\"{}\" fill=\"{}\"{} stroke=\"{}\"{}{}{}{}{}/>",
                pd.trim(), fill, rule, stroke, stroke_attrs, transform, opacity, clip, bm,
            ));
        }
    }

    fn apply_clip(&mut self, clip_rule: &str) {
        if let Some(pd) = self.path_data.clone() {
            let clip_id = self.next_id("clip");
            let transform = self.svg_transform_attr();
            let rule = if clip_rule == "evenodd" {
                " clip-rule=\"evenodd\""
            } else {
                ""
            };
            self.defs.push(format!(
                "<clipPath id=\"{}\"><path d=\"{}\"{}  {}/></clipPath>",
                clip_id, pd.trim(), rule, transform,
            ));
            self.active_clip_id = Some(clip_id);
        }
    }

    // -----------------------------------------------------------------------
    // Text rendering
    // -----------------------------------------------------------------------

    fn render_text_string(&mut self, string_bytes: &[u8]) {
        let font_name = self.state.text.font_name.clone();
        let font = match self.fonts.get(&font_name) {
            Some(f) => f,
            None => return,
        };

        let font_size = self.state.text.font_size;
        let horiz_scaling = self.state.text.horiz_scaling;
        let char_spacing = self.state.text.char_spacing;
        let word_spacing = self.state.text.word_spacing;
        let text_rise = self.state.text.text_rise;
        let render_mode = self.state.text.render_mode;
        let is_cid = font.info.subtype == b"Type0";

        // Decode char codes
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

        // Collect Unicode text using the CMap
        let cmap = font.cmap.as_ref();

        // Get widths
        let widths: Vec<f64> = char_codes.iter().map(|code| font.info.widths.get_width(*code)).collect();

        // Get font family name from font info
        let font_family = extract_font_family(&font.info);

        for (i, code) in char_codes.iter().enumerate() {
            let width = widths[i];
            let w0 = width / 1000.0;

            if render_mode != 3 {
                // Try to get Unicode text
                let text_char = if let Some(cm) = cmap {
                    cm.lookup(*code)
                } else if *code < 128 {
                    // Basic ASCII fallback
                    char::from_u32(*code).map(|c| c.to_string())
                } else {
                    None
                };

                if let Some(text) = text_char {
                    // Emit a <text> element
                    let trm = Matrix {
                        a: font_size * horiz_scaling,
                        b: 0.0,
                        c: 0.0,
                        d: font_size,
                        e: 0.0,
                        f: text_rise,
                    }
                    .concat(&self.state.text_matrix)
                    .concat(&self.state.ctm)
                    .concat(&self.page_transform);

                    let fill_color = self.fill_color_svg();
                    let opacity = self.opacity_attrs(true, false);
                    let clip = self.clip_attr();
                    let bm = self.blend_mode_attr();

                    // Extract effective font size from the matrix
                    let effective_size = trm.font_size_scale();

                    // Position: the text matrix gives us x, y in SVG space
                    let x = trm.e;
                    let y = trm.f;

                    let escaped = xml_escape(&text);
                    self.elements.push(format!(
                        "<text x=\"{}\" y=\"{}\" font-family=\"{}\" font-size=\"{}\" fill=\"{}\"{}{}{}>{}</text>",
                        fmt_f(x), fmt_f(y), xml_escape(&font_family), fmt_f(effective_size),
                        fill_color, opacity, clip, bm, escaped,
                    ));
                } else {
                    // Fallback: draw a rectangle placeholder
                    let glyph_width = w0 * font_size;
                    if glyph_width.abs() > 0.001 {
                        let trm = self
                            .state
                            .text_matrix
                            .concat(&self.state.ctm)
                            .concat(&self.page_transform);

                        let fill_color = self.fill_color_svg();
                        let opacity = self.opacity_attrs(true, false);
                        let clip = self.clip_attr();

                        let rx = 0.0_f64;
                        let ry = text_rise - font_size * 0.2;
                        let rw = glyph_width;
                        let rh = font_size * 0.8;

                        let pd = format!(
                            "M{} {} L{} {} L{} {} L{} {} Z",
                            fmt_f(rx), fmt_f(ry),
                            fmt_f(rx + rw), fmt_f(ry),
                            fmt_f(rx + rw), fmt_f(ry + rh),
                            fmt_f(rx), fmt_f(ry + rh),
                        );

                        self.elements.push(format!(
                            "<path d=\"{}\" fill=\"{}\" stroke=\"none\" transform=\"matrix({},{},{},{},{},{})\"{}{}/>",
                            pd, fill_color,
                            fmt_f(trm.a), fmt_f(trm.b), fmt_f(trm.c), fmt_f(trm.d), fmt_f(trm.e), fmt_f(trm.f),
                            opacity, clip,
                        ));
                    }
                }
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
    }

    fn adjust_text_position(&mut self, amount: f64) {
        let font_size = self.state.text.font_size;
        let horiz_scaling = self.state.text.horiz_scaling;
        let tx = -amount / 1000.0 * font_size * horiz_scaling;
        let advance = Matrix::translate(tx, 0.0);
        self.state.text_matrix = advance.concat(&self.state.text_matrix);
    }

    // -----------------------------------------------------------------------
    // XObject handling
    // -----------------------------------------------------------------------

    fn do_xobject(&mut self, name: &[u8], page: &PageInfo) -> Result<()> {
        let xobj = self.resolve_xobject(name, page)?;
        let xobj = match xobj {
            Some(x) => x,
            None => return Ok(()),
        };

        match xobj {
            XObjectData::Image { dict, data } => {
                let _ = self.render_image(&dict, &data);
            }
            XObjectData::Form { dict, data } => {
                if self.xobject_depth > 10 {
                    return Ok(());
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
                self.doc.resolve(&r)?
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

        let xobj = self.doc.resolve(&xobj_ref)?;

        match xobj {
            PdfObject::Stream { dict, data } => {
                let subtype = dict.get_name(b"Subtype").unwrap_or(b"");
                match subtype {
                    b"Image" => {
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
                    b"Form" => match self.doc.decode_stream(&dict, &data) {
                        Ok(decoded) => Ok(Some(XObjectData::Form {
                            dict,
                            data: decoded,
                        })),
                        Err(_) => Ok(None),
                    },
                    _ => Ok(None),
                }
            }
            _ => Ok(None),
        }
    }

    fn render_image(&mut self, dict: &PdfDict, data: &[u8]) -> Result<()> {
        let decoded = image::decode_image(data, dict).map_err(RenderError::Core)?;

        let rgba_data = image_to_rgba(&decoded);
        let w = decoded.width;
        let h = decoded.height;

        // Encode as PNG, then base64
        let png_bytes = encode_rgba_to_png(&rgba_data, w, h);
        let b64 = base64_encode(&png_bytes);

        // PDF images are placed in a 1x1 unit square, scaled by the CTM
        let image_transform = Matrix {
            a: 1.0 / w as f64,
            b: 0.0,
            c: 0.0,
            d: -1.0 / h as f64,
            e: 0.0,
            f: 1.0,
        };

        let full_transform = image_transform
            .concat(&self.state.ctm)
            .concat(&self.page_transform);

        let opacity = if self.state.fill_alpha < 1.0 {
            format!(" opacity=\"{}\"", fmt_f(self.state.fill_alpha))
        } else {
            String::new()
        };
        let clip = self.clip_attr();
        let bm = self.blend_mode_attr();

        self.elements.push(format!(
            "<image width=\"{}\" height=\"{}\" href=\"data:image/png;base64,{}\" transform=\"matrix({},{},{},{},{},{})\" preserveAspectRatio=\"none\"{}{}{}/>",
            w, h, b64,
            fmt_f(full_transform.a), fmt_f(full_transform.b),
            fmt_f(full_transform.c), fmt_f(full_transform.d),
            fmt_f(full_transform.e), fmt_f(full_transform.f),
            opacity, clip, bm,
        ));

        Ok(())
    }

    fn render_form_xobject(
        &mut self,
        dict: &PdfDict,
        data: &[u8],
        page: &PageInfo,
    ) -> Result<()> {
        self.state_stack.push(self.state.clone());
        self.clip_id_stack.push(self.active_clip_id.clone());

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

        let ops = parse_content_stream(data).map_err(RenderError::Core)?;
        let _ = self.execute_ops(&ops, page);

        if let Some(s) = self.state_stack.pop() {
            self.state = s;
        }
        if let Some(clip_id) = self.clip_id_stack.pop() {
            self.active_clip_id = clip_id;
        }

        Ok(())
    }

    // -----------------------------------------------------------------------
    // ExtGState
    // -----------------------------------------------------------------------

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
                self.doc.resolve(&r)?
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
                self.doc.resolve(&r)?
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
            if let Some(a) = gs_dict.get(b"ca").and_then(|o| o.as_f64()) {
                self.state.fill_alpha = a;
            }
            if let Some(a) = gs_dict.get(b"CA").and_then(|o| o.as_f64()) {
                self.state.stroke_alpha = a;
            }
            if let Some(bm_name) = gs_dict.get(b"BM").and_then(|o| o.as_name()) {
                self.state.blend_mode = PdfBlendMode::from_name(bm_name);
            }
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Helper types and functions
// ---------------------------------------------------------------------------

enum XObjectData {
    Image { dict: PdfDict, data: Vec<u8> },
    Form { dict: PdfDict, data: Vec<u8> },
}

/// Extract operand as f64 at the given index.
fn f(operands: &[Operand], idx: usize) -> f64 {
    operands.get(idx).and_then(|o| o.as_f64()).unwrap_or(0.0)
}

/// Map a PDF color space name to ColorSpace.
fn cs_from_name(name: &[u8]) -> ColorSpace {
    match name {
        b"DeviceRGB" => ColorSpace::DeviceRGB,
        b"DeviceCMYK" => ColorSpace::DeviceCMYK,
        b"DeviceGray" => ColorSpace::DeviceGray,
        _ => ColorSpace::DeviceGray,
    }
}

/// Format a float compactly (avoid trailing zeros).
fn fmt_f(v: f64) -> String {
    if (v - v.round()).abs() < 1e-6 {
        format!("{}", v.round() as i64)
    } else {
        format!("{:.4}", v)
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string()
    }
}

/// Escape special XML characters.
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Extract a reasonable font-family name from FontInfo.
fn extract_font_family(info: &FontInfo) -> String {
    let base = String::from_utf8_lossy(&info.base_font).to_string();
    if base.is_empty() {
        return "sans-serif".to_string();
    }
    // Strip the subset prefix (6 chars + '+') if present, e.g. "ABCDEF+Arial" -> "Arial"
    let name = if base.len() > 7 && base.as_bytes()[6] == b'+' {
        &base[7..]
    } else {
        &base
    };
    // Replace common separators
    name.replace(',', " ").replace('-', " ")
}

/// Convert decoded image data to RGBA.
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
            // CMYK -> RGB
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

/// Encode RGBA data as PNG bytes (minimal encoder without external crate).
fn encode_rgba_to_png(rgba: &[u8], width: u32, height: u32) -> Vec<u8> {
    // Use the image crate which is already a dependency
    let mut buf = std::io::Cursor::new(Vec::new());
    let encoder = ::image::codecs::png::PngEncoder::new(&mut buf);
    let _ = ::image::ImageEncoder::write_image(
        encoder,
        rgba,
        width,
        height,
        ::image::ColorType::Rgba8.into(),
    );
    buf.into_inner()
}

/// Simple base64 encoder (no external dependency).
fn base64_encode(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::with_capacity((data.len() + 2) / 3 * 4);

    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };

        let n = (b0 << 16) | (b1 << 8) | b2;

        result.push(CHARS[((n >> 18) & 0x3F) as usize] as char);
        result.push(CHARS[((n >> 12) & 0x3F) as usize] as char);

        if chunk.len() > 1 {
            result.push(CHARS[((n >> 6) & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }

        if chunk.len() > 2 {
            result.push(CHARS[(n & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
    }

    result
}
