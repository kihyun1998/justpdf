//! Decoded font programs, as [`crate::PdfDocument::font_program`] shares them,
//! and caller-supplied TrueType programs for drawing text ([`FontProgram`]).

use std::sync::Arc;

use crate::error::{JustPdfError, Result};

/// A caller-supplied TrueType font program: one standalone font, or one face
/// of a TrueType collection extracted as a standalone font. Cloning shares the
/// bytes. Registered on a document with
/// [`DocumentBuilder::add_font`](crate::writer::DocumentBuilder::add_font) or
/// [`DocumentModifier::add_font`](crate::writer::DocumentModifier::add_font).
#[derive(Debug, Clone)]
pub struct FontProgram {
    /// The standalone TrueType font.
    data: Arc<[u8]>,
    units_per_em: f64,
}

impl FontProgram {
    /// Parse a TrueType font (`.ttf`), or face `face_index` of a TrueType
    /// collection (`.ttc`); `face_index` is 0 for a single font.
    ///
    /// Returns [`JustPdfError::FontError`] for a CFF-flavoured OpenType font
    /// (`OTTO`), a face index the data does not have, or data that does not
    /// parse as a TrueType font with `glyf` outlines.
    pub fn parse(bytes: impl Into<Arc<[u8]>>, face_index: u32) -> Result<FontProgram> {
        let bytes: Arc<[u8]> = bytes.into();
        let data: Arc<[u8]> = match bytes.get(..4) {
            Some(b"OTTO") => return Err(cff_error()),
            Some(b"ttcf") => collection_face(&bytes, face_index)?.into(),
            _ if face_index != 0 => {
                return Err(font_error(format!(
                    "face index {face_index} requested from a single font, which has only face 0"
                )));
            }
            _ => bytes,
        };
        if data.get(..4) == Some(b"OTTO") {
            return Err(cff_error());
        }
        let face = ttf_parser::Face::parse(&data, 0)
            .map_err(|e| font_error(format!("not a TrueType font: {e}")))?;
        if face.tables().glyf.is_none() {
            return Err(font_error("the font has no glyf table".into()));
        }
        let units_per_em = f64::from(face.units_per_em());
        Ok(FontProgram { data, units_per_em })
    }

    /// The standalone TrueType font bytes.
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// The parsed font.
    pub(crate) fn face(&self) -> ttf_parser::Face<'_> {
        ttf_parser::Face::parse(&self.data, 0).expect("parsed in FontProgram::parse")
    }

    /// The glyph `ch` maps to through the font's `cmap`, `None` when it has
    /// none.
    pub fn glyph_id(&self, ch: char) -> Option<u16> {
        self.face()
            .glyph_index(ch)
            .map(|g| g.0)
            .filter(|&gid| gid != 0)
    }

    /// Horizontal advance of `ch`'s glyph in thousandths of an em, `None`
    /// when the font has no glyph for `ch`.
    pub fn advance(&self, ch: char) -> Option<f64> {
        self.glyph_id(ch).map(|gid| self.glyph_advance(gid))
    }

    /// Horizontal advance of glyph `gid` in thousandths of an em, from `hmtx`.
    pub(crate) fn glyph_advance(&self, gid: u16) -> f64 {
        let advance = self
            .face()
            .glyph_hor_advance(ttf_parser::GlyphId(gid))
            .unwrap_or(0);
        f64::from(advance) * 1000.0 / self.units_per_em
    }

    /// Font design units per em, from `head`.
    pub(crate) fn units_per_em(&self) -> f64 {
        self.units_per_em
    }

    /// The PostScript name from the `name` table, or `"Font"` when it has none.
    pub fn postscript_name(&self) -> String {
        self.face()
            .names()
            .into_iter()
            .find(|n| n.name_id == ttf_parser::name_id::POST_SCRIPT_NAME)
            .and_then(|n| n.to_string())
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| "Font".to_string())
    }
}

fn font_error(detail: String) -> JustPdfError {
    JustPdfError::FontError { detail }
}

fn cff_error() -> JustPdfError {
    font_error(
        "CFF-flavoured OpenType fonts (OTTO) are not supported; supply a TrueType (glyf) font"
            .into(),
    )
}

/// Face `index` of the TrueType collection `ttc`, rewritten as a standalone
/// font: its offset table, and each of its tables copied after it.
fn collection_face(ttc: &[u8], index: u32) -> Result<Vec<u8>> {
    let u16_at = |at: usize| {
        ttc.get(at..at + 2)
            .map(|b| u16::from_be_bytes([b[0], b[1]]))
    };
    let u32_at = |at: usize| {
        ttc.get(at..at + 4)
            .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    };
    let malformed = || font_error("malformed TrueType collection".into());

    let num_fonts = u32_at(8).ok_or_else(malformed)?;
    if index >= num_fonts {
        return Err(font_error(format!(
            "face index {index} requested from a collection of {num_fonts} faces"
        )));
    }
    let face = (index as usize)
        .checked_mul(4)
        .and_then(|at| u32_at(12 + at))
        .ok_or_else(malformed)? as usize;
    let sfnt_version = ttc.get(face..face + 4).ok_or_else(malformed)?;
    let num_tables = u16_at(face + 4).ok_or_else(malformed)? as usize;

    let header_len = 12 + 16 * num_tables;
    let mut out = Vec::with_capacity(header_len);
    out.extend_from_slice(sfnt_version);
    out.extend_from_slice(ttc.get(face + 4..face + 12).ok_or_else(malformed)?);
    let mut body = Vec::new();
    for i in 0..num_tables {
        let rec = face + 12 + 16 * i;
        let record = ttc.get(rec..rec + 16).ok_or_else(malformed)?;
        let offset = u32_at(rec + 8).ok_or_else(malformed)? as usize;
        let length = u32_at(rec + 12).ok_or_else(malformed)? as usize;
        let table = ttc
            .get(offset..offset.checked_add(length).ok_or_else(malformed)?)
            .ok_or_else(malformed)?;
        if body.len() + table.len() > ttc.len() {
            return Err(font_error(
                "the face's tables add up to more than the collection holds".into(),
            ));
        }
        out.extend_from_slice(&record[..8]);
        out.extend_from_slice(&((header_len + body.len()) as u32).to_be_bytes());
        out.extend_from_slice(&record[12..16]);
        body.extend_from_slice(table);
        while body.len() % 4 != 0 {
            body.push(0);
        }
    }
    out.extend_from_slice(&body);
    Ok(out)
}

/// A decoded font program and its [`font_hash`].
#[derive(Debug)]
pub struct FontProgramData {
    /// The decoded program bytes.
    pub data: Arc<[u8]>,
    /// [`font_hash`] of `data`.
    pub hash: u64,
}

impl FontProgramData {
    /// `data` with its [`font_hash`].
    pub fn new(data: impl Into<Arc<[u8]>>) -> Self {
        let data = data.into();
        let hash = font_hash(&data);
        Self { data, hash }
    }
}

/// FNV-1a 64-bit hash of a font program's length and all of its bytes.
pub fn font_hash(data: &[u8]) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x00000100000001B3;

    let mut hash = FNV_OFFSET;
    for &b in (data.len() as u64).to_le_bytes().iter().chain(data) {
        hash ^= b as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}
