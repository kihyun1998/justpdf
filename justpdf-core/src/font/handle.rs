//! Per-document font handles ([`Font`]) and the Type0 font each embedded one
//! is written as.

use std::collections::{BTreeSet, HashMap};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use super::FontDescriptor;
use super::cjk::{CJKOrdering, build_w_array, generate_to_unicode_cmap};
use super::program::FontProgram;
use crate::object::{IndirectRef, PdfDict, PdfObject};
use crate::writer::PdfWriter;
use crate::writer::encode::make_stream;

/// How an embedded font's program goes into the document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FontEmbedding {
    /// Subset at build to the glyphs drawn plus `.notdef`, named with a
    /// six-letter subset tag.
    #[default]
    Subset,
    /// The whole program, with no subset tag, for a document whose text will be
    /// added later.
    Whole,
}

/// A font registered on one document, with the fallback fonts tried after it.
///
/// Made by `add_font` or `add_standard_font` on
/// [`DocumentBuilder`](crate::writer::DocumentBuilder) or
/// [`DocumentModifier`](crate::writer::DocumentModifier), selected with
/// [`PageBuilder::set_font`](crate::writer::PageBuilder::set_font). Clones
/// share the document's record of the characters drawn, which `show_text`
/// adds to whether or not its page is then added to the document.
///
/// Characters map one-to-one to glyphs through the font's `cmap`: there is no
/// shaping, no bidi and no vertical writing (text is written Identity-H), so
/// Arabic, Hebrew, Indic scripts and conjoining Hangul jamo do not draw
/// correctly.
#[derive(Debug, Clone)]
pub struct Font {
    /// The primary font, then its fallbacks in order.
    chain: Vec<Arc<FontEntry>>,
}

/// One registered font.
#[derive(Debug)]
pub(crate) struct FontEntry {
    document: u64,
    resource_name: String,
    font_ref: IndirectRef,
    kind: FontKind,
    usage: Mutex<Usage>,
    /// Embedded fonts: the glyph of each character looked up so far.
    glyphs: Mutex<HashMap<char, Glyph>>,
}

/// An embedded font's glyph for one character.
#[derive(Debug, Clone, Copy)]
struct Glyph {
    /// `None` when the font has no glyph for the character.
    gid: Option<u16>,
    /// Advance in thousandths of an em: the glyph's, or `.notdef`'s.
    advance: f64,
}

#[derive(Debug)]
enum FontKind {
    /// A standard Type1 font: declared `/WinAnsiEncoding`, or Symbol and
    /// ZapfDingbats in their built-in encodings.
    Standard {
        base_font: String,
        encoding: StandardEncoding,
    },
    /// A TrueType program written as a Type0 font with an Identity-H encoding.
    Embedded {
        program: FontProgram,
        embedding: FontEmbedding,
        objects: EmbeddedObjects,
    },
}

/// The encoding a standard font's text is written in.
#[derive(Debug, Clone, Copy)]
enum StandardEncoding {
    WinAnsi,
    Symbol,
    ZapfDingbats,
}

impl StandardEncoding {
    fn of(base_font: &str) -> StandardEncoding {
        match base_font {
            "Symbol" => StandardEncoding::Symbol,
            "ZapfDingbats" => StandardEncoding::ZapfDingbats,
            _ => StandardEncoding::WinAnsi,
        }
    }

    /// `ch`'s code, `None` when the encoding has none.
    fn code(self, ch: char) -> Option<u8> {
        match self {
            StandardEncoding::WinAnsi => super::winansi_code(ch),
            StandardEncoding::Symbol => super::symbol_code(ch),
            StandardEncoding::ZapfDingbats => super::zapf_dingbats_code(ch),
        }
    }

    /// The code drawn for a character the encoding has none for: `?`, or a
    /// space in ZapfDingbats, which has no `?`.
    fn code_or_fallback(self, ch: char) -> u8 {
        self.code(ch).unwrap_or(match self {
            StandardEncoding::ZapfDingbats => b' ',
            _ => b'?',
        })
    }
}

/// Object numbers of the objects an embedded font is written as, besides the
/// Type0 font dictionary.
#[derive(Debug, Clone, Copy)]
struct EmbeddedObjects {
    cid_font: u32,
    descriptor: u32,
    font_file: u32,
    to_unicode: u32,
    cid_to_gid: u32,
}

