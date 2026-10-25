use std::collections::HashMap;
use std::sync::Arc;

use justpdf_core::PdfDocument;
use justpdf_core::color::{Color as PdfColor, ColorSpace};
use justpdf_core::content::{ContentOp, Operand, parse_content_stream};
use justpdf_core::font::FontProgramData;
use justpdf_core::font::recovery::find_substitute;
use justpdf_core::font::{
    Encoding, EncodingCMap, FontInfo, ToUnicodeCMap, descendant_font, parse_font_info,
    resolve_font_entries, type0_encoding_cmap,
};
use justpdf_core::function::PdfFunction;
use justpdf_core::image;
use justpdf_core::object::{IndirectRef, PdfDict, PdfObject};
use justpdf_core::ocg::{self, OCConfig};
use justpdf_core::page::PageInfo;
use justpdf_core::ttf_parser;
use tiny_skia::{FillRule, Mask, PathBuilder, Pixmap, Transform};

use crate::device::PixmapDevice;
use crate::error::{RenderError, Result};
use crate::glyph_cache::GlyphCache;
use crate::graphics_state::{
    FontKey, GraphicsState, LineCap, LineJoin, Matrix, PatternSelection, PdfBlendMode, SharedMask,
    SoftMask, SoftMaskSubtype,
};
use crate::resources::ResourceScopes;

/// Resolved font for rendering.
struct ResolvedFont {
    info: FontInfo,
    #[allow(dead_code)]
    cmap: Option<ToUnicodeCMap>,
    /// Embedded font program for glyph outlines.
    font_program: Option<Arc<FontProgramData>>,
    /// CIDToGIDMap for Type0 CID fonts: maps CID → glyph ID.
    /// None = identity mapping (CID == GID).
    cid_to_gid_map: Option<Vec<u16>>,
    /// The base encoding of the font's `/Encoding`, `None` when it has none.
    encoding: Option<Encoding>,
    /// That base only when the PDF names it: an encoding name or `/BaseEncoding`.
    explicit_base: Option<Encoding>,
    /// CID to glyph ID for a CID-keyed bare CFF program.
    cff_cid_to_gid: Option<std::collections::HashMap<u16, u16>>,
    /// Bit 3 of the font descriptor's `/Flags`.
    symbolic: bool,
    /// A Type0 font's encoding CMap, codes to CIDs.
    cid_encoding: Option<EncodingCMap>,
}

/// The rendering interpreter: walks content stream ops and renders onto a device.
pub struct RenderInterpreter<'a> {
    doc: &'a PdfDocument,
    device: &'a mut PixmapDevice,
    state: GraphicsState,
    state_stack: Vec<GraphicsState>,
    fonts: HashMap<FontKey, ResolvedFont>,
    /// The `/Resources` names resolve in.
    resources: ResourceScopes,
    /// Transform from PDF user space to device (pixel) space.
    page_transform: Matrix,
    /// Current path being constructed.
    path_builder: Option<PathBuilder>,
    /// Form XObject recursion depth limit.
    xobject_depth: u32,
    /// The `state_stack` depth `Q` cannot pop below: that on entry to the
    /// content stream being executed.
    state_floor: usize,
    /// The last clip × soft mask product, with the clip and soft mask it was
    /// made from.
    combined_mask: Option<(SharedMask, SharedMask, SharedMask)>,
    /// Content streams being executed (Form XObjects, tiling patterns, soft
    /// mask forms), outermost first.
    running_streams: Vec<IndirectRef>,
    /// Cache for pre-built glyph paths.
    glyph_cache: GlyphCache,
    /// Optional content configuration for layer visibility.
    oc_config: Option<OCConfig>,
    /// Marked content skip depth: when > 0, all drawing ops are skipped
    /// until the matching EMC reduces it back to 0.
    oc_skip_depth: u32,
}

impl<'a> RenderInterpreter<'a> {
    pub fn new(doc: &'a PdfDocument, device: &'a mut PixmapDevice, page_transform: Matrix) -> Self {
        // Load optional content config for layer visibility
        let oc_config = ocg::read_oc_properties(doc)
            .ok()
            .flatten()
            .and_then(|props| props.default_config);

        Self {
            doc,
            device,
            state: GraphicsState::default(),
            state_stack: Vec::new(),
            fonts: HashMap::new(),
            resources: ResourceScopes::default(),
            page_transform,
            path_builder: None,
            xobject_depth: 0,
            state_floor: 0,
            combined_mask: None,
            running_streams: Vec::new(),
            glyph_cache: GlyphCache::with_default_capacity(),
            oc_config,
            oc_skip_depth: 0,
        }
    }

    /// Render a page's content streams, then annotation appearance streams.
    pub fn render_page(&mut self, page: &PageInfo) -> Result<()> {
        self.resources = ResourceScopes::for_page(self.doc, page);

        // Get content stream data
        let content_data = self.get_page_content(page)?;
        if !content_data.is_empty() {
            let ops = parse_content_stream(&content_data).map_err(RenderError::Core)?;
            self.execute_ops(&ops)?;
        }

        // Render annotation appearance streams
        let _ = self.render_annotations(page);

        Ok(())
    }

    /// Render annotation appearance streams on top of page content.
    fn render_annotations(&mut self, page: &PageInfo) -> Result<()> {
        // Get page dict → /Annots array
        let page_obj = self.doc.resolve(&page.page_ref)?;
        let page_dict = match page_obj.as_dict() {
            Some(d) => d.clone(),
            None => return Ok(()),
        };

        let annots_arr = match page_dict.get(b"Annots") {
            Some(PdfObject::Array(arr)) => arr.clone(),
            Some(PdfObject::Reference(r)) => {
                let r = r.clone();
                match self.doc.resolve(&r)? {
                    PdfObject::Array(arr) => arr,
                    _ => return Ok(()),
                }
            }
            _ => return Ok(()),
        };

        for item in &annots_arr {
            let annot_dict = match item {
                PdfObject::Reference(r) => {
                    let r = r.clone();
                    match self.doc.resolve(&r)? {
                        PdfObject::Dict(d) => d,
                        _ => continue,
                    }
                }
                PdfObject::Dict(d) => d.clone(),
                _ => continue,
            };

            // Skip hidden/no-view annotations
            if justpdf_core::annot::placement::is_hidden(&annot_dict)
                || !self.oc_entry_visible(&annot_dict)
            {
                // Hidden, NoView, or in hidden optional content
                continue;
            }

            let Some(rect) = annot_dict
                .get_array(b"Rect")
                .and_then(justpdf_core::page::Rect::from_pdf_array)
            else {
                continue;
            };
            let Some(Appearance {
                obj_ref: ap_ref,
                dict: ap_dict,
                data: ap_data,
            }) = self.normal_appearance(&annot_dict)?
            else {
                continue;
            };
            let Some(mapping) = appearance_mapping(&ap_dict, &rect) else {
                continue;
            };

            self.state_stack.push(self.state.clone());
            self.state.ctm = mapping.concat(&self.state.ctm);
            self.clip_to_bbox(&ap_dict);
            self.with_stream_resources(&ap_dict, ap_ref.as_ref(), |this| {
                let _ = this.render_form_xobject(&ap_dict, &ap_data);
            });
            if let Some(saved) = self.state_stack.pop() {
                self.state = saved;
            }
        }

        Ok(())
    }

    /// The decoded normal appearance of `annot`: `/AP /N` when it is a
    /// stream, else the entry of that state dictionary `/AS` names. `None`
    /// when there is none or it does not decode.
    fn normal_appearance(&mut self, annot: &PdfDict) -> Result<Option<Appearance>> {
        let doc = self.doc;
        let Some((obj_ref, PdfObject::Stream { dict, data })) =
            justpdf_core::annot::placement::normal_appearance(annot, |r| doc.resolve(r).ok())
        else {
            return Ok(None);
        };
        Ok(doc.decode_stream(&dict, &data).ok().map(|data| Appearance {
            obj_ref,
            dict,
            data,
        }))
    }

    /// Clip to the form `dict`'s `/BBox` in form space: through its
    /// `/Matrix` and the current CTM.
    fn clip_to_bbox(&mut self, dict: &PdfDict) {
        let Some(bbox) = dict
            .get_array(b"BBox")
            .and_then(justpdf_core::page::Rect::from_pdf_array)
        else {
            return;
        };
        let Some(path) = tiny_skia::Rect::from_ltrb(
            bbox.llx as f32,
            bbox.lly as f32,
            bbox.urx as f32,
            bbox.ury as f32,
        )
        .map(PathBuilder::from_rect) else {
            return;
        };
        let transform = form_matrix(dict)
            .concat(&self.state.ctm)
            .concat(&self.page_transform)
            .to_skia();
        self.intersect_clip(&path, FillRule::Winding, transform);
    }

