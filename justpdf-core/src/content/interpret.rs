//! Content-stream interpreter shared by text extraction and redaction.
//!
//! Walks parsed content operations, tracks the graphics and text state
//! (ISO 32000-2 §8.4, §9.3), and reports each operation with the state in
//! effect and each shown glyph with its position and box.

use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Range;
use std::rc::Rc;

use crate::content::{ContentOp, Operand, parse_content_stream};
use crate::font::{
    Encoding, FontInfo, ToUnicodeCMap, descendant_font, parse_font_info, resolve_font_entries,
};
use crate::object::{IndirectRef, PdfDict, PdfObject};
use crate::parser::PdfDocument;

// ---------------------------------------------------------------------------
// 2D affine transformation matrix [a b 0; c d 0; e f 1]
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
pub struct Matrix {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub e: f64,
    pub f: f64,
}

impl Matrix {
    pub fn identity() -> Self {
        Self {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            e: 0.0,
            f: 0.0,
        }
    }

    /// Multiply: self × other (row-vector convention as in PDF spec).
    pub fn concat(&self, other: &Matrix) -> Matrix {
        Matrix {
            a: self.a * other.a + self.b * other.c,
            b: self.a * other.b + self.b * other.d,
            c: self.c * other.a + self.d * other.c,
            d: self.c * other.b + self.d * other.d,
            e: self.e * other.a + self.f * other.c + other.e,
            f: self.e * other.b + self.f * other.d + other.f,
        }
    }

    /// Transform a point (x, y) by this matrix.
    pub fn transform_point(&self, x: f64, y: f64) -> (f64, f64) {
        (
            self.a * x + self.c * y + self.e,
            self.b * x + self.d * y + self.f,
        )
    }

    /// Translation matrix.
    pub fn translate(tx: f64, ty: f64) -> Self {
        Self {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            e: tx,
            f: ty,
        }
    }

    /// Get the effective font size (y-scale factor).
    pub fn font_size_scale(&self) -> f64 {
        (self.b * self.b + self.d * self.d).sqrt()
    }

    fn from_operands(operands: &[Operand]) -> Option<Matrix> {
        if operands.len() < 6 {
            return None;
        }
        Some(Matrix {
            a: operands[0].as_f64()?,
            b: operands[1].as_f64()?,
            c: operands[2].as_f64()?,
            d: operands[3].as_f64()?,
            e: operands[4].as_f64()?,
            f: operands[5].as_f64()?,
        })
    }

    fn from_objects(array: &[PdfObject]) -> Option<Matrix> {
        if array.len() != 6 {
            return None;
        }
        let n = |i: usize| array[i].as_f64();
        Some(Matrix {
            a: n(0)?,
            b: n(1)?,
            c: n(2)?,
            d: n(3)?,
            e: n(4)?,
            f: n(5)?,
        })
    }
}

// ---------------------------------------------------------------------------
// Fonts
// ---------------------------------------------------------------------------

/// A font dictionary interpreted for positioning and decoding glyphs.
#[derive(Debug, Clone)]
pub(crate) struct ResolvedFont {
    pub(crate) info: FontInfo,
    pub(crate) cmap: Option<ToUnicodeCMap>,
}

impl ResolvedFont {
    /// Width of a character code in glyph space (1/1000 text space units).
    pub(crate) fn char_width(&self, code: u32) -> f64 {
        self.info.widths.get_width(code)
    }

    /// Whether codes are read two bytes at a time.
    fn is_two_byte(&self) -> bool {
        matches!(self.info.encoding, Encoding::Identity) || self.info.subtype == b"Type0"
    }

    /// Bottom and top of the glyph box in glyph space (1/1000 units):
    /// the `/FontBBox` height, or −250…1000 without a usable one.
    fn glyph_height(&self) -> (f64, f64) {
        match self.info.descriptor.as_ref().and_then(|d| d.font_b_box) {
            Some([_, y0, _, y1]) if y0 != y1 => (y0.min(y1), y0.max(y1)),
            _ => DEFAULT_GLYPH_HEIGHT,
        }
    }
}

const DEFAULT_GLYPH_HEIGHT: (f64, f64) = (-250.0, 1000.0);

/// Interpret a font dictionary: widths (`/W` for a Type0's descendant),
/// encoding, descriptor and ToUnicode.
pub(crate) fn load_font(doc: &PdfDocument, font_dict: &PdfDict) -> ResolvedFont {
    let font_dict = resolve_font_entries(font_dict, |r| doc.resolve(r).ok());
    let mut info = parse_font_info(&font_dict);

    let cmap = resolve_to_unicode(doc, &font_dict);
    if cmap.is_some() {
        info.to_unicode = None;
    }

    if font_dict.get_name(b"Subtype") == Some(b"Type0") {
        resolve_type0_descendant(doc, &font_dict, &mut info);
    }

    ResolvedFont { info, cmap }
}

fn resolve_to_unicode(doc: &PdfDocument, font_dict: &PdfDict) -> Option<ToUnicodeCMap> {
    let tu_obj = font_dict.get(b"ToUnicode")?;
    match tu_obj {
        PdfObject::Reference(r) => {
            let obj = doc.resolve(r).ok()?;
            match obj {
                PdfObject::Stream { dict, data } => {
                    let decoded = doc.decode_stream(&dict, &data).ok()?;
                    Some(ToUnicodeCMap::parse(&decoded))
                }
                _ => None,
            }
        }
        PdfObject::Stream { dict, data } => {
            let decoded = doc.decode_stream(dict, data).ok()?;
            Some(ToUnicodeCMap::parse(&decoded))
        }
        _ => None,
    }
}

fn resolve_type0_descendant(doc: &PdfDocument, font_dict: &PdfDict, info: &mut FontInfo) {
    let Some(descendant) = descendant_font(font_dict, |r| doc.resolve(r).ok()) else {
        return;
    };
    let descendant = resolve_font_entries(&descendant, |r| doc.resolve(r).ok());
    let descendant_info = parse_font_info(&descendant);
    info.widths = descendant_info.widths;
    if info.descriptor.is_none() {
        info.descriptor = descendant_info.descriptor;
    }
    info.encoding = Encoding::Identity;
}

// ---------------------------------------------------------------------------
// Resources
// ---------------------------------------------------------------------------

/// A Form XObject ready to be interpreted.
pub(crate) struct FormXObject<'r> {
    /// The form stream's object, for cycle detection.
    pub(crate) id: IndirectRef,
    pub(crate) ops: Vec<ContentOp>,
    /// `/Matrix`, form space to user space.
    pub(crate) matrix: Matrix,
    /// The resources names inside the form resolve against.
    pub(crate) resources: Box<dyn ContentResources + 'r>,
}