/// The characters a document drew with one font.
#[derive(Debug, Default)]
struct Usage {
    /// Embedded fonts: the character of CID `n` at index `n - 1`.
    chars: Vec<char>,
    cids: HashMap<char, u16>,
    /// Characters drawn with this font that it has no glyph for.
    missing: BTreeSet<char>,
    /// Whether characters were added since the font was last written.
    written: bool,
}

impl Font {
    /// This font with `fallback`, and `fallback`'s own fallbacks, tried after
    /// the fonts it already has.
    ///
    /// # Panics
    ///
    /// When `fallback` is registered on a different document.
    pub fn with_fallback(&self, fallback: &Font) -> Font {
        assert_eq!(
            self.chain[0].document, fallback.chain[0].document,
            "a fallback font must be registered on the same document"
        );
        let mut chain = self.chain.clone();
        for entry in &fallback.chain {
            if !chain.iter().any(|e| Arc::ptr_eq(e, entry)) {
                chain.push(entry.clone());
            }
        }
        Font { chain }
    }

    /// The resource name the primary font is declared under in a page's
    /// `/Resources /Font` (e.g. `"F1"`).
    pub fn resource_name(&self) -> &str {
        &self.chain[0].resource_name
    }

    /// The width of `text` drawn at `size` points, in points: each character
    /// measured in the first font of the chain that has a glyph for it, or as
    /// the primary font's `.notdef` (or `?`) when none has.
    pub fn advance(&self, text: &str, size: f64) -> f64 {
        let thousandths: f64 = text
            .chars()
            .map(|ch| match self.font_for(ch) {
                Some(entry) => entry.advance(ch),
                None => self.chain[0].advance(ch),
            })
            .sum();
        thousandths * size / 1000.0
    }

    /// The fonts of the chain, the primary first.
    pub(crate) fn entries(&self) -> &[Arc<FontEntry>] {
        &self.chain
    }

    /// `text` split into runs, each with the font it is drawn in and its
    /// codes in that font's encoding. Records the characters drawn.
    pub(crate) fn encode(&self, text: &str) -> Vec<(Arc<FontEntry>, Vec<u8>)> {
        let mut runs: Vec<(Arc<FontEntry>, Vec<u8>)> = Vec::new();
        for ch in text.chars() {
            let entry = self.font_for(ch).unwrap_or(&self.chain[0]);
            let mut buf = Vec::with_capacity(2);
            entry.encode_char(ch, &mut buf);
            match runs.last_mut() {
                Some((last, codes)) if Arc::ptr_eq(last, entry) => codes.extend_from_slice(&buf),
                _ => runs.push((entry.clone(), buf)),
            }
        }
        runs
    }

    fn font_for(&self, ch: char) -> Option<&Arc<FontEntry>> {
        self.chain.iter().find(|entry| entry.has_glyph(ch))
    }
}

impl FontEntry {
    pub(crate) fn resource_name(&self) -> &str {
        &self.resource_name
    }

    pub(crate) fn font_ref(&self) -> &IndirectRef {
        &self.font_ref
    }

    fn has_glyph(&self, ch: char) -> bool {
        match &self.kind {
            FontKind::Standard { encoding, .. } => encoding.code(ch).is_some(),
            FontKind::Embedded { program, .. } => self.glyph(program, ch).gid.is_some(),
        }
    }

    /// `ch`'s glyph in `program`, this entry's embedded program.
    fn glyph(&self, program: &FontProgram, ch: char) -> Glyph {
        let mut glyphs = self.glyphs.lock().unwrap_or_else(|e| e.into_inner());
        *glyphs.entry(ch).or_insert_with(|| {
            let gid = program.glyph_id(ch);
            Glyph {
                gid,
                advance: program.glyph_advance(gid.unwrap_or(0)),
            }
        })
    }

    /// Advance of `ch` in thousandths of an em: its glyph's, or `.notdef`'s
    /// (the fallback code's for a standard font) when the font has none.
    fn advance(&self, ch: char) -> f64 {
        match &self.kind {
            FontKind::Standard {
                base_font,
                encoding,
            } => {
                let code = encoding.code_or_fallback(ch);
                super::standard14_widths(base_font.as_bytes())[code as usize]
            }
            FontKind::Embedded { program, .. } => self.glyph(program, ch).advance,
        }
    }