    /// Whether an `/OC` marked-content section is visible: its properties
    /// operand is an inline dictionary, or a name looked up in the current
    /// scope's `/Properties`, evaluated by `ocg::is_oc_visible`. A name that
    /// does not resolve is visible.
    fn check_oc_visibility(&self, operands: &[Operand], config: &OCConfig) -> bool {
        let oc = match operands.get(1) {
            Some(Operand::Name(name)) => {
                match self.resources.lookup(self.doc, b"Properties", name) {
                    Ok(Some(found)) => match found.obj_ref {
                        Some(r) => PdfObject::Reference(r),
                        None => found.object,
                    },
                    _ => return true,
                }
            }
            Some(Operand::Dict(dict)) => PdfObject::Dict(self.operand_dict_to_pdf_dict(dict)),
            _ => return true,
        };
        ocg::is_oc_visible(&oc, self.doc, config)
    }

    /// Whether an object with the `/OC` entry of `dict` is visible: always
    /// when there is no entry or no optional content configuration.
    fn oc_entry_visible(&self, dict: &PdfDict) -> bool {
        match (dict.get(b"OC"), &self.oc_config) {
            (Some(oc), Some(config)) => ocg::is_oc_visible(oc, self.doc, config),
            _ => true,
        }
    }

    /// Convert operand dict entries to a PdfDict for OCG/OCMD parsing.
    fn operand_dict_to_pdf_dict(&self, entries: &[(Vec<u8>, Operand)]) -> PdfDict {
        let mut dict = PdfDict::new();
        for (key, value) in entries {
            dict.insert(key.clone(), Self::operand_to_pdf_object(value));
        }
        dict
    }

    /// Convert an Operand to a PdfObject (best-effort for OCG dict parsing).
    fn operand_to_pdf_object(op: &Operand) -> PdfObject {
        match op {
            Operand::Integer(v) => PdfObject::Integer(*v),
            Operand::Real(v) => PdfObject::Real(*v),
            Operand::Bool(v) => PdfObject::Bool(*v),
            Operand::Null => PdfObject::Null,
            Operand::Name(n) => PdfObject::Name(n.clone()),
            Operand::String(s) => PdfObject::String(s.clone()),
            Operand::Array(arr) => {
                PdfObject::Array(arr.iter().map(Self::operand_to_pdf_object).collect())
            }
            Operand::Dict(entries) => {
                let mut dict = PdfDict::new();
                for (k, v) in entries {
                    dict.insert(k.clone(), Self::operand_to_pdf_object(v));
                }
                PdfObject::Dict(dict)
            }
            Operand::InlineImage { .. } => PdfObject::Null,
        }
    }

    /// Selects the font named `name` in the current resource scope,
    /// resolving it on first use.
    fn select_font(&mut self, name: &[u8]) -> Option<FontKey> {
        let fonts = &self.fonts;
        let selection = self
            .resources
            .select_font(self.doc, name, |key| fonts.contains_key(key))?;
        if let Some(dict) = &selection.dict {
            let font = self.resolve_font(dict).ok()?;
            self.fonts.insert(selection.key.clone(), font);
        }
        Some(selection.key)
    }

    /// Selects the font object `font_ref`, as an ExtGState's `/Font` names
    /// it, resolving it on first use.
    fn select_font_ref(&mut self, font_ref: &IndirectRef) -> Option<FontKey> {
        let key = FontKey::Object(font_ref.clone());
        if !self.fonts.contains_key(&key) {
            let PdfObject::Dict(dict) = self.doc.resolve(font_ref).ok()? else {
                return None;
            };
            let font = self.resolve_font(&dict).ok()?;
            self.fonts.insert(key.clone(), font);
        }
        Some(key)
    }

    fn resolve_font(&mut self, fd: &PdfDict) -> Result<ResolvedFont> {
        let doc = self.doc;
        let fd = &resolve_font_entries(fd, |r| doc.resolve(r).ok());
        let mut info = parse_font_info(fd);
        let encoding = fd.get(b"Encoding").map(|_| info.encoding);
        let explicit_base = justpdf_core::font::explicit_base_encoding(fd);
        let symbolic = self
            .get_font_descriptor(fd)
            .and_then(|d| d.get_i64(b"Flags"))
            .is_some_and(|flags| flags & 4 != 0);

        // Resolve ToUnicode CMap
        let cmap = if let Some(PdfObject::Reference(tu_ref)) = fd.get(b"ToUnicode") {
            let tu_ref = tu_ref.clone();
            if let Ok(PdfObject::Stream { dict, data }) = self.doc.resolve(&tu_ref) {
                let decoded = self.doc.decode_stream(&dict, &data).ok();
                decoded.map(|d| ToUnicodeCMap::parse(&d))
            } else {
                None
            }
        } else {
            None
        };

        // Resolve CIDFont widths, font descriptor, and CIDToGIDMap for Type0 fonts
        let mut cid_font_descriptor: Option<PdfDict> = None;
        let mut cid_to_gid_map: Option<Vec<u16>> = None;
        let cid_encoding = (info.subtype == b"Type0").then(|| {
            type0_encoding_cmap(
                fd,
                |r| doc.resolve(r).ok(),
                |dict, data| doc.decode_stream(dict, data).ok(),
            )
        });
        if info.subtype == b"Type0"
            && let Some(cid_dict) = descendant_font(fd, |r| doc.resolve(r).ok())
        {
            let cid_dict = &resolve_font_entries(&cid_dict, |r| doc.resolve(r).ok());
            info.widths = parse_font_info(cid_dict).widths;
            // Get font descriptor from CID font
            if let Some(fd_obj) = cid_dict.get(b"FontDescriptor") {
                let fd_resolved = match fd_obj {
                    PdfObject::Reference(r) => {
                        let r = r.clone();
                        self.doc.resolve(&r).ok()
                    }
                    other => Some(other.clone()),
                };
                if let Some(PdfObject::Dict(d)) = fd_resolved {
                    cid_font_descriptor = Some(d);
                }
            }

            // Parse CIDToGIDMap
            cid_to_gid_map = self.parse_cid_to_gid_map(cid_dict);
        }

        // Extract embedded font data from FontDescriptor; a simple font
        // without one it can read is drawn with the substitute its name picks.
        let font_program = self
            .extract_font_data(fd, cid_font_descriptor.as_ref())
            .filter(|program| crate::glyph::GlyphSource::parse(&program.data).is_some())
            .or_else(|| {
                (info.subtype != b"Type0" && info.subtype != b"Type3")
                    .then(|| crate::substitute::base14_program(find_substitute(&info.base_font)))
                    .flatten()
            });

        let cff_cid_to_gid = font_program
            .as_ref()
            .filter(|_| info.subtype == b"Type0")
            .and_then(|p| crate::glyph::GlyphSource::parse(&p.data))
            .and_then(|source| source.cff_cid_to_gid());

        Ok(ResolvedFont {
            info,
            cmap,
            font_program,
            cid_to_gid_map,
            encoding,
            explicit_base,
            cff_cid_to_gid,
            symbolic,
            cid_encoding,
        })
    }

    /// Extract embedded font program from FontDescriptor.
    /// Looks for FontFile, FontFile2 (TrueType), FontFile3 (CFF/OpenType).
    fn extract_font_data(
        &mut self,
        font_dict: &PdfDict,
        cid_descriptor: Option<&PdfDict>,
    ) -> Option<Arc<FontProgramData>> {
        // First try the font's own descriptor, then CID font descriptor
        let descriptor = self
            .get_font_descriptor(font_dict)
            .or_else(|| cid_descriptor.cloned());

        let descriptor = descriptor?;

        // Try FontFile2 (TrueType), FontFile3 (CFF/OpenType), FontFile (Type1)
        for key in &[b"FontFile2".as_slice(), b"FontFile3", b"FontFile"] {
            let program = match descriptor.get(key) {
                Some(PdfObject::Reference(r)) => self.doc.font_program(r).ok(),
                Some(PdfObject::Stream { dict, data }) => self
                    .doc
                    .decode_stream(dict, data)
                    .ok()
                    .map(|decoded| Arc::new(FontProgramData::new(decoded))),
                _ => None,
            };
            if program.is_some() {
                return program;
            }
        }

        None
    }