/// Where the interpreter looks up resource names.
pub(crate) trait ContentResources {
    /// The font named `name` in the `/Font` category.
    fn font(&mut self, name: &[u8]) -> Option<Rc<ResolvedFont>>;

    /// The font and size the `/Font [font size]` entry of the ExtGState
    /// named `name` sets; `None` when the entry is missing or malformed.
    fn ext_gstate_font(&mut self, name: &[u8]) -> Option<(Rc<ResolvedFont>, f64)>;

    /// The Form XObject named `name` in the `/XObject` category. `None` for
    /// an image, a missing name or a stream that does not decode.
    fn form(&mut self, name: &[u8]) -> Option<FormXObject<'_>>;
}

/// Resources read from a document: a page's `/Resources`, and inside a Form
/// XObject the form's own `/Resources` in front of its caller's.
pub(crate) struct DocResources<'d> {
    doc: &'d PdfDocument,
    /// `/Resources` dictionaries, innermost last.
    scopes: Vec<PdfDict>,
    fonts_by_ref: Rc<RefCell<HashMap<IndirectRef, Option<Rc<ResolvedFont>>>>>,
    /// What each font name resolved to in this scope chain.
    fonts_by_name: HashMap<Vec<u8>, Option<Rc<ResolvedFont>>>,
}

impl<'d> DocResources<'d> {
    /// The resources of a page, from its (possibly indirect) `/Resources`.
    pub(crate) fn page(doc: &'d PdfDocument, resources: Option<&PdfObject>) -> Self {
        let mut scopes = Vec::new();
        if let Some(dict) = resources.and_then(|o| resolve_dict(doc, o)) {
            scopes.push(dict);
        }
        Self {
            doc,
            scopes,
            fonts_by_ref: Rc::default(),
            fonts_by_name: HashMap::new(),
        }
    }

    fn child(&self, form_resources: Option<PdfDict>) -> Self {
        let mut scopes = self.scopes.clone();
        scopes.extend(form_resources);
        Self {
            doc: self.doc,
            scopes,
            fonts_by_ref: Rc::clone(&self.fonts_by_ref),
            fonts_by_name: HashMap::new(),
        }
    }

    /// The font dictionary `r`, loaded once per page.
    fn font_by_ref(&self, r: &IndirectRef) -> Option<Rc<ResolvedFont>> {
        let doc = self.doc;
        self.fonts_by_ref
            .borrow_mut()
            .entry(r.clone())
            .or_insert_with(|| match doc.resolve(r) {
                Ok(PdfObject::Dict(d)) => Some(Rc::new(load_font(doc, &d))),
                _ => None,
            })
            .clone()
    }

    /// The entry `name` in resource category `category`, innermost scope first.
    fn lookup(&self, category: &[u8], name: &[u8]) -> Option<PdfObject> {
        self.scopes.iter().rev().find_map(|scope| {
            let entries = resolve_dict(self.doc, scope.get(category)?)?;
            entries.get(name).cloned()
        })
    }
}

impl ContentResources for DocResources<'_> {
    fn font(&mut self, name: &[u8]) -> Option<Rc<ResolvedFont>> {
        if let Some(font) = self.fonts_by_name.get(name) {
            return font.clone();
        }
        let font = match self.lookup(b"Font", name) {
            Some(PdfObject::Reference(r)) => self.font_by_ref(&r),
            Some(PdfObject::Dict(d)) => Some(Rc::new(load_font(self.doc, &d))),
            _ => None,
        };
        self.fonts_by_name.insert(name.to_vec(), font.clone());
        font
    }

    fn ext_gstate_font(&mut self, name: &[u8]) -> Option<(Rc<ResolvedFont>, f64)> {
        let ext_gstate = resolve_dict(self.doc, &self.lookup(b"ExtGState", name)?)?;
        let Some(PdfObject::Array(entry)) = ext_gstate.get(b"Font") else {
            return None;
        };
        let [PdfObject::Reference(font), size] = entry.as_slice() else {
            return None;
        };
        let size = size.as_f64()?;
        Some((self.font_by_ref(font)?, size))
    }

    fn form(&mut self, name: &[u8]) -> Option<FormXObject<'_>> {
        let PdfObject::Reference(id) = self.lookup(b"XObject", name)? else {
            return None;
        };
        let PdfObject::Stream { dict, data } = self.doc.resolve(&id).ok()? else {
            return None;
        };
        if dict.get_name(b"Subtype") != Some(b"Form") {
            return None;
        }
        let decoded = self.doc.decode_stream(&dict, &data).ok()?;
        let ops = parse_content_stream(&decoded).ok()?;
        let matrix = dict
            .get_array(b"Matrix")
            .and_then(Matrix::from_objects)
            .unwrap_or_else(Matrix::identity);
        let form_resources = dict
            .get(b"Resources")
            .and_then(|o| resolve_dict(self.doc, o));
        Some(FormXObject {
            id,
            ops,
            matrix,
            resources: Box::new(self.child(form_resources)),
        })
    }
}