    /// Append `ch`'s code to `out`: a byte in its encoding (`?`, or a space in
    /// ZapfDingbats, when it has none) for a standard font, a 2-byte CID for an
    /// embedded one.
    fn encode_char(&self, ch: char, out: &mut Vec<u8>) {
        let mut usage = self.usage.lock().unwrap_or_else(|e| e.into_inner());
        if !self.has_glyph(ch) {
            usage.missing.insert(ch);
        }
        match &self.kind {
            FontKind::Standard { encoding, .. } => out.push(encoding.code_or_fallback(ch)),
            FontKind::Embedded { .. } => {
                let cid = match usage.cids.get(&ch) {
                    Some(&cid) => cid,
                    None if usage.chars.len() < usize::from(u16::MAX) => {
                        usage.chars.push(ch);
                        usage.written = false;
                        let cid = usage.chars.len() as u16;
                        usage.cids.insert(ch, cid);
                        cid
                    }
                    None => {
                        usage.missing.insert(ch);
                        0
                    }
                };
                out.extend_from_slice(&cid.to_be_bytes());
            }
        }
    }

    /// Write the font at its reserved numbers: a standard font's dictionary,
    /// or the Type0 font and the objects under it from the characters drawn so
    /// far. Nothing when it was written before and has drawn no new character
    /// since.
    fn write(&self, writer: &mut PdfWriter) {
        let mut usage = self.usage.lock().unwrap_or_else(|e| e.into_inner());
        if usage.written {
            return;
        }
        usage.written = true;
        let (program, embedding, objects) = match &self.kind {
            FontKind::Standard { base_font, .. } => {
                writer.set_object(
                    self.font_ref.obj_num,
                    PdfObject::Dict(crate::writer::document::standard_font_dict(base_font)),
                );
                return;
            }
            FontKind::Embedded {
                program,
                embedding,
                objects,
            } => (program, embedding, objects),
        };
        let gids: Vec<u16> = usage
            .chars
            .iter()
            .map(|&ch| program.glyph_id(ch).unwrap_or(0))
            .collect();

        let (font_data, base_font) = match embedding {
            FontEmbedding::Whole => (program.data().to_vec(), program.postscript_name()),
            FontEmbedding::Subset => {
                let mut keep: Vec<u16> = gids.iter().copied().chain([0]).collect();
                keep.sort_unstable();
                keep.dedup();
                match super::subset::subset_font(program.data(), &keep) {
                    Some(subset) => (
                        subset.data,
                        format!("{}+{}", subset_tag(&keep), program.postscript_name()),
                    ),
                    None => (program.data().to_vec(), program.postscript_name()),
                }
            }
        };
        let base_font = base_font.into_bytes();

        let (mut file_dict, file_data) = make_stream(&font_data, true);
        file_dict.insert(
            b"Length1".to_vec(),
            PdfObject::Integer(font_data.len() as i64),
        );
        writer.set_object(
            objects.font_file,
            PdfObject::Stream {
                dict: file_dict,
                data: file_data,
            },
        );

        writer.set_object(
            objects.descriptor,
            PdfObject::Dict(descriptor(program, &base_font, objects.font_file)),
        );

        let mut cid_to_gid = vec![0u8; 2];
        for gid in &gids {
            cid_to_gid.extend_from_slice(&gid.to_be_bytes());
        }
        let (map_dict, map_data) = make_stream(&cid_to_gid, true);
        writer.set_object(
            objects.cid_to_gid,
            PdfObject::Stream {
                dict: map_dict,
                data: map_data,
            },
        );

        let widths: Vec<(u16, u16)> = gids
            .iter()
            .enumerate()
            .map(|(i, &gid)| (i as u16 + 1, rounded_width(program, gid)))
            .collect();
        let mut cid_font = PdfDict::new();
        cid_font.insert(b"Type".to_vec(), PdfObject::Name(b"Font".to_vec()));
        cid_font.insert(
            b"Subtype".to_vec(),
            PdfObject::Name(b"CIDFontType2".to_vec()),
        );
        cid_font.insert(b"BaseFont".to_vec(), PdfObject::Name(base_font.clone()));
        cid_font.insert(
            b"CIDSystemInfo".to_vec(),
            PdfObject::Dict(CJKOrdering::Identity.system_info().to_pdf_dict()),
        );
        cid_font.insert(
            b"FontDescriptor".to_vec(),
            PdfObject::Reference(reference(objects.descriptor)),
        );
        cid_font.insert(
            b"DW".to_vec(),
            PdfObject::Integer(i64::from(rounded_width(program, 0))),
        );
        if !widths.is_empty() {
            cid_font.insert(b"W".to_vec(), PdfObject::Array(build_w_array(widths)));
        }
        cid_font.insert(
            b"CIDToGIDMap".to_vec(),
            PdfObject::Reference(reference(objects.cid_to_gid)),
        );
        writer.set_object(objects.cid_font, PdfObject::Dict(cid_font));

        let mappings: Vec<(u16, char)> = usage
            .chars
            .iter()
            .enumerate()
            .map(|(i, &ch)| (i as u16 + 1, ch))
            .collect();
        let (cmap_dict, cmap_data) = make_stream(&generate_to_unicode_cmap(&mappings), true);
        writer.set_object(
            objects.to_unicode,
            PdfObject::Stream {
                dict: cmap_dict,
                data: cmap_data,
            },
        );

        let mut type0 = PdfDict::new();
        type0.insert(b"Type".to_vec(), PdfObject::Name(b"Font".to_vec()));
        type0.insert(b"Subtype".to_vec(), PdfObject::Name(b"Type0".to_vec()));
        type0.insert(b"BaseFont".to_vec(), PdfObject::Name(base_font));
        type0.insert(
            b"Encoding".to_vec(),
            PdfObject::Name(b"Identity-H".to_vec()),
        );
        type0.insert(
            b"DescendantFonts".to_vec(),
            PdfObject::Array(vec![PdfObject::Reference(reference(objects.cid_font))]),
        );
        type0.insert(
            b"ToUnicode".to_vec(),
            PdfObject::Reference(reference(objects.to_unicode)),
        );
        writer.set_object(self.font_ref.obj_num, PdfObject::Dict(type0));
    }
}