    fn get_font_descriptor(&mut self, font_dict: &PdfDict) -> Option<PdfDict> {
        let fd_obj = font_dict.get(b"FontDescriptor")?;
        let resolved = match fd_obj {
            PdfObject::Reference(r) => {
                let r = r.clone();
                self.doc.resolve(&r).ok()
            }
            other => Some(other.clone()),
        };
        match resolved {
            Some(PdfObject::Dict(d)) => Some(d),
            _ => None,
        }
    }

    /// Parse CIDToGIDMap from a CID font dictionary.
    /// Returns None for Identity mapping (CID == GID) or if not present.
    /// Returns Some(vec) for stream-based mapping (2 bytes per CID entry).
    fn parse_cid_to_gid_map(&mut self, cid_dict: &PdfDict) -> Option<Vec<u16>> {
        let map_obj = cid_dict.get(b"CIDToGIDMap")?;

        match map_obj {
            PdfObject::Name(n) if n == b"Identity" => {
                // Identity mapping: CID == GID
                None
            }
            PdfObject::Reference(r) => {
                let r = r.clone();
                let resolved = self.doc.resolve(&r).ok()?;
                if let PdfObject::Stream { dict, data } = resolved {
                    let decoded = self.doc.decode_stream(&dict, &data).ok()?;
                    Some(parse_cid_gid_stream(&decoded))
                } else {
                    None
                }
            }
            PdfObject::Stream { dict, data } => {
                let decoded = self.doc.decode_stream(dict, data).ok()?;
                Some(parse_cid_gid_stream(&decoded))
            }
            _ => None,
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
                // Skip streams that fail to decode (corrupt data)
                if let Ok(decoded) = self.doc.decode_stream(&dict, &data) {
                    combined.extend_from_slice(&decoded);
                    combined.push(b' ');
                }
            }
        }
        Ok(combined)
    }

    fn execute_ops(&mut self, ops: &[ContentOp]) -> Result<()> {
        for op in ops {
            let operator = op.operator_str();

            // Handle OCG skip: when inside a hidden layer, only track
            // BMC/BDC/EMC nesting to know when to resume.
            if self.oc_skip_depth > 0 {
                match operator {
                    "BMC" | "BDC" => self.oc_skip_depth += 1,
                    "EMC" => self.oc_skip_depth -= 1,
                    _ => {}
                }
                continue;
            }

            self.execute_op(op)?;
        }
        Ok(())
    }

    fn execute_op(&mut self, op: &ContentOp) -> Result<()> {
        let operator = op.operator_str();
        let operands = &op.operands;

        match operator {
            // --- Graphics state ---
            "q" => {
                self.state_stack.push(self.state.clone());
            }
            "Q" => {
                if self.state_stack.len() > self.state_floor
                    && let Some(s) = self.state_stack.pop()
                {
                    self.state = s;
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
                        self.state.dash_pattern = arr.iter().filter_map(|o| o.as_f64()).collect();
                    }
                    self.state.dash_phase = f(operands, 1);
                }
            }

            // ExtGState
            "gs" => {
                if let Some(name) = operands.first().and_then(|o| o.as_name()) {
                    self.apply_extgstate(name)?;
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
                self.stroke_current_path();
            }
            "s" => {
                // Close and stroke
                if let Some(pb) = &mut self.path_builder {
                    pb.close();
                }
                self.stroke_current_path();
            }
            "f" | "F" => {
                // Fill (non-zero winding)
                self.fill_current_path(FillRule::Winding);
            }
            "f*" => {
                // Fill (even-odd)
                self.fill_current_path(FillRule::EvenOdd);
            }
            "B" => {
                // Fill and stroke (non-zero)
                self.fill_current_path_keep(FillRule::Winding);
                self.stroke_current_path();
            }
            "B*" => {
                self.fill_current_path_keep(FillRule::EvenOdd);
                self.stroke_current_path();
            }
            "b" => {
                if let Some(pb) = &mut self.path_builder {
                    pb.close();
                }
                self.fill_current_path_keep(FillRule::Winding);
                self.stroke_current_path();
            }
            "b*" => {
                if let Some(pb) = &mut self.path_builder {
                    pb.close();
                }
                self.fill_current_path_keep(FillRule::EvenOdd);
                self.stroke_current_path();
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
                    match self.colour_space(name) {
                        Some(cs) => {
                            self.state.stroke_color = cs.initial_color();
                            self.state.stroke_cs = cs;
                            self.state.stroke_pattern = None;
                        }
                        None => self.state.stroke_cs = ColorSpace::DeviceRGB,
                    }
                }
            }
            "cs" => {
                if let Some(name) = operands.first().and_then(|o| o.as_name()) {
                    match self.colour_space(name) {
                        Some(cs) => {
                            self.state.fill_color = cs.initial_color();
                            self.state.fill_cs = cs;
                            self.state.fill_pattern = None;
                        }
                        None => self.state.fill_cs = ColorSpace::DeviceRGB,
                    }
                }
            }
            "SC" | "SCN" => {
                // Last operand may be a pattern name if stroke CS is Pattern
                let last_is_name = operands.last().and_then(|o| o.as_name());
                if last_is_name.is_some() {
                    self.state.stroke_pattern =
                        last_is_name.and_then(|n| self.resources.select_pattern(self.doc, n));
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
                    self.state.fill_pattern =
                        last_is_name.and_then(|n| self.resources.select_pattern(self.doc, n));
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
                self.state.stroke_pattern = None;
                self.state.stroke_color = PdfColor::gray(f(operands, 0));
            }
            "g" => {
                self.state.fill_cs = ColorSpace::DeviceGray;
                self.state.fill_pattern = None;
                self.state.fill_color = PdfColor::gray(f(operands, 0));
            }
            "RG" => {
                self.state.stroke_cs = ColorSpace::DeviceRGB;
                self.state.stroke_pattern = None;
                self.state.stroke_color =
                    PdfColor::rgb(f(operands, 0), f(operands, 1), f(operands, 2));
            }
            "rg" => {
                self.state.fill_cs = ColorSpace::DeviceRGB;
                self.state.fill_pattern = None;
                self.state.fill_color =
                    PdfColor::rgb(f(operands, 0), f(operands, 1), f(operands, 2));
            }
            "K" => {
                self.state.stroke_cs = ColorSpace::DeviceCMYK;
                self.state.stroke_pattern = None;
                self.state.stroke_color = PdfColor::cmyk(
                    f(operands, 0),
                    f(operands, 1),
                    f(operands, 2),
                    f(operands, 3),
                );
            }
            "k" => {
                self.state.fill_cs = ColorSpace::DeviceCMYK;
                self.state.fill_pattern = None;
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
                    self.state.text.font = self.select_font(name);
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
                    self.do_xobject(name)?;
                }
            }

            // --- Inline image ---
            "BI" => {
                if let Some(Operand::InlineImage { dict, data }) = operands.first() {
                    self.render_inline_image(dict, data)?;
                }
            }

            // --- Marked content / Optional Content ---
            "BMC" => {}
            "BDC" => {
                // Check if this is an OC (Optional Content) marked content
                // BDC operands: tag properties
                // If tag is /OC and properties is a dict with /OCGS or /OCGs,
                // check visibility against the OCG config.
                if let Some(config) = &self.oc_config {
                    let tag = operands.first().and_then(|o| o.as_name());
                    if tag == Some(b"OC") {
                        let visible = self.check_oc_visibility(operands, config);
                        if !visible {
                            self.oc_skip_depth = 1;
                        }
                    }
                }
            }
            "EMC" => {}
            "MP" | "DP" => {}

            // --- Shading ---
            "sh" => {
                if let Some(name) = operands.first().and_then(|o| o.as_name()) {
                    self.render_shading(name)?;
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

    fn fill_current_path(&mut self, rule: FillRule) {
        if let Some(pb) = self.path_builder.take()
            && let Some(path) = pb.finish()
        {
            // Check for pattern fill
            if self.state.fill_pattern.is_some() && self.try_fill_with_pattern(&path, rule) {
                return;
            }
            let transform = self.effective_transform();
            let color = self.state.fill_color_rgba();
            let bm = self.blend_mode();
            let mask = self.draw_mask();
            self.device.fill_path(
                &path,
                rule,
                transform,
                color,
                bm,
                mask.as_ref().map(SharedMask::mask),
            );
        }
    }

    fn fill_current_path_keep(&mut self, rule: FillRule) {
        if let Some(pb) = &self.path_builder {
            let pb_clone = pb.clone();
            if let Some(path) = pb_clone.finish() {
                // Check for pattern fill
                if self.state.fill_pattern.is_some() && self.try_fill_with_pattern(&path, rule) {
                    return;
                }
                let transform = self.effective_transform();
                let color = self.state.fill_color_rgba();
                let bm = self.blend_mode();
                let mask = self.draw_mask();
                self.device.fill_path(
                    &path,
                    rule,
                    transform,
                    color,
                    bm,
                    mask.as_ref().map(SharedMask::mask),
                );
            }
        }
    }

    fn stroke_current_path(&mut self) {
        if let Some(pb) = self.path_builder.take()
            && let Some(path) = pb.finish()
        {
            // Check for pattern stroke
            if self.state.stroke_pattern.is_some() && self.try_stroke_with_pattern(&path) {
                return;
            }
            let transform = self.effective_transform();
            let color = self.state.stroke_color_rgba();
            let bm = self.blend_mode();
            let mask = self.draw_mask();
            self.device.stroke_path(
                &path,
                transform,
                color,
                &self.state,
                bm,
                mask.as_ref().map(SharedMask::mask),
            );
        }
    }

    fn apply_clip(&mut self, rule: FillRule) {
        if let Some(pb) = &self.path_builder {
            let pb_clone = pb.clone();
            if let Some(path) = pb_clone.finish() {
                let transform = self.effective_transform();
                self.intersect_clip(&path, rule, transform);
            }
        }
    }

    /// Intersect the current clip with `path`, or make `path` the clip when
    /// there is none. The mask is copied only while another state shares it.
    fn intersect_clip(&mut self, path: &tiny_skia::Path, rule: FillRule, transform: Transform) {
        match &mut self.state.clip {
            Some(clip) => clip.make_mut().intersect_path(path, rule, true, transform),
            None => {
                self.state.clip = self
                    .device
                    .clip_mask(path, rule, transform)
                    .map(SharedMask::new);
            }
        }
    }

    // --- Text rendering ---

    fn render_text_string(&mut self, string_bytes: &[u8]) -> Result<()> {
        let font = match self
            .state
            .text
            .font
            .as_ref()
            .and_then(|k| self.fonts.get(k))
        {
            Some(f) => f,
            None => return Ok(()), // font not found, skip
        };

        let font_size = self.state.text.font_size;
        let horiz_scaling = self.state.text.horiz_scaling;
        let char_spacing = self.state.text.char_spacing;
        let word_spacing = self.state.text.word_spacing;
        let text_rise = self.state.text.text_rise;
        let render_mode = self.state.text.render_mode;

        let is_cid = font.cid_encoding.is_some();

        // Pre-compute char codes, CIDs, widths, and font data while borrowing fonts immutably
        let (char_codes, cids): (Vec<u32>, Vec<u32>) = match &font.cid_encoding {
            Some(cmap) => cmap
                .decode(string_bytes)
                .into_iter()
                .map(|c| (c.code, c.cid))
                .unzip(),
            None => string_bytes.iter().map(|&b| (b as u32, b as u32)).unzip(),
        };

        let widths: Vec<f64> = cids
            .iter()
            .map(|cid| font.info.widths.get_width(*cid))
            .collect();

        // The font program, shared, and its CIDToGIDMap for glyph outline rendering
        let font_program = font.font_program.clone();
        let cid_to_gid_map = font.cid_to_gid_map.clone();
        let source = font_program
            .as_ref()
            .and_then(|p| crate::glyph::GlyphSource::parse(&p.data));

        // The glyph each code draws, when the font program can be read.
        let glyphs: Option<Vec<Option<crate::glyph::GlyphRef>>> = source.as_ref().map(|source| {
            char_codes
                .iter()
                .zip(&cids)
                .map(|(&code, &cid)| {
                    if !is_cid {
                        return source.simple_glyph(
                            code as u8,
                            font.encoding,
                            font.explicit_base,
                            &font.info.differences,
                            font.symbolic,
                        );
                    }
                    let Ok(cid) = u16::try_from(cid) else {
                        return Some(crate::glyph::GlyphRef::Id(ttf_parser::GlyphId(0)));
                    };
                    if let Some(map) = &font.cff_cid_to_gid {
                        return map
                            .get(&cid)
                            .map(|&gid| crate::glyph::GlyphRef::Id(ttf_parser::GlyphId(gid)));
                    }
                    let gid = cid_to_gid_map
                        .as_deref()
                        .and_then(|map| map.get(cid as usize).copied())
                        .unwrap_or(cid);
                    Some(crate::glyph::GlyphRef::Id(ttf_parser::GlyphId(gid)))
                })
                .collect()
        });

        // Now we're done borrowing self.fonts, can mutably borrow self
        for (i, code) in char_codes.iter().enumerate() {
            let width = widths[i];
            let w0 = width / 1000.0;

            if render_mode != 3 {
                self.render_glyph(
                    w0 * font_size,
                    font_size,
                    text_rise,
                    source.as_ref().zip(font_program.as_ref().map(|p| p.hash)),
                    glyphs.as_ref().map(|g| g[i].clone()),
                )?;
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

    #[allow(clippy::too_many_arguments)]
    fn render_glyph(
        &mut self,
        glyph_width: f64,
        font_size: f64,
        text_rise: f64,
        source: Option<(&crate::glyph::GlyphSource, u64)>,
        glyph: Option<Option<crate::glyph::GlyphRef>>,
    ) -> Result<()> {
        if glyph_width.abs() < 0.001 {
            return Ok(());
        }

        // Try to render with real glyph outlines (using glyph cache)
        if let Some((source, program_hash)) = source
            && let Some(glyph) = glyph
        {
            // No glyph for this code: draw nothing.
            let Some(glyph) = glyph else {
                return Ok(());
            };

            let (key_hash, key_glyph) = glyph.cache_key(program_hash);
            let cached_path = self
                .glyph_cache
                .get_or_insert(key_hash, key_glyph, || source.outline(&glyph))
                .cloned();

            if let Some(path) = cached_path {
                {
                    // Glyph space (y up, like PDF text space) to text space at this size.
                    let [a, b, c, d, e, f] = source.font_matrix();
                    let glyph_matrix = Matrix {
                        a: a * font_size,
                        b: b * font_size,
                        c: c * font_size,
                        d: d * font_size,
                        e: e * font_size,
                        f: f * font_size + text_rise,
                    };

                    let text_rendering_matrix = glyph_matrix
                        .concat(&self.state.text_matrix)
                        .concat(&self.state.ctm)
                        .concat(&self.page_transform);

                    let transform = text_rendering_matrix.to_skia();
                    let color = self.state.fill_color_rgba();
                    let bm = self.blend_mode();
                    let mask = self.draw_mask();
                    self.device.fill_path(
                        &path,
                        FillRule::Winding,
                        transform,
                        color,
                        bm,
                        mask.as_ref().map(SharedMask::mask),
                    );
                    return Ok(());
                }
            } else {
                // The chosen glyph has no outline (a space): nothing to draw.
                return Ok(());
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
            let mask = self.draw_mask();
            self.device.fill_path(
                &path,
                FillRule::Winding,
                transform,
                color,
                bm,
                mask.as_ref().map(SharedMask::mask),
            );
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

    /// Runs `f` with `stream` recorded as a content stream being executed.
    /// Returns `None` without running `f` when `stream` is already being
    /// executed. A direct (`None`) stream is always run.
    fn with_running_stream<T>(
        &mut self,
        stream: Option<&IndirectRef>,
        f: impl FnOnce(&mut Self) -> T,
    ) -> Option<T> {
        let Some(stream) = stream else {
            return Some(f(self));
        };
        if self.running_streams.contains(stream) {
            return None;
        }
        self.running_streams.push(stream.clone());
        let out = f(self);
        self.running_streams.pop();
        Some(out)
    }

    /// Runs `f` inside the resource scope of the stream with dictionary
    /// `dict` and reference `stream`.
    fn with_stream_resources<T>(
        &mut self,
        dict: &PdfDict,
        stream: Option<&IndirectRef>,
        f: impl FnOnce(&mut Self) -> T,
    ) -> T {
        let entered = self.resources.enter(self.doc, dict, stream);
        let out = f(self);
        if entered {
            self.resources.leave();
        }
        out
    }

    fn do_xobject(&mut self, name: &[u8]) -> Result<()> {
        let xobj = self.resolve_xobject(name)?;
        let xobj = match xobj {
            Some(x) => x,
            None => return Ok(()),
        };

        let dict = match &xobj {
            XObjectData::Image { dict, .. } | XObjectData::Form { dict, .. } => dict,
        };
        if !self.oc_entry_visible(dict) {
            return Ok(());
        }

        match xobj {
            XObjectData::Image { dict, data } => {
                let _ = self.render_image(&dict, &data); // skip broken images
            }
            XObjectData::Form {
                obj_ref,
                dict,
                data,
            } => {
                if self.xobject_depth > 10 {
                    return Ok(()); // nested too deep
                }
                self.xobject_depth += 1;
                self.state_stack.push(self.state.clone());
                self.clip_to_bbox(&dict);
                self.with_running_stream(Some(&obj_ref), |this| {
                    this.with_stream_resources(&dict, Some(&obj_ref), |this| {
                        let _ = this.render_form_xobject(&dict, &data);
                    });
                });
                if let Some(saved) = self.state_stack.pop() {
                    self.state = saved;
                }
                self.xobject_depth -= 1;
            }
        }

        Ok(())
    }

    fn resolve_xobject(&mut self, name: &[u8]) -> Result<Option<XObjectData>> {
        let Some(found) = self.resources.lookup(self.doc, b"XObject", name)? else {
            return Ok(None);
        };
        let Some(xobj_ref) = found.obj_ref else {
            return Ok(None);
        };
        let xobj = found.object;

        match xobj {
            PdfObject::Stream { dict, data } => {
                let subtype = dict.get_name(b"Subtype").unwrap_or(b"");
                match subtype {
                    b"Image" => Ok(Some(XObjectData::Image { dict, data })),
                    b"Form" => match self.doc.decode_stream(&dict, &data) {
                        Ok(decoded) => Ok(Some(XObjectData::Form {
                            obj_ref: xobj_ref,
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
        let info = image::image_info(dict);

        // Check for ImageMask (stencil mask)
        if let Some(ref img_info) = info
            && img_info.is_mask
        {
            return self.render_image_mask(img_info, data, dict);
        }

        let decoded = image::decode_image(data, dict, self.doc).map_err(RenderError::Core)?;

        // Convert decoded image to RGBA
        let mut rgba_data = image_to_rgba(&decoded);
        let w = decoded.width;
        let h = decoded.height;

        // Apply SMask if present on the image dict
        if let Some(PdfObject::Reference(smask_ref)) = dict.get(b"SMask") {
            let smask_ref = smask_ref.clone();
            if let Ok(smask_obj) = self.doc.resolve(&smask_ref)
                && let PdfObject::Stream {
                    dict: smask_dict,
                    data: smask_data,
                } = smask_obj
            {
                self.apply_image_smask(&mut rgba_data, w, h, &smask_dict, &smask_data);
            }
        }

        // Apply Mask (explicit mask) if present — a 1-bit image defining transparency
        if let Some(PdfObject::Reference(mask_ref)) = dict.get(b"Mask") {
            let mask_ref = mask_ref.clone();
            if let Ok(mask_obj) = self.doc.resolve(&mask_ref)
                && let PdfObject::Stream {
                    dict: mask_dict,
                    data: mask_data,
                } = mask_obj
            {
                self.apply_image_explicit_mask(&mut rgba_data, w, h, &mask_dict, &mask_data);
            }
        }

        let img_pixmap = match tiny_skia::Pixmap::from_vec(
            rgba_data,
            tiny_skia::IntSize::from_wh(w, h).unwrap(),
        ) {
            Some(p) => p,
            None => return Ok(()),
        };

        // PDF images are placed in a 1x1 unit square, scaled by the CTM
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
        let mask = if dict.get(b"SMask").is_some() || dict.get(b"Mask").is_some() {
            self.state.clip.clone()
        } else {
            self.draw_mask()
        };
        self.device.draw_image(
            &img_pixmap.as_ref(),
            full_transform.to_skia(),
            self.state.fill_alpha as f32,
            bm,
            mask.as_ref().map(SharedMask::mask),
        );

        Ok(())
    }

    /// Render a 1-bit image mask (stencil mask): paint current fill color
    /// where mask bits are set.
    fn render_image_mask(
        &mut self,
        info: &image::ImageInfo,
        data: &[u8],
        dict: &PdfDict,
    ) -> Result<()> {
        let w = info.width;
        let h = info.height;
        let pixel_count = (w * h) as usize;

        // Decode the mask data if it has filters
        let decoded_data = match dict.get(b"Filter") {
            Some(_) => match image::decode_image(data, dict, self.doc) {
                Ok(img) => img.data,
                Err(_) => data.to_vec(),
            },
            None => data.to_vec(),
        };

        // Get the Decode array to determine polarity
        // Default for ImageMask: [0 1] means 0=paint, 1=mask (transparent)
        let invert = dict
            .get_array(b"Decode")
            .map(|arr| {
                let d0 = arr.first().and_then(|o| o.as_f64()).unwrap_or(0.0);
                d0 != 0.0 // [1 0] means inverted
            })
            .unwrap_or(false);

        let fill_color = self.state.fill_color_rgba();
        let mut rgba = vec![0u8; pixel_count * 4];

        // Unpack bits: the data is 1 bit per pixel, packed MSB first
        for i in 0..pixel_count {
            let byte_idx = i / 8;
            let bit_idx = 7 - (i % 8);
            let bit = if byte_idx < decoded_data.len() {
                (decoded_data[byte_idx] >> bit_idx) & 1
            } else {
                0
            };

            // Determine if this pixel should be painted
            let paint = if invert { bit == 1 } else { bit == 0 };

            if paint {
                rgba[i * 4] = fill_color[0];
                rgba[i * 4 + 1] = fill_color[1];
                rgba[i * 4 + 2] = fill_color[2];
                rgba[i * 4 + 3] = fill_color[3];
            }
            // else: transparent (0,0,0,0)
        }

        let img_pixmap =
            match tiny_skia::Pixmap::from_vec(rgba, tiny_skia::IntSize::from_wh(w, h).unwrap()) {
                Some(p) => p,
                None => return Ok(()),
            };

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

        let bm = self.blend_mode();
        let mask = self.draw_mask();
        self.device.draw_image(
            &img_pixmap.as_ref(),
            full_transform.to_skia(),
            self.state.fill_alpha as f32,
            bm,
            mask.as_ref().map(SharedMask::mask),
        );

        Ok(())
    }

    /// Apply an SMask (soft mask) from the image's own /SMask entry to the RGBA data.
    fn apply_image_smask(
        &mut self,
        rgba: &mut [u8],
        w: u32,
        h: u32,
        smask_dict: &PdfDict,
        smask_data: &[u8],
    ) {
        let smask_decoded = match image::decode_image(smask_data, smask_dict, self.doc) {
            Ok(img) => img,
            Err(_) => return,
        };

        let pixel_count = (w * h) as usize;
        let mask_pixels = smask_decoded.data;

        // SMask is typically a grayscale image — use its values as alpha
        for i in 0..pixel_count {
            let mask_val = if smask_decoded.components == 1 {
                // Grayscale: direct alpha value
                if smask_decoded.bpc == 8 {
                    mask_pixels.get(i).copied().unwrap_or(255)
                } else if smask_decoded.bpc == 1 {
                    if mask_pixels.get(i).copied().unwrap_or(255) != 0 {
                        255
                    } else {
                        0
                    }
                } else {
                    mask_pixels.get(i).copied().unwrap_or(255)
                }
            } else {
                // Multi-component: use luminosity
                let idx = i * smask_decoded.components as usize;
                let r = mask_pixels.get(idx).copied().unwrap_or(255) as f32;
                let g = mask_pixels.get(idx + 1).copied().unwrap_or(255) as f32;
                let b = mask_pixels.get(idx + 2).copied().unwrap_or(255) as f32;
                (0.2126 * r + 0.7152 * g + 0.0722 * b).clamp(0.0, 255.0) as u8
            };

            // Multiply existing alpha with mask value
            let existing_alpha = rgba[i * 4 + 3] as u16;
            rgba[i * 4 + 3] = ((existing_alpha * mask_val as u16) / 255) as u8;
        }
    }

    /// Apply an explicit /Mask image (1-bit transparency mask) to the RGBA data.
    fn apply_image_explicit_mask(
        &mut self,
        rgba: &mut [u8],
        w: u32,
        h: u32,
        mask_dict: &PdfDict,
        mask_data: &[u8],
    ) {
        let mask_decoded = match image::decode_image(mask_data, mask_dict, self.doc) {
            Ok(img) => img,
            Err(_) => return,
        };

        let pixel_count = (w * h) as usize;

        // Scale mask to image dimensions if different
        let mask_w = mask_decoded.width as usize;
        let mask_h = mask_decoded.height as usize;
        let img_w = w as usize;
        let img_h = h as usize;

        for y in 0..img_h {
            for x in 0..img_w {
                let i = y * img_w + x;
                if i >= pixel_count {
                    break;
                }

                // Map to mask coordinates
                let mx = if mask_w > 0 { x * mask_w / img_w } else { 0 };
                let my = if mask_h > 0 { y * mask_h / img_h } else { 0 };
                let mi = my * mask_w + mx;

                let mask_bit = if mask_decoded.bpc == 1 {
                    // 1-bit packed
                    let byte_idx = mi / 8;
                    let bit_idx = 7 - (mi % 8);
                    if byte_idx < mask_decoded.data.len() {
                        (mask_decoded.data[byte_idx] >> bit_idx) & 1
                    } else {
                        1
                    }
                } else {
                    // 8bpc or higher
                    if mi < mask_decoded.data.len() {
                        if mask_decoded.data[mi] > 127 { 1 } else { 0 }
                    } else {
                        1
                    }
                };

                // In PDF, mask bit 0 = paint (opaque), 1 = do not paint (transparent)
                if mask_bit == 1 {
                    rgba[i * 4 + 3] = 0; // transparent
                }
            }
        }
    }

    fn render_inline_image(&mut self, _dict: &[(Vec<u8>, Operand)], _data: &[u8]) -> Result<()> {
        // TODO: implement inline image rendering
        Ok(())
    }

    fn render_shading(&mut self, name: &[u8]) -> Result<()> {
        let sh_obj = match self.resources.lookup(self.doc, b"Shading", name)? {
            Some(found) => found.object,
            None => return Ok(()),
        };

        // Extract dict and optional stream data from the shading object
        let (sh_dict, stream_data) = match &sh_obj {
            PdfObject::Dict(d) => (d.clone(), None),
            PdfObject::Stream { dict, data } => {
                let decoded = self.doc.decode_stream(dict, data).ok();
                (dict.clone(), decoded)
            }
            _ => return Ok(()),
        };

        let resolved_dict = sh_dict;

        let mask = self.draw_mask();
        let cs = shading_colour_space(&resolved_dict, self.doc);
        let function = resolved_dict
            .get(b"Function")
            .and_then(|f| PdfFunction::resolve(f, self.doc));
        crate::shading::render_shading(
            &mut self.device.pixmap,
            &resolved_dict,
            &self.state.ctm,
            &self.page_transform,
            mask.as_ref().map(SharedMask::mask),
            stream_data.as_deref(),
            &cs,
            function.as_ref(),
        );

        Ok(())
    }

    fn render_form_xobject(&mut self, dict: &PdfDict, data: &[u8]) -> Result<()> {
        // Check if this form has a transparency group
        let has_transparency_group = self
            .dict_entry(dict, b"Group")
            .is_some_and(|group| group.get_name(b"S") == Some(b"Transparency"));

        if has_transparency_group {
            return self.render_transparency_group(dict, data);
        }
        self.render_form_xobject_direct(dict, data)
    }

    /// Render a transparency group: draw content into a temporary pixmap,
    /// then composite onto the main device.
    fn render_transparency_group(&mut self, dict: &PdfDict, data: &[u8]) -> Result<()> {
        let w = self.device.pixmap.width();
        let h = self.device.pixmap.height();

        // Create temporary pixmap for the group
        let mut temp_pixmap = match Pixmap::new(w, h) {
            Some(p) => p,
            None => {
                // Fallback: render directly (non-grouped)
                return self.render_form_xobject_direct(dict, data);
            }
        };
        let ops = parse_content_stream(data).map_err(RenderError::Core)?;

        // Check if group is isolated (starts with transparent backdrop)
        let is_isolated = self
            .dict_entry(dict, b"Group")
            .and_then(|group| group.get(b"I").and_then(|o| o.as_bool()))
            .unwrap_or(false);

        // If not isolated, copy the current pixmap as backdrop
        if !is_isolated {
            temp_pixmap
                .data_mut()
                .copy_from_slice(self.device.pixmap.data());
        }

        // Draw the content into the temp pixmap, unclipped (it is clipped when
        // composited) and with blend Normal, alpha 1 and no soft mask, which
        // apply once, when the group is composited
        std::mem::swap(&mut self.device.pixmap, &mut temp_pixmap);
        self.nested_stream(|this| {
            this.state.clip = None;
            this.reset_transparency();
            this.state.ctm = form_matrix(dict).concat(&this.state.ctm);
            let _ = this.execute_ops(&ops);
        });
        std::mem::swap(&mut self.device.pixmap, &mut temp_pixmap);

        // Composite the group result onto the main pixmap
        let alpha = self.state.fill_alpha as f32;
        let bm = self.blend_mode();
        let mask = self.draw_mask();
        self.device.draw_pixmap(
            &temp_pixmap.as_ref(),
            Transform::identity(),
            alpha,
            bm,
            mask.as_ref().map(SharedMask::mask),
        );

        Ok(())
    }

    /// Render a form xobject directly (without transparency group handling).
    fn render_form_xobject_direct(&mut self, dict: &PdfDict, data: &[u8]) -> Result<()> {
        let ops = parse_content_stream(data).map_err(RenderError::Core)?;
        self.nested_stream(|this| {
            this.state.ctm = form_matrix(dict).concat(&this.state.ctm);
            this.execute_ops(&ops)
        })
    }

    /// Runs `f` as a nested content stream: on a copy of the graphics state
    /// with no current path and no hidden optional-content section open,
    /// where `Q` cannot pop below the state on entry.
    /// Afterwards the stack is unwound to its depth on entry and the caller's
    /// state and path are restored, whatever `f` returned.
    fn nested_stream<T>(&mut self, f: impl FnOnce(&mut Self) -> T) -> T {
        let depth = self.state_stack.len();
        self.state_stack.push(self.state.clone());
        let floor = std::mem::replace(&mut self.state_floor, depth + 1);
        let path = self.path_builder.take();
        let oc_skip = std::mem::replace(&mut self.oc_skip_depth, 0);
        let out = f(self);
        self.oc_skip_depth = oc_skip;
        self.state_stack.truncate(depth + 1);
        if let Some(s) = self.state_stack.pop() {
            self.state = s;
        }
        self.state_floor = floor;
        self.path_builder = path;
        out
    }

    /// Sets blend mode Normal, alpha 1 and no soft mask, the state a soft mask
    /// form or a transparency group starts from (ISO 32000-2 §11.6.6).
    fn reset_transparency(&mut self) {
        self.state.blend_mode = PdfBlendMode::Normal;
        self.state.fill_alpha = 1.0;
        self.state.stroke_alpha = 1.0;
        self.state.soft_mask = None;
    }

    /// The mask one draw is painted through: the clip, the soft mask, or their
    /// product when both are set. It is never stored in the graphics state,
    /// so the soft mask does not enter the clip. The product is kept for the
    /// next draw under the same clip and soft mask.
    fn draw_mask(&mut self) -> Option<SharedMask> {
        let clip = self.state.clip.clone();
        let Some(soft) = self.state.soft_mask.as_ref().map(|s| s.mask.clone()) else {
            return clip;
        };
        let Some(clip) = clip else {
            return Some(soft);
        };
        if let Some((c, s, product)) = &self.combined_mask
            && c.ptr_eq(&clip)
            && s.ptr_eq(&soft)
        {
            return Some(product.clone());
        }
        let mut combined = clip.mask().clone();
        for (c, m) in combined.data_mut().iter_mut().zip(soft.mask().data()) {
            *c = ((*c as u16 * *m as u16) / 255) as u8;
        }
        let product = SharedMask::new(combined);
        self.combined_mask = Some((clip, soft, product.clone()));
        Some(product)
    }

    /// The colour space `cs`/`CS` name: a device space by name, else the
    /// `/ColorSpace` resource of that name in the current scope, resolved
    /// (DeviceRGB when there is none). `None` for `/Pattern`.
    fn colour_space(&self, name: &[u8]) -> Option<ColorSpace> {
        match name {
            b"DeviceGray" | b"G" => return Some(ColorSpace::DeviceGray),
            b"DeviceRGB" | b"RGB" => return Some(ColorSpace::DeviceRGB),
            b"DeviceCMYK" | b"CMYK" => return Some(ColorSpace::DeviceCMYK),
            b"Pattern" => return None,
            _ => {}
        }
        match self.resources.lookup(self.doc, b"ColorSpace", name) {
            Ok(Some(found)) => Some(ColorSpace::resolve(&found.object, self.doc)),
            _ => Some(ColorSpace::DeviceRGB),
        }
    }

    /// `dict[key]` as a dictionary, through an indirect reference when it is
    /// one.
    fn dict_entry(&self, dict: &PdfDict, key: &[u8]) -> Option<PdfDict> {
        match dict.get(key)? {
            PdfObject::Dict(d) => Some(d.clone()),
            PdfObject::Reference(r) => match self.doc.resolve(r).ok()? {
                PdfObject::Dict(d) => Some(d),
                _ => None,
            },
            _ => None,
        }
    }

    fn apply_extgstate(&mut self, name: &[u8]) -> Result<()> {
        let gs_obj = match self.resources.lookup(self.doc, b"ExtGState", name)? {
            Some(found) => found.object,
            None => return Ok(()),
        };

        if let PdfObject::Dict(gs_dict) = &gs_obj {
            if let Some(PdfObject::Array(font)) = gs_dict.get(b"Font")
                && let [PdfObject::Reference(font_ref), size] = font.as_slice()
                && let Some(size) = size.as_f64()
                && let Some(key) = self.select_font_ref(font_ref)
            {
                self.state.text.font = Some(key);
                self.state.text.font_size = size;
            }
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
                Some(_) => {
                    if let Some(smask_dict) = self.dict_entry(gs_dict, b"SMask") {
                        let _ = self.apply_soft_mask(smask_dict);
                    }
                }
                None => {}
            }
        }

        Ok(())
    }

    /// Resolve and render a soft mask from an SMask dictionary.
    fn apply_soft_mask(&mut self, smask_dict: PdfDict) -> Result<()> {
        // /S: Luminosity or Alpha
        let subtype = match smask_dict.get_name(b"S") {
            Some(b"Luminosity") => SoftMaskSubtype::Luminosity,
            Some(b"Alpha") => SoftMaskSubtype::Alpha,
            _ => return Ok(()), // unsupported subtype
        };

        // /G: form XObject reference for the mask
        let (form_ref, form_obj) = match smask_dict.get(b"G") {
            Some(PdfObject::Reference(r)) => {
                let r = r.clone();
                match self.doc.resolve(&r) {
                    Ok(obj) => (Some(r), obj),
                    Err(_) => return Ok(()),
                }
            }
            Some(other) => (None, other.clone()),
            None => return Ok(()),
        };

        let (form_dict, form_data) = match form_obj {
            PdfObject::Stream { dict, data } => (dict, data),
            _ => return Ok(()),
        };
        let backdrop = match subtype {
            SoftMaskSubtype::Luminosity => self.mask_backdrop(&smask_dict, &form_dict),
            SoftMaskSubtype::Alpha => [0.0; 3],
        };
        let transfer = self.transfer_table(&smask_dict);

        self.with_running_stream(form_ref.as_ref(), |this| {
            let form_data = match this.doc.decode_stream(&form_dict, &form_data) {
                Ok(decoded) => decoded,
                Err(_) => return Ok(()),
            };
            this.with_stream_resources(&form_dict, form_ref.as_ref(), |this| {
                this.render_soft_mask(subtype, &form_dict, &form_data, backdrop, transfer)
            })
        })
        .unwrap_or(Ok(()))
    }

    /// `obj`, through an indirect reference when it is one.
    fn resolved(&self, obj: &PdfObject) -> Option<PdfObject> {
        match obj {
            PdfObject::Reference(r) => self.doc.resolve(r).ok(),
            other => Some(other.clone()),
        }
    }

    /// The RGB backdrop a Luminosity soft mask's group is composited over:
    /// the soft-mask dictionary's `/BC` in the group's `/CS` (resolved, so an
    /// ICCBased space converts through its `/Alternate` or `/N`), black
    /// without one. Without a usable `/CS`, or when its component count
    /// differs from `/BC`'s, the space is taken from that count: gray, RGB or
    /// CMYK.
    fn mask_backdrop(&self, smask: &PdfDict, form: &PdfDict) -> [f64; 3] {
        let Some(PdfObject::Array(bc)) = smask.get(b"BC").and_then(|o| self.resolved(o)) else {
            return [0.0; 3];
        };
        let components: Vec<f64> = bc.iter().filter_map(PdfObject::as_f64).collect();
        let group_cs = self
            .dict_entry(form, b"Group")
            .and_then(|group| group.get(b"CS").map(|cs| ColorSpace::resolve(cs, self.doc)));
        let cs = match group_cs {
            Some(cs) if cs.num_components() == components.len() => cs,
            _ => match components.len() {
                1 => ColorSpace::DeviceGray,
                3 => ColorSpace::DeviceRGB,
                4 => ColorSpace::DeviceCMYK,
                _ => return [0.0; 3],
            },
        };
        PdfColor { components }.to_rgb(&cs)
    }

    /// The soft-mask dictionary's `/TR` as a table from mask value to mask
    /// value. `None` for `Identity`, no `/TR`, and a function that does not
    /// parse.
    fn transfer_table(&self, smask: &PdfDict) -> Option<[u8; 256]> {
        let function = PdfFunction::resolve(smask.get(b"TR")?, self.doc)?;
        let mut table = [0u8; 256];
        for (i, out) in table.iter_mut().enumerate() {
            let v = i as f64 / 255.0;
            let mapped = function.evaluate(&[v]).first().copied().unwrap_or(v);
            *out = (mapped.clamp(0.0, 1.0) * 255.0).round() as u8;
        }
        Some(table)
    }

    /// Render a soft mask form into a mask and install it as the current
    /// soft mask.
    fn render_soft_mask(
        &mut self,
        subtype: SoftMaskSubtype,
        form_dict: &PdfDict,
        form_data: &[u8],
        backdrop: [f64; 3],
        transfer: Option<[u8; 256]>,
    ) -> Result<()> {
        let w = self.device.pixmap.width();
        let h = self.device.pixmap.height();

        // Create a temporary pixmap to render the mask form into
        let mut mask_pixmap = match Pixmap::new(w, h) {
            Some(p) => p,
            None => return Ok(()),
        };

        // A luminosity mask's group is composited over its backdrop colour
        if subtype == SoftMaskSubtype::Luminosity {
            let [r, g, b] = backdrop.map(|c| c.clamp(0.0, 1.0) as f32);
            mask_pixmap
                .fill(tiny_skia::Color::from_rgba(r, g, b, 1.0).unwrap_or(tiny_skia::Color::BLACK));
        }

        // Swap in the mask pixmap for rendering
        std::mem::swap(&mut self.device.pixmap, &mut mask_pixmap);

        // Render the mask form unclipped, with blend Normal, alpha 1 and no
        // soft mask
        self.nested_stream(|this| {
            this.state.clip = None;
            this.reset_transparency();
            this.state.ctm = form_matrix(form_dict).concat(&this.state.ctm);
            if let Ok(ops) = parse_content_stream(form_data) {
                let _ = this.execute_ops(&ops);
            }
        });

        // Swap back
        std::mem::swap(&mut self.device.pixmap, &mut mask_pixmap);

        // Convert the rendered mask pixmap to a tiny_skia::Mask
        if let Some(mut mask) = self.pixmap_to_mask(&mask_pixmap, subtype) {
            if let Some(table) = transfer {
                for v in mask.data_mut() {
                    *v = table[*v as usize];
                }
            }
            self.state.soft_mask = Some(SoftMask {
                mask: SharedMask::new(mask),
                subtype,
            });
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

        for (i, out) in mask_data.iter_mut().enumerate().take((w * h) as usize) {
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
                SoftMaskSubtype::Alpha => src_data[idx + 3] as f32,
            };
            *out = value as u8;
        }

        Some(mask)
    }

    // --- Pattern rendering ---

    /// Render a tiling pattern cell and return the pixmap.
    fn render_tiling_pattern(
        &mut self,
        pattern_dict: &PdfDict,
        pattern_data: &[u8],
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
        #[allow(clippy::manual_clamp)]
        let cell_w = (xstep * sx).ceil().max(1.0).min(2048.0) as u32;
        #[allow(clippy::manual_clamp)]
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
        let scale_to_device = Matrix::scale(cell_w as f64 / xstep, cell_h as f64 / ystep);
        let cell_transform = pattern_matrix.concat(&scale_to_device);

        // Swap in the cell pixmap
        std::mem::swap(&mut self.device.pixmap, &mut cell_pixmap);

        // Render the pattern content stream into the cell, without the page's
        // clip and soft mask
        let saved_page_transform = self.page_transform;
        self.page_transform = cell_transform;
        self.nested_stream(|this| {
            this.state.ctm = Matrix::identity();
            this.state.clip = None;
            this.state.soft_mask = None;
            if let Ok(ops) = parse_content_stream(pattern_data) {
                let _ = this.execute_ops(&ops);
            }
        });
        self.page_transform = saved_page_transform;

        // Swap back
        std::mem::swap(&mut self.device.pixmap, &mut cell_pixmap);

        Ok(Some(cell_pixmap))
    }

    /// Try to fill a path using the current fill pattern, if one is set.
    /// Returns true if pattern fill was performed.
    fn try_fill_with_pattern(&mut self, path: &tiny_skia::Path, rule: FillRule) -> bool {
        let pattern = match &self.state.fill_pattern {
            Some(pattern) => pattern.clone(),
            None => return false,
        };

        if let Ok(Some(pattern_pixmap)) = self.render_pattern(&pattern) {
            let transform = self.effective_transform();
            let bm = self.blend_mode();
            let mask = self.draw_mask();
            self.device.fill_path_with_pattern(
                path,
                rule,
                transform,
                &pattern_pixmap.as_ref(),
                Transform::identity(),
                bm,
                mask.as_ref().map(SharedMask::mask),
            );
            true
        } else {
            false
        }
    }

    /// Try to stroke a path using the current stroke pattern, if one is set.
    /// Returns true if pattern stroke was performed.
    fn try_stroke_with_pattern(&mut self, path: &tiny_skia::Path) -> bool {
        let pattern = match &self.state.stroke_pattern {
            Some(pattern) => pattern.clone(),
            None => return false,
        };

        if let Ok(Some(pattern_pixmap)) = self.render_pattern(&pattern) {
            let transform = self.effective_transform();
            let bm = self.blend_mode();
            let mask = self.draw_mask();
            self.device.stroke_path_with_pattern(
                path,
                transform,
                &self.state,
                &pattern_pixmap.as_ref(),
                Transform::identity(),
                bm,
                mask.as_ref().map(SharedMask::mask),
            );
            true
        } else {
            false
        }
    }

    /// Render a selected pattern. Handles both tiling and shading patterns.
    fn render_pattern(&mut self, pattern: &PatternSelection) -> Result<Option<Pixmap>> {
        let pattern_ref = pattern.obj_ref.as_ref();
        match &*pattern.object {
            PdfObject::Stream { dict, data } => {
                let pattern_type = dict.get_i64(b"PatternType").unwrap_or(0);
                match pattern_type {
                    1 => {
                        // Tiling pattern
                        self.with_running_stream(pattern_ref, |this| {
                            let decoded = match this.doc.decode_stream(dict, data) {
                                Ok(d) => d,
                                Err(_) => return Ok(None),
                            };
                            this.with_stream_resources(dict, pattern_ref, |this| {
                                this.render_tiling_pattern(dict, &decoded)
                            })
                        })
                        .unwrap_or(Ok(None))
                    }
                    2 => {
                        // Shading pattern: render shading into a temp pixmap
                        self.render_shading_pattern(dict)
                    }
                    _ => Ok(None),
                }
            }
            PdfObject::Dict(dict) => {
                let pattern_type = dict.get_i64(b"PatternType").unwrap_or(0);
                if pattern_type == 2 {
                    self.render_shading_pattern(dict)
                } else {
                    Ok(None)
                }
            }
            _ => Ok(None),
        }
    }

    /// Render a shading pattern (PatternType 2) into a pixmap.
    fn render_shading_pattern(&mut self, pattern_dict: &PdfDict) -> Result<Option<Pixmap>> {
        // Get the shading dict from the pattern
        let shading_obj = match pattern_dict.get(b"Shading") {
            Some(PdfObject::Reference(r)) => {
                let r = r.clone();
                match self.doc.resolve(&r) {
                    Ok(obj) => obj,
                    Err(_) => return Ok(None),
                }
            }
            Some(other) => other.clone(),
            None => return Ok(None),
        };

        // Extract dict and optional stream data
        let (shading_dict, stream_data) = match &shading_obj {
            PdfObject::Dict(d) => (d.clone(), None),
            PdfObject::Stream { dict, data } => {
                let decoded = self.doc.decode_stream(dict, data).ok();
                (dict.clone(), decoded)
            }
            _ => return Ok(None),
        };

        let resolved_shading = shading_dict;

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

        let clip = self.state.clip.as_ref().map(SharedMask::mask);
        let cs = shading_colour_space(&resolved_shading, self.doc);
        let function = resolved_shading
            .get(b"Function")
            .and_then(|f| PdfFunction::resolve(f, self.doc));
        crate::shading::render_shading(
            &mut shading_pixmap,
            &resolved_shading,
            &effective_ctm,
            &self.page_transform,
            clip,
            stream_data.as_deref(),
            &cs,
            function.as_ref(),
        );

        Ok(Some(shading_pixmap))
    }
}

enum XObjectData {
    Image {
        dict: PdfDict,
        data: Vec<u8>,
    },
    Form {
        obj_ref: IndirectRef,
        dict: PdfDict,
        data: Vec<u8>,
    },
}

/// Convert decoded image data to RGBA bytes.
/// An annotation's decoded appearance stream: its reference (when it is
/// indirect), dictionary and content.
struct Appearance {
    obj_ref: Option<IndirectRef>,
    dict: PdfDict,
    data: Vec<u8>,
}

/// A shading dictionary's `/ColorSpace`, resolved; DeviceRGB without one.
fn shading_colour_space(dict: &PdfDict, doc: &PdfDocument) -> ColorSpace {
    dict.get(b"ColorSpace")
        .map(|cs| ColorSpace::resolve(cs, doc))
        .unwrap_or(ColorSpace::DeviceRGB)
}

/// `[a b c d e f]` as a [`Matrix`].
fn matrix_of([a, b, c, d, e, f]: [f64; 6]) -> Matrix {
    Matrix { a, b, c, d, e, f }
}

/// A form dictionary's `/Matrix`, identity when absent.
fn form_matrix(dict: &PdfDict) -> Matrix {
    matrix_of(justpdf_core::annot::placement::form_matrix(dict))
}

/// The matrix A of ISO 32000-2 §12.5.5 mapping the appearance form `dict`
/// onto `rect`; `None` when its box is degenerate.
fn appearance_mapping(dict: &PdfDict, rect: &justpdf_core::page::Rect) -> Option<Matrix> {
    justpdf_core::annot::placement::appearance_matrix(dict, rect).map(matrix_of)
}

fn image_to_rgba(img: &image::DecodedImage) -> Vec<u8> {
    img.to_rgb8()
        .chunks_exact(3)
        .flat_map(|rgb| [rgb[0], rgb[1], rgb[2], 255])
        .collect()
}

/// Parse a CIDToGIDMap stream: 2 bytes (big-endian) per CID entry.
fn parse_cid_gid_stream(data: &[u8]) -> Vec<u16> {
    data.chunks(2)
        .map(|c| {
            if c.len() == 2 {
                ((c[0] as u16) << 8) | (c[1] as u16)
            } else {
                c[0] as u16
            }
        })
        .collect()
}

/// Get f64 from operands at index.
fn f(operands: &[Operand], idx: usize) -> f64 {
    operands.get(idx).and_then(|o| o.as_f64()).unwrap_or(0.0)
}