fn resolve_dict(doc: &PdfDocument, obj: &PdfObject) -> Option<PdfDict> {
    match obj {
        PdfObject::Dict(d) => Some(d.clone()),
        PdfObject::Reference(r) => match doc.resolve(r) {
            Ok(PdfObject::Dict(d)) => Some(d),
            _ => None,
        },
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

/// Text state parameters (ISO 32000-2 §9.3).
#[derive(Debug, Clone)]
pub(crate) struct TextState {
    /// Character spacing (Tc).
    pub(crate) char_spacing: f64,
    /// Word spacing (Tw).
    pub(crate) word_spacing: f64,
    /// Horizontal scaling (Tz) as a fraction (1.0 = 100%).
    pub(crate) horiz_scaling: f64,
    /// Leading (TL).
    pub(crate) leading: f64,
    /// Resource name the font was selected by; empty for a font an
    /// ExtGState's `/Font` set.
    pub(crate) font_name: Vec<u8>,
    /// The font selected by `Tf` (resolved when `Tf` ran) or by an
    /// ExtGState's `/Font`.
    pub(crate) font: Option<Rc<ResolvedFont>>,
    /// Font size (Tfs).
    pub(crate) font_size: f64,
    /// Text rise (Ts).
    pub(crate) text_rise: f64,
    /// Text rendering mode (Tr).
    pub(crate) render_mode: i64,
}

impl Default for TextState {
    fn default() -> Self {
        Self {
            char_spacing: 0.0,
            word_spacing: 0.0,
            horiz_scaling: 1.0,
            leading: 0.0,
            font_name: Vec::new(),
            font: None,
            font_size: 12.0,
            text_rise: 0.0,
            render_mode: 0,
        }
    }
}

/// The part of the graphics state the interpreter tracks.
#[derive(Debug, Clone)]
pub(crate) struct GraphicsState {
    /// Current transformation matrix, user space to the page's default space.
    pub(crate) ctm: Matrix,
    pub(crate) text: TextState,
}

impl GraphicsState {
    fn new(ctm: Matrix) -> Self {
        Self {
            ctm,
            text: TextState::default(),
        }
    }
}

// ---------------------------------------------------------------------------
// Output
// ---------------------------------------------------------------------------

/// One glyph shown by a text-showing operator.
#[derive(Debug)]
pub(crate) struct Glyph<'a> {
    /// Index of the showing operation in its content stream.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) op_index: usize,
    /// Index of the string within a `TJ` array; `None` for `Tj`, `'` and `"`.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) tj_index: Option<usize>,
    /// The code's bytes within that string.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) byte_range: Range<usize>,
    /// The code's bytes.
    pub(crate) bytes: &'a [u8],
    pub(crate) code: u32,
    /// The font in effect; `None` when `Tf` named no font that resolves.
    pub(crate) font: Option<&'a ResolvedFont>,
    /// Glyph width in glyph space (1/1000 text space units).
    pub(crate) width: f64,
    /// Text rendering matrix at the glyph's origin (glyph space × Tfs, Th,
    /// Ts, then Tm and the CTM).
    pub(crate) trm: Matrix,
    /// The glyph box — advance width by the `/FontBBox` height (−0.25…1.0
    /// without one) — through `trm`: lower-left, lower-right, upper-right,
    /// upper-left.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) quad: [(f64, f64); 4],
}

/// Why a `Do` naming a Form XObject was not entered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FormRefusal {
    /// Already [`MAX_FORM_DEPTH`] forms deep.
    Depth,
    /// The form is already being interpreted further out.
    Cycle,
}

/// Forms nested deeper than this are not entered.
pub(crate) const MAX_FORM_DEPTH: usize = 10;

/// Receives what the interpreter reports. Every method defaults to nothing.
pub(crate) trait ContentVisitor {
    /// Operation `index` of the current stream, with the state in effect
    /// before it runs.
    fn op(&mut self, _index: usize, _op: &ContentOp, _state: &GraphicsState) {}

    /// A glyph, with the state it is shown in.
    fn glyph(&mut self, _glyph: &Glyph<'_>, _state: &GraphicsState) {}

    /// The `Do` at `op_index` enters the form `id`; operations reported until
    /// the matching [`leave_form`](Self::leave_form) are the form's.
    fn enter_form(&mut self, _op_index: usize, _id: &IndirectRef) {}

    fn leave_form(&mut self) {}

    /// The `Do` at `op_index` names a form that is not entered.
    fn form_refused(&mut self, _op_index: usize, _reason: FormRefusal) {}
}

/// Interpreter options.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct InterpretOptions {
    /// Enter Form XObjects named by `Do`.
    pub(crate) enter_forms: bool,
}

// ---------------------------------------------------------------------------
// Interpreter
// ---------------------------------------------------------------------------

/// Interpret `ops` against `resources`, starting from `ctm`.
pub(crate) fn interpret(
    ops: &[ContentOp],
    resources: &mut dyn ContentResources,
    ctm: Matrix,
    options: InterpretOptions,
    visitor: &mut dyn ContentVisitor,
) {
    let mut interpreter = Interpreter {
        gs: GraphicsState::new(ctm),
        gs_stack: Vec::new(),
        stack_floor: 0,
        tm: Matrix::identity(),
        tlm: Matrix::identity(),
        options,
        active_forms: Vec::new(),
    };
    interpreter.run(ops, resources, visitor);
}

struct Interpreter {
    gs: GraphicsState,
    gs_stack: Vec<GraphicsState>,
    /// `Q` does not pop below this: the stack depth at which the current
    /// form was entered.
    stack_floor: usize,
    /// Text matrix (Tm).
    tm: Matrix,
    /// Text line matrix (Tlm).
    tlm: Matrix,
    options: InterpretOptions,
    /// Forms being interpreted, outermost first.
    active_forms: Vec<IndirectRef>,
}

impl Interpreter {
    fn run(
        &mut self,
        ops: &[ContentOp],
        resources: &mut dyn ContentResources,
        visitor: &mut dyn ContentVisitor,
    ) {
        for (index, op) in ops.iter().enumerate() {
            visitor.op(index, op, &self.gs);
            self.process_op(index, op, resources, visitor);
        }
    }

    fn process_op(
        &mut self,
        index: usize,
        op: &ContentOp,
        resources: &mut dyn ContentResources,
        visitor: &mut dyn ContentVisitor,
    ) {
        let num = |i: usize| op.operands.get(i).and_then(|o| o.as_f64());
        match op.operator.as_slice() {
            b"q" => self.gs_stack.push(self.gs.clone()),
            b"Q" => {
                if self.gs_stack.len() > self.stack_floor
                    && let Some(gs) = self.gs_stack.pop()
                {
                    self.gs = gs;
                }
            }
            b"cm" => {
                if let Some(m) = Matrix::from_operands(&op.operands) {
                    self.gs.ctm = m.concat(&self.gs.ctm);
                }
            }

            b"Tc" => {
                if let Some(v) = num(0) {
                    self.gs.text.char_spacing = v;
                }
            }
            b"Tw" => {
                if let Some(v) = num(0) {
                    self.gs.text.word_spacing = v;
                }
            }
            b"Tz" => {
                if let Some(v) = num(0) {
                    self.gs.text.horiz_scaling = v / 100.0;
                }
            }
            b"TL" => {
                if let Some(v) = num(0) {
                    self.gs.text.leading = v;
                }
            }
            b"Tf" => {
                if op.operands.len() >= 2 {
                    if let Some(name) = op.operands[0].as_name() {
                        self.gs.text.font_name = name.to_vec();
                        self.gs.text.font = resources.font(name);
                    }
                    if let Some(size) = num(1) {
                        self.gs.text.font_size = size;
                    }
                }
            }
            b"Ts" => {
                if let Some(v) = num(0) {
                    self.gs.text.text_rise = v;
                }
            }
            b"gs" => {
                if let Some(name) = op.operands.first().and_then(|o| o.as_name())
                    && let Some((font, size)) = resources.ext_gstate_font(name)
                {
                    self.gs.text.font_name = Vec::new();
                    self.gs.text.font = Some(font);
                    self.gs.text.font_size = size;
                }
            }
            b"Tr" => {
                if let Some(v) = op.operands.first().and_then(|o| o.as_i64()) {
                    self.gs.text.render_mode = v;
                }
            }

            b"BT" => {
                self.tm = Matrix::identity();
                self.tlm = Matrix::identity();
            }

            b"Td" => {
                if op.operands.len() >= 2 {
                    let tx = num(0).unwrap_or(0.0);
                    let ty = num(1).unwrap_or(0.0);
                    self.move_line(tx, ty);
                }
            }
            b"TD" => {
                if op.operands.len() >= 2 {
                    let tx = num(0).unwrap_or(0.0);
                    let ty = num(1).unwrap_or(0.0);
                    self.gs.text.leading = -ty;
                    self.move_line(tx, ty);
                }
            }
            b"Tm" => {
                if let Some(m) = Matrix::from_operands(&op.operands) {
                    self.tm = m;
                    self.tlm = m;
                }
            }
            b"T*" => self.next_line(),

            b"Tj" => {
                if let Some(s) = op.operands.first().and_then(|o| o.as_str()) {
                    self.show_string(s, index, None, visitor);
                }
            }
            b"TJ" => {
                if let Some(arr) = op.operands.first().and_then(|o| o.as_array()) {
                    self.show_tj_array(arr, index, visitor);
                }
            }
            b"'" => {
                self.next_line();
                if let Some(s) = op.operands.first().and_then(|o| o.as_str()) {
                    self.show_string(s, index, None, visitor);
                }
            }
            b"\"" => {
                if op.operands.len() >= 3 {
                    if let Some(aw) = num(0) {
                        self.gs.text.word_spacing = aw;
                    }
                    if let Some(ac) = num(1) {
                        self.gs.text.char_spacing = ac;
                    }
                    self.next_line();
                    if let Some(s) = op.operands[2].as_str() {
                        self.show_string(s, index, None, visitor);
                    }
                }
            }

            b"Do" if self.options.enter_forms => {
                if let Some(name) = op.operands.first().and_then(|o| o.as_name()) {
                    self.do_form(name, index, resources, visitor);
                }
            }

            _ => {}
        }
    }