fn reference(obj_num: u32) -> IndirectRef {
    IndirectRef {
        obj_num,
        gen_num: 0,
    }
}

/// Advance of glyph `gid` in thousandths of an em, rounded.
fn rounded_width(program: &FontProgram, gid: u16) -> u16 {
    program
        .glyph_advance(gid)
        .round()
        .clamp(0.0, f64::from(u16::MAX)) as u16
}

/// The font descriptor of `program`, from its `head`, `hhea`, `OS/2` and `post`
/// tables, scaled to thousandths of an em.
fn descriptor(program: &FontProgram, font_name: &[u8], font_file: u32) -> PdfDict {
    let face = program.face();
    let scale = 1000.0 / program.units_per_em();
    let scaled = |v: i16| PdfObject::Integer((f64::from(v) * scale).round() as i64);
    let bbox = face.global_bounding_box();
    let mut flags = FontDescriptor::NONSYMBOLIC;
    if face.is_monospaced() {
        flags |= FontDescriptor::FIXED_PITCH;
    }
    if face.is_italic() {
        flags |= FontDescriptor::ITALIC;
    }

    let mut fd = PdfDict::new();
    fd.insert(
        b"Type".to_vec(),
        PdfObject::Name(b"FontDescriptor".to_vec()),
    );
    fd.insert(b"FontName".to_vec(), PdfObject::Name(font_name.to_vec()));
    fd.insert(b"Flags".to_vec(), PdfObject::Integer(i64::from(flags)));
    fd.insert(
        b"FontBBox".to_vec(),
        PdfObject::Array(vec![
            scaled(bbox.x_min),
            scaled(bbox.y_min),
            scaled(bbox.x_max),
            scaled(bbox.y_max),
        ]),
    );
    fd.insert(
        b"ItalicAngle".to_vec(),
        PdfObject::Real(f64::from(face.italic_angle())),
    );
    fd.insert(b"Ascent".to_vec(), scaled(face.ascender()));
    fd.insert(b"Descent".to_vec(), scaled(face.descender()));
    fd.insert(
        b"CapHeight".to_vec(),
        scaled(face.capital_height().unwrap_or(face.ascender())),
    );
    fd.insert(b"StemV".to_vec(), PdfObject::Integer(80));
    fd.insert(
        b"FontFile2".to_vec(),
        PdfObject::Reference(reference(font_file)),
    );
    fd
}