    fn move_line(&mut self, tx: f64, ty: f64) {
        self.tlm = Matrix::translate(tx, ty).concat(&self.tlm);
        self.tm = self.tlm;
    }

    fn next_line(&mut self) {
        let tl = self.gs.text.leading;
        self.move_line(0.0, -tl);
    }

    fn do_form(
        &mut self,
        name: &[u8],
        op_index: usize,
        resources: &mut dyn ContentResources,
        visitor: &mut dyn ContentVisitor,
    ) {
        let Some(mut form) = resources.form(name) else {
            return;
        };
        if self.active_forms.contains(&form.id) {
            visitor.form_refused(op_index, FormRefusal::Cycle);
            return;
        }
        if self.active_forms.len() >= MAX_FORM_DEPTH {
            visitor.form_refused(op_index, FormRefusal::Depth);
            return;
        }

        let saved_gs = self.gs.clone();
        let saved_stack_len = self.gs_stack.len();
        let saved_floor = self.stack_floor;
        let (saved_tm, saved_tlm) = (self.tm, self.tlm);

        self.gs.ctm = form.matrix.concat(&self.gs.ctm);
        self.stack_floor = saved_stack_len;
        self.active_forms.push(form.id.clone());
        visitor.enter_form(op_index, &form.id);

        self.run(&form.ops, form.resources.as_mut(), visitor);

        visitor.leave_form();
        self.active_forms.pop();
        self.gs_stack.truncate(saved_stack_len);
        self.stack_floor = saved_floor;
        self.gs = saved_gs;
        self.tm = saved_tm;
        self.tlm = saved_tlm;
    }

    fn show_string(
        &mut self,
        raw: &[u8],
        op_index: usize,
        tj_index: Option<usize>,
        visitor: &mut dyn ContentVisitor,
    ) {
        let font = self.gs.text.font.clone();
        let font = font.as_deref();
        let is_two_byte = font.map(ResolvedFont::is_two_byte).unwrap_or(false);
        let (bottom, top) = font
            .map(ResolvedFont::glyph_height)
            .unwrap_or(DEFAULT_GLYPH_HEIGHT);

        let tfs = self.gs.text.font_size;
        let tc = self.gs.text.char_spacing;
        let tw = self.gs.text.word_spacing;
        let th = self.gs.text.horiz_scaling;
        let rise = self.gs.text.text_rise;

        let mut i = 0;
        while i < raw.len() {
            let (code, byte_len) = if is_two_byte && i + 1 < raw.len() {
                (((raw[i] as u32) << 8) | raw[i + 1] as u32, 2)
            } else {
                (raw[i] as u32, 1)
            };

            let w0 = font.map(|f| f.char_width(code)).unwrap_or(500.0);
            let trm = self.text_rendering_matrix(tfs, th, rise);
            let (w, lo, hi) = (w0 / 1000.0, bottom / 1000.0, top / 1000.0);
            let quad = [
                trm.transform_point(0.0, lo),
                trm.transform_point(w, lo),
                trm.transform_point(w, hi),
                trm.transform_point(0.0, hi),
            ];

            visitor.glyph(
                &Glyph {
                    op_index,
                    tj_index,
                    byte_range: i..i + byte_len,
                    bytes: &raw[i..i + byte_len],
                    code,
                    font,
                    width: w0,
                    trm,
                    quad,
                },
                &self.gs,
            );

            // Glyph displacement (§9.4.4): word spacing applies to code 32.
            let advance = (w0 / 1000.0 * tfs + tc) * th;
            let total_advance = if code == 32 {
                advance + tw * th
            } else {
                advance
            };
            self.tm = Matrix::translate(total_advance, 0.0).concat(&self.tm);

            i += byte_len;
        }
    }

    fn show_tj_array(
        &mut self,
        items: &[Operand],
        op_index: usize,
        visitor: &mut dyn ContentVisitor,
    ) {
        let th = self.gs.text.horiz_scaling;
        let tfs = self.gs.text.font_size;

        for (tj_index, item) in items.iter().enumerate() {
            let adjustment = match item {
                Operand::String(s) => {
                    self.show_string(s, op_index, Some(tj_index), visitor);
                    continue;
                }
                Operand::Integer(n) => *n as f64,
                Operand::Real(n) => *n,
                _ => continue,
            };
            let displacement = -adjustment / 1000.0 * tfs * th;
            self.tm = Matrix::translate(displacement, 0.0).concat(&self.tm);
        }
    }

    /// Trm = [Tfs×Th 0 0 Tfs 0 Ts] × Tm × CTM (§9.4.4).
    fn text_rendering_matrix(&self, tfs: f64, th: f64, rise: f64) -> Matrix {
        let text_state = Matrix {
            a: tfs * th,
            b: 0.0,
            c: 0.0,
            d: tfs,
            e: 0.0,
            f: rise,
        };
        text_state.concat(&self.tm).concat(&self.gs.ctm)
    }
}

// ---------------------------------------------------------------------------
// Test support
// ---------------------------------------------------------------------------

/// Resources from in-memory maps.
#[cfg(test)]
#[derive(Default)]
pub(crate) struct MapResources {
    pub(crate) fonts: HashMap<Vec<u8>, Rc<ResolvedFont>>,
    /// Forms by name: object, operations, `/Matrix`, own fonts (`None` to
    /// inherit the caller's).
    pub(crate) forms: HashMap<Vec<u8>, MapForm>,
}

#[cfg(test)]
#[derive(Clone)]
pub(crate) struct MapForm {
    pub(crate) id: IndirectRef,
    pub(crate) ops: Vec<ContentOp>,
    pub(crate) matrix: Matrix,
    pub(crate) fonts: Option<HashMap<Vec<u8>, Rc<ResolvedFont>>>,
}

#[cfg(test)]
impl ContentResources for MapResources {
    fn font(&mut self, name: &[u8]) -> Option<Rc<ResolvedFont>> {
        self.fonts.get(name).cloned()
    }

    fn ext_gstate_font(&mut self, _name: &[u8]) -> Option<(Rc<ResolvedFont>, f64)> {
        None
    }