/// Six uppercase letters derived from the glyph IDs a subset keeps.
fn subset_tag(gids: &[u16]) -> String {
    let bytes: Vec<u8> = gids.iter().flat_map(|g| g.to_be_bytes()).collect();
    let mut hash = super::font_hash(&bytes);
    (0..6)
        .map(|_| {
            let letter = (b'A' + (hash % 26) as u8) as char;
            hash /= 26;
            letter
        })
        .collect()
}

/// The fonts registered on one document.
#[derive(Debug)]
pub(crate) struct FontRegistry {
    document: u64,
    resource_counter: u32,
    entries: Vec<Arc<FontEntry>>,
    standard: HashMap<String, Font>,
}

impl FontRegistry {
    pub(crate) fn new() -> Self {
        static NEXT_DOCUMENT: AtomicU64 = AtomicU64::new(1);
        Self {
            document: NEXT_DOCUMENT.fetch_add(1, Ordering::Relaxed),
            resource_counter: 0,
            entries: Vec::new(),
            standard: HashMap::new(),
        }
    }

    /// The next font resource name: `F1`, `F2`, …
    pub(crate) fn next_resource_name(&mut self) -> String {
        self.resource_counter += 1;
        format!("F{}", self.resource_counter)
    }

    /// Whether `entry` is registered on this document.
    pub(crate) fn owns(&self, entry: &FontEntry) -> bool {
        entry.document == self.document
    }

    /// The standard Type1 font `base_font`, its object number reserved in
    /// `writer` the first time and its dictionary written by [`Self::write`].
    pub(crate) fn add_standard(&mut self, writer: &mut PdfWriter, base_font: &str) -> Font {
        if let Some(font) = self.standard.get(base_font) {
            return font.clone();
        }
        let font_ref = reference(writer.alloc_object_num());
        let font = self.register(
            font_ref,
            FontKind::Standard {
                base_font: base_font.to_string(),
                encoding: StandardEncoding::of(base_font),
            },
        );
        self.standard.insert(base_font.to_string(), font.clone());
        font
    }

    /// `program` as an embedded font, its object numbers reserved in `writer`
    /// and its objects written by [`Self::write`].
    pub(crate) fn add_embedded(
        &mut self,
        writer: &mut PdfWriter,
        program: &FontProgram,
        embedding: FontEmbedding,
    ) -> Font {
        let font_ref = reference(writer.alloc_object_num());
        let objects = EmbeddedObjects {
            cid_font: writer.alloc_object_num(),
            descriptor: writer.alloc_object_num(),
            font_file: writer.alloc_object_num(),
            to_unicode: writer.alloc_object_num(),
            cid_to_gid: writer.alloc_object_num(),
        };
        self.register(
            font_ref,
            FontKind::Embedded {
                program: program.clone(),
                embedding,
                objects,
            },
        )
    }

    fn register(&mut self, font_ref: IndirectRef, kind: FontKind) -> Font {
        let entry = Arc::new(FontEntry {
            document: self.document,
            resource_name: self.next_resource_name(),
            font_ref,
            kind,
            usage: Mutex::new(Usage::default()),
            glyphs: Mutex::new(HashMap::new()),
        });
        self.entries.push(entry.clone());
        Font { chain: vec![entry] }
    }

    /// Write every font not yet written, and every embedded font that has drawn
    /// more since an earlier call, replacing what that call wrote.
    pub(crate) fn write(&self, writer: &mut PdfWriter) {
        for entry in &self.entries {
            entry.write(writer);
        }
    }

    /// The characters drawn with a font of this document that it has no glyph
    /// for.
    pub(crate) fn missing_chars(&self) -> BTreeSet<char> {
        self.entries
            .iter()
            .flat_map(|entry| {
                let usage = entry.usage.lock().unwrap_or_else(|e| e.into_inner());
                usage.missing.iter().copied().collect::<Vec<_>>()
            })
            .collect()
    }
}