    fn form(&mut self, name: &[u8]) -> Option<FormXObject<'_>> {
        let form = self.forms.get(name)?.clone();
        let mut fonts = self.fonts.clone();
        fonts.extend(form.fonts.unwrap_or_default());
        Some(FormXObject {
            id: form.id,
            ops: form.ops,
            matrix: form.matrix,
            resources: Box::new(MapResources {
                fonts,
                forms: self.forms.clone(),
            }),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font::{FontWidths, parse_font_descriptor};

    fn op(operator: &str, operands: Vec<Operand>) -> ContentOp {
        ContentOp {
            operator: operator.as_bytes().to_vec(),
            operands,
        }
    }

    fn int(n: i64) -> Operand {
        Operand::Integer(n)
    }

    fn real(n: f64) -> Operand {
        Operand::Real(n)
    }

    fn name(s: &str) -> Operand {
        Operand::Name(s.as_bytes().to_vec())
    }

    fn string(s: &str) -> Operand {
        Operand::String(s.as_bytes().to_vec())
    }

    fn font(default_width: f64, bbox: Option<[f64; 4]>) -> Rc<ResolvedFont> {
        Rc::new(ResolvedFont {
            info: FontInfo {
                base_font: b"Test".to_vec(),
                subtype: b"Type1".to_vec(),
                encoding: Encoding::WinAnsiEncoding,
                differences: Vec::new(),
                widths: FontWidths::None { default_width },
                to_unicode: None,
                is_standard14: false,
                descriptor: bbox.and_then(|b| {
                    let mut d = PdfDict::new();
                    d.insert(b"FontName".to_vec(), PdfObject::Name(b"Test".to_vec()));
                    let b = b.iter().map(|&v| PdfObject::Real(v)).collect();
                    d.insert(b"FontBBox".to_vec(), PdfObject::Array(b));
                    parse_font_descriptor(&d)
                }),
            },
            cmap: None,
        })
    }

    fn resources(fonts: &[(&str, Rc<ResolvedFont>)]) -> MapResources {
        MapResources {
            fonts: fonts
                .iter()
                .map(|(n, f)| (n.as_bytes().to_vec(), Rc::clone(f)))
                .collect(),
            forms: HashMap::new(),
        }
    }

    #[derive(Debug, Clone)]
    struct Seen {
        op_index: usize,
        tj_index: Option<usize>,
        byte_range: Range<usize>,
        code: u32,
        quad: [(f64, f64); 4],
        trm_origin: (f64, f64),
        font_size: f64,
        ctm: Matrix,
        render_mode: i64,
        form_depth: usize,
    }

    #[derive(Default)]
    struct Recorder {
        glyphs: Vec<Seen>,
        ops: Vec<(usize, Vec<u8>, Matrix)>,
        depth: usize,
        entered: Vec<(usize, IndirectRef)>,
        refused: Vec<(usize, FormRefusal)>,
    }

    impl ContentVisitor for Recorder {
        fn op(&mut self, index: usize, op: &ContentOp, state: &GraphicsState) {
            self.ops.push((index, op.operator.clone(), state.ctm));
        }
        fn glyph(&mut self, g: &Glyph<'_>, state: &GraphicsState) {
            self.glyphs.push(Seen {
                op_index: g.op_index,
                tj_index: g.tj_index,
                byte_range: g.byte_range.clone(),
                code: g.code,
                quad: g.quad,
                trm_origin: g.trm.transform_point(0.0, 0.0),
                font_size: state.text.font_size,
                ctm: state.ctm,
                render_mode: state.text.render_mode,
                form_depth: self.depth,
            });
        }
        fn enter_form(&mut self, op_index: usize, id: &IndirectRef) {
            self.depth += 1;
            self.entered.push((op_index, id.clone()));
        }
        fn leave_form(&mut self) {
            self.depth -= 1;
        }
        fn form_refused(&mut self, op_index: usize, reason: FormRefusal) {
            self.refused.push((op_index, reason));
        }
    }

    fn run(ops: &[ContentOp], res: &mut MapResources, options: InterpretOptions) -> Recorder {
        let mut rec = Recorder::default();
        interpret(ops, res, Matrix::identity(), options, &mut rec);
        rec
    }

    fn assert_point(actual: (f64, f64), expected: (f64, f64)) {
        assert!(
            (actual.0 - expected.0).abs() < 1e-9 && (actual.1 - expected.1).abs() < 1e-9,
            "{actual:?} != {expected:?}"
        );
    }

    fn assert_quad(actual: [(f64, f64); 4], expected: [(f64, f64); 4]) {
        for (a, e) in actual.into_iter().zip(expected) {
            assert_point(a, e);
        }
    }

    #[test]
    fn tj_with_kerning_under_cm_and_tm_gives_hand_computed_boxes() {
        // Widths 500 and /FontBBox [0 -200 1000 800].
        // 2 0 0 2 10 20 cm  BT /F1 10 Tf 1 0 0 1 5 7 Tm [(AB) -250 (C)] TJ ET
        let f = font(500.0, Some([0.0, -200.0, 1000.0, 800.0]));
        let mut res = resources(&[("F1", f)]);
        let ops = vec![
            op("cm", vec![int(2), int(0), int(0), int(2), int(10), int(20)]),
            op("BT", vec![]),
            op("Tf", vec![name("F1"), int(10)]),
            op("Tm", vec![int(1), int(0), int(0), int(1), int(5), int(7)]),
            op(
                "TJ",
                vec![Operand::Array(vec![string("AB"), int(-250), string("C")])],
            ),
            op("ET", vec![]),
        ];
        let rec = run(&ops, &mut res, InterpretOptions::default());

        // Text space: A at x=5, B at 5+5=10, kerning +2.5, C at 17.5;
        // each glyph 5 wide, y from 7-2=5 to 7+8=15. Device = 2·text + (10, 20).
        let quad = |x0: f64| {
            [
                (2.0 * x0 + 10.0, 30.0),
                (2.0 * (x0 + 5.0) + 10.0, 30.0),
                (2.0 * (x0 + 5.0) + 10.0, 50.0),
                (2.0 * x0 + 10.0, 50.0),
            ]
        };
        assert_eq!(rec.glyphs.len(), 3);
        assert_quad(rec.glyphs[0].quad, quad(5.0));
        assert_quad(rec.glyphs[1].quad, quad(10.0));
        assert_quad(rec.glyphs[2].quad, quad(17.5));
        assert_point(rec.glyphs[2].trm_origin, (45.0, 34.0));
    }

    #[test]
    fn glyph_box_without_font_bbox_spans_minus_quarter_to_one_em() {
        // A zero-height /FontBBox is treated as absent.
        for bbox in [None, Some([0.0, 0.0, 0.0, 0.0])] {
            let mut res = resources(&[("F1", font(600.0, bbox))]);
            let ops = vec![
                op("BT", vec![]),
                op("Tf", vec![name("F1"), int(20)]),
                op("Tj", vec![string("A")]),
            ];
            let rec = run(&ops, &mut res, InterpretOptions::default());
            assert_quad(
                rec.glyphs[0].quad,
                [(0.0, -5.0), (12.0, -5.0), (12.0, 20.0), (0.0, 20.0)],
            );
        }
    }

    #[test]
    fn a_font_bbox_given_by_its_other_corners_spans_the_same_height() {
        let mut res = resources(&[("F1", font(500.0, Some([1000.0, 800.0, 0.0, -200.0])))]);
        let ops = vec![
            op("BT", vec![]),
            op("Tf", vec![name("F1"), int(10)]),
            op("Tj", vec![string("A")]),
        ];
        let rec = run(&ops, &mut res, InterpretOptions::default());
        assert_quad(
            rec.glyphs[0].quad,
            [(0.0, -2.0), (5.0, -2.0), (5.0, 8.0), (0.0, 8.0)],
        );
    }

    #[test]
    fn glyph_box_follows_horizontal_scaling_and_rise_but_not_spacing() {
        // 50 Tz, 3 Ts, 4 Tc: box = 0.6·10·0.5 wide, lifted 3; next glyph
        // starts after (6 + 4)·0.5 = 5.
        let mut res = resources(&[("F1", font(600.0, Some([0.0, 0.0, 600.0, 1000.0])))]);
        let ops = vec![
            op("BT", vec![]),
            op("Tf", vec![name("F1"), int(10)]),
            op("Tz", vec![int(50)]),
            op("Ts", vec![int(3)]),
            op("Tc", vec![int(4)]),
            op("Tj", vec![string("AB")]),
        ];
        let rec = run(&ops, &mut res, InterpretOptions::default());
        assert_quad(
            rec.glyphs[0].quad,
            [(0.0, 3.0), (3.0, 3.0), (3.0, 13.0), (0.0, 13.0)],
        );
        assert_point(rec.glyphs[1].quad[0], (5.0, 3.0));
    }

    #[test]
    fn q_and_q_restore_ctm_and_text_state_across_text_blocks() {
        let mut res = resources(&[("F1", font(500.0, None)), ("F2", font(1000.0, None))]);
        let ops = vec![
            op("BT", vec![]),
            op("Tf", vec![name("F1"), int(10)]),
            op("Tc", vec![int(1)]),
            op("ET", vec![]),
            op("q", vec![]),
            op("cm", vec![int(3), int(0), int(0), int(3), int(0), int(0)]),
            op("BT", vec![]),
            op("Tf", vec![name("F2"), int(20)]),
            op("Tc", vec![int(9)]),
            op("Tr", vec![int(3)]),
            op("Tj", vec![string("A")]),
            op("ET", vec![]),
            op("Q", vec![]),
            op("BT", vec![]),
            op("Tj", vec![string("AB")]),
            op("ET", vec![]),
        ];
        let rec = run(&ops, &mut res, InterpretOptions::default());
        assert_eq!(rec.glyphs.len(), 3);
        assert_eq!(rec.glyphs[0].font_size, 20.0);
        assert_eq!(rec.glyphs[0].ctm.a, 3.0);
        assert_eq!(rec.glyphs[0].render_mode, 3);
        // After Q: F1 at 10, Tc 1, identity CTM, Tr 0.
        assert_eq!(rec.glyphs[1].font_size, 10.0);
        assert_eq!(rec.glyphs[1].ctm.a, 1.0);
        assert_eq!(rec.glyphs[1].render_mode, 0);
        assert_point(rec.glyphs[2].trm_origin, (6.0, 0.0)); // 0.5·10 + 1
    }

    #[test]
    fn t_star_and_quote_move_down_by_the_current_leading() {
        let mut res = resources(&[("F1", font(500.0, None))]);
        let ops = vec![
            op("BT", vec![]),
            op("Tf", vec![name("F1"), int(10)]),
            op("Td", vec![int(100), int(500)]),
            op("TL", vec![int(17)]),
            op("T*", vec![]),
            op("Tj", vec![string("A")]),
            op("TL", vec![real(4.5)]),
            op("'", vec![string("B")]),
            op("TD", vec![int(0), int(-30)]),
            op("T*", vec![]),
            op("Tj", vec![string("C")]),
        ];
        let rec = run(&ops, &mut res, InterpretOptions::default());
        assert_point(rec.glyphs[0].trm_origin, (100.0, 483.0));
        assert_point(rec.glyphs[1].trm_origin, (100.0, 478.5));
        // TD sets TL to 30: down 30, then T* down 30 more.
        assert_point(rec.glyphs[2].trm_origin, (100.0, 418.5));
    }

    #[test]
    fn each_glyph_traces_to_its_operation_and_tj_element() {
        let mut res = resources(&[("F1", font(500.0, None))]);
        let ops = vec![
            op("BT", vec![]),
            op("Tf", vec![name("F1"), int(10)]),
            op("Tj", vec![string("ab")]),
            op(
                "TJ",
                vec![Operand::Array(vec![
                    string("c"),
                    int(100),
                    string("de"),
                    real(-5.0),
                ])],
            ),
            op("\"", vec![int(1), int(2), string("f")]),
        ];
        let rec = run(&ops, &mut res, InterpretOptions::default());
        let trace: Vec<_> = rec
            .glyphs
            .iter()
            .map(|g| {
                let c = char::from_u32(g.code).unwrap();
                (g.op_index, g.tj_index, g.byte_range.clone(), c)
            })
            .collect();
        assert_eq!(
            trace,
            vec![
                (2, None, 0..1, 'a'),
                (2, None, 1..2, 'b'),
                (3, Some(0), 0..1, 'c'),
                (3, Some(2), 0..1, 'd'),
                (3, Some(2), 1..2, 'e'),
                (4, None, 0..1, 'f'),
            ]
        );
    }

    #[test]
    fn op_reports_the_state_before_the_operation_runs() {
        let mut res = MapResources::default();
        let ops = vec![
            op("cm", vec![int(2), int(0), int(0), int(2), int(0), int(0)]),
            op("n", vec![]),
        ];
        let rec = run(&ops, &mut res, InterpretOptions::default());
        assert_eq!(rec.ops[0].2.a, 1.0);
        assert_eq!(rec.ops[1].2.a, 2.0);
    }

    fn form_resources(
        form_ops: Vec<ContentOp>,
        matrix: Matrix,
        own_fonts: Option<&[(&str, Rc<ResolvedFont>)]>,
    ) -> MapResources {
        let mut res = resources(&[("F1", font(500.0, None))]);
        res.forms.insert(
            b"Fm".to_vec(),
            MapForm {
                id: IndirectRef {
                    obj_num: 7,
                    gen_num: 0,
                },
                ops: form_ops,
                matrix,
                fonts: own_fonts.map(|fs| {
                    fs.iter()
                        .map(|(n, f)| (n.as_bytes().to_vec(), Rc::clone(f)))
                        .collect()
                }),
            },
        );
        res
    }

    #[test]
    fn forms_are_not_entered_by_default() {
        let mut res = form_resources(
            vec![op("BT", vec![]), op("Tj", vec![string("x")])],
            Matrix::identity(),
            None,
        );
        let rec = run(
            &[op("Do", vec![name("Fm")])],
            &mut res,
            InterpretOptions::default(),
        );
        assert!(rec.glyphs.is_empty());
        assert!(rec.entered.is_empty());
    }

    #[test]
    fn entered_form_applies_its_matrix_and_resolves_names_in_its_own_resources() {
        // The form's /F1 is 1000 wide; the page's is 500.
        let mut res = form_resources(
            vec![
                op("BT", vec![]),
                op("Tf", vec![name("F1"), int(10)]),
                op("Tj", vec![string("xy")]),
                op("ET", vec![]),
            ],
            Matrix {
                a: 1.0,
                b: 0.0,
                c: 0.0,
                d: 1.0,
                e: 100.0,
                f: 200.0,
            },
            Some(&[("F1", font(1000.0, None))]),
        );
        let ops = vec![
            op("cm", vec![int(2), int(0), int(0), int(2), int(0), int(0)]),
            op("Do", vec![name("Fm")]),
            op("BT", vec![]),
            op("Tf", vec![name("F1"), int(10)]),
            op("Tj", vec![string("z")]),
        ];
        let options = InterpretOptions { enter_forms: true };
        let rec = run(&ops, &mut res, options);

        assert_eq!(
            rec.entered,
            vec![(
                1,
                IndirectRef {
                    obj_num: 7,
                    gen_num: 0
                }
            )]
        );
        assert_eq!(rec.glyphs.len(), 3);
        // Form space → ×2 after translating by (100, 200).
        assert_point(rec.glyphs[0].trm_origin, (200.0, 400.0));
        assert_point(rec.glyphs[1].trm_origin, (220.0, 400.0)); // 1000 wide at 10
        assert_eq!((rec.glyphs[0].op_index, rec.glyphs[0].form_depth), (2, 1));
        // Back on the page: the page's F1, no form matrix.
        assert_eq!(rec.glyphs[2].form_depth, 0);
        assert_point(rec.glyphs[2].quad[1], (10.0, -5.0));
    }

    #[test]
    fn form_inherits_the_callers_text_state_and_cannot_pop_it() {
        // The form shows text with no Tf, and its stray Q must not restore
        // the page's q.
        let mut res = form_resources(
            vec![
                op("Q", vec![]),
                op("BT", vec![]),
                op("Tj", vec![string("x")]),
                op("ET", vec![]),
            ],
            Matrix::identity(),
            None,
        );
        let ops = vec![
            op("BT", vec![]),
            op("Tf", vec![name("F1"), int(30)]),
            op("ET", vec![]),
            op("q", vec![]),
            op("cm", vec![int(2), int(0), int(0), int(2), int(0), int(0)]),
            op("Do", vec![name("Fm")]),
            op("BT", vec![]),
            op("Tj", vec![string("y")]),
        ];
        let rec = run(&ops, &mut res, InterpretOptions { enter_forms: true });
        assert_eq!(rec.glyphs.len(), 2);
        assert_eq!(rec.glyphs[0].font_size, 30.0);
        assert_eq!(rec.glyphs[0].ctm.a, 2.0);
        assert_eq!(rec.glyphs[1].ctm.a, 2.0);
    }

    #[test]
    fn a_form_that_draws_itself_is_refused_as_a_cycle() {
        let mut res = form_resources(vec![op("Do", vec![name("Fm")])], Matrix::identity(), None);
        let rec = run(
            &[op("Do", vec![name("Fm")])],
            &mut res,
            InterpretOptions { enter_forms: true },
        );
        assert_eq!(rec.entered.len(), 1);
        assert_eq!(rec.refused, vec![(0, FormRefusal::Cycle)]);
    }

    #[test]
    fn forms_nested_past_the_depth_limit_are_refused() {
        // F0 draws F1 draws … each a distinct object.
        let mut res = MapResources::default();
        let total = MAX_FORM_DEPTH + 2;
        for i in 0..total {
            res.forms.insert(
                format!("F{i}").into_bytes(),
                MapForm {
                    id: IndirectRef {
                        obj_num: 100 + i as u32,
                        gen_num: 0,
                    },
                    ops: vec![op("Do", vec![name(&format!("F{}", i + 1))])],
                    matrix: Matrix::identity(),
                    fonts: None,
                },
            );
        }
        let rec = run(
            &[op("Do", vec![name("F0")])],
            &mut res,
            InterpretOptions { enter_forms: true },
        );
        assert_eq!(rec.entered.len(), MAX_FORM_DEPTH);
        assert_eq!(rec.refused, vec![(0, FormRefusal::Depth)]);
    }

    /// A one-page document: `page_resources` is the page's `/Resources`,
    /// `content` its content stream, and `objects` are numbered from 4.
    fn document(page_resources: &str, content: &str, objects: &[&str]) -> PdfDocument {
        // `dict` is a whole dictionary, or empty.
        let stream = |body: &str, dict: &str| {
            let dict = dict.trim();
            let entries = dict
                .strip_prefix("<<")
                .and_then(|d| d.strip_suffix(">>"))
                .unwrap_or(dict);
            format!(
                "<< {entries} /Length {} >>\nstream\n{body}\nendstream",
                body.len()
            )
        };
        let mut bodies = vec![
            "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 600 600] /Resources {page_resources} /Contents {} 0 R >>",
                objects.len() + 4
            ),
        ];
        bodies.extend(objects.iter().map(|o| match o.split_once("\nSTREAM ") {
            Some((dict, body)) => stream(body, dict),
            None => o.to_string(),
        }));
        bodies.push(stream(content, ""));

        let mut out = b"%PDF-1.7\n".to_vec();
        let mut offsets = Vec::new();
        for (i, body) in bodies.iter().enumerate() {
            offsets.push(out.len());
            out.extend_from_slice(format!("{} 0 obj\n{body}\nendobj\n", i + 1).as_bytes());
        }
        let xref_at = out.len();
        out.extend_from_slice(
            format!("xref\n0 {}\n0000000000 65535 f \n", bodies.len() + 1).as_bytes(),
        );
        for off in offsets {
            out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
        }
        out.extend_from_slice(
            format!(
                "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref_at}\n%%EOF\n",
                bodies.len() + 1
            )
            .as_bytes(),
        );
        PdfDocument::from_bytes(out).unwrap()
    }

    /// Runs the page's content stream against its resources.
    fn run_document(doc: &PdfDocument, options: InterpretOptions) -> Recorder {
        let page = crate::page::get_page(doc, 0).unwrap();
        let content = match &page.contents_ref {
            Some(PdfObject::Reference(r)) => match doc.resolve(r).unwrap() {
                PdfObject::Stream { dict, data } => doc.decode_stream(&dict, &data).unwrap(),
                _ => unreachable!(),
            },
            _ => unreachable!(),
        };
        let ops = parse_content_stream(&content).unwrap();
        let mut res = DocResources::page(doc, page.resources_ref.as_ref());
        let mut rec = Recorder::default();
        interpret(&ops, &mut res, Matrix::identity(), options, &mut rec);
        rec
    }

    const DESCRIPTOR: &str =
        "<< /Type /FontDescriptor /FontName /Test /Flags 32 /FontBBox [0 -200 1000 800] >>";

    #[test]
    fn an_indirect_font_descriptor_gives_the_glyph_box_its_height() {
        let doc = document(
            "<< /Font << /F1 5 0 R >> >>",
            "BT /F1 10 Tf (A) Tj ET",
            &[
                DESCRIPTOR,
                "<< /Type /Font /Subtype /Type1 /BaseFont /Test /FirstChar 65 /LastChar 65 /Widths [500] /FontDescriptor 4 0 R >>",
            ],
        );
        let rec = run_document(&doc, InterpretOptions::default());
        assert_quad(
            rec.glyphs[0].quad,
            [(0.0, -2.0), (5.0, -2.0), (5.0, 8.0), (0.0, 8.0)],
        );
    }

    #[test]
    fn a_type0_font_takes_the_descendants_descriptor_and_widths() {
        let doc = document(
            "<< /Font << /F1 6 0 R >> >>",
            "BT /F1 10 Tf <0041> Tj ET",
            &[
                DESCRIPTOR,
                "<< /Type /Font /Subtype /CIDFontType2 /BaseFont /Test /CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> /DW 700 /FontDescriptor 4 0 R >>",
                "<< /Type /Font /Subtype /Type0 /BaseFont /Test /Encoding /Identity-H /DescendantFonts [5 0 R] >>",
            ],
        );
        let rec = run_document(&doc, InterpretOptions::default());
        assert_eq!(rec.glyphs.len(), 1);
        assert_eq!(rec.glyphs[0].code, 0x41);
        assert_quad(
            rec.glyphs[0].quad,
            [(0.0, -2.0), (7.0, -2.0), (7.0, 8.0), (0.0, 8.0)],
        );
    }

    #[test]
    fn document_forms_resolve_names_in_their_own_resources_then_the_callers() {
        // The page's /F1 is 1000 wide. /Fm names its own /F1, 500 wide, and
        // draws /Fn, which only the page names; /Fn has no /Resources, so it
        // runs in /Fm's scope. /Im is an image and is not entered.
        let doc = document(
            "<< /Font << /F1 4 0 R >> /XObject << /Fm 6 0 R /Fn 7 0 R /Im 8 0 R >> >>",
            "BT /F1 10 Tf (A) Tj ET /Fm Do /Im Do",
            &[
                "<< /Type /Font /Subtype /Type1 /BaseFont /Wide /FirstChar 65 /LastChar 65 /Widths [1000] >>",
                "<< /Type /Font /Subtype /Type1 /BaseFont /Narrow /FirstChar 65 /LastChar 65 /Widths [500] >>",
                "<< /Type /XObject /Subtype /Form /BBox [0 0 600 600] /Matrix [1 0 0 1 100 0] /Resources << /Font << /F1 5 0 R >> >> >>\nSTREAM BT /F1 10 Tf (A) Tj ET /Fn Do",
                "<< /Type /XObject /Subtype /Form /BBox [0 0 600 600] /Matrix [1 0 0 1 0 50] >>\nSTREAM BT /F1 10 Tf (A) Tj ET",
                "<< /Type /XObject /Subtype /Image /Width 1 /Height 1 /ColorSpace /DeviceGray /BitsPerComponent 8 >>\nSTREAM x",
            ],
        );
        let rec = run_document(&doc, InterpretOptions { enter_forms: true });

        let ids: Vec<u32> = rec.entered.iter().map(|(_, id)| id.obj_num).collect();
        assert_eq!(ids, vec![6, 7]);
        let widths: Vec<f64> = rec
            .glyphs
            .iter()
            .map(|g| g.quad[1].0 - g.quad[0].0)
            .collect();
        assert_eq!(widths, vec![10.0, 5.0, 5.0]);
        assert_point(rec.glyphs[1].quad[0], (100.0, -2.5));
        assert_point(rec.glyphs[2].quad[0], (100.0, 47.5));
    }
}
