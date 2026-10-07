use std::collections::HashMap;

use justpdf_core::font::{Encoding, encoding_glyph_name, truetype_glyph_candidates};
use justpdf_core::ttf_parser::{self, Face, GlyphId, cff};
use tiny_skia::{Path, PathBuilder};

/// A font program glyphs are drawn from: an sfnt (TrueType or OpenType), a
/// bare CFF (`/FontFile3 /Subtype /Type1C` or `/CIDFontType0C`), or a Type1
/// program (`/FontFile`, through `hayro_font`).
pub enum GlyphSource<'a> {
    Sfnt(Box<Face<'a>>),
    Cff(Box<cff::Table<'a>>),
    Type1(Box<hayro_font::type1::Table>),
}

/// A glyph of a [`GlyphSource`]: by glyph ID (sfnt, CFF) or by name (Type1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GlyphRef {
    Id(GlyphId),
    Name(String),
}

impl GlyphRef {
    /// The glyph part of a [`crate::glyph_cache::GlyphCache`] key for a program
    /// hashed as `program_hash`: `(program_hash, id)` for an ID, and the
    /// program hash mixed with the name's hash for a name.
    pub fn cache_key(&self, program_hash: u64) -> (u64, u16) {
        match self {
            Self::Id(gid) => (program_hash, gid.0),
            Self::Name(name) => (
                program_hash ^ crate::glyph_cache::font_hash(name.as_bytes()),
                u16::MAX,
            ),
        }
    }
}

impl<'a> GlyphSource<'a> {
    /// Parses `data` as an sfnt, else as a bare CFF, else as a Type1 program.
    pub fn parse(data: &'a [u8]) -> Option<Self> {
        if let Ok(face) = Face::parse(data, 0) {
            return Some(Self::Sfnt(Box::new(face)));
        }
        if let Some(table) = cff::Table::parse(data) {
            return Some(Self::Cff(Box::new(table)));
        }
        hayro_font::type1::Table::parse(data).map(|t| Self::Type1(Box::new(t)))
    }

    /// The outline of `glyph`, or `None` when it has none.
    pub fn outline(&self, glyph: &GlyphRef) -> Option<Path> {
        let mut builder = OutlineBuilder::new();
        match (self, glyph) {
            (Self::Sfnt(face), GlyphRef::Id(gid)) => {
                face.outline_glyph(*gid, &mut builder)?;
            }
            (Self::Cff(table), GlyphRef::Id(gid)) => {
                table.outline(*gid, &mut builder).ok()?;
            }
            (Self::Type1(table), GlyphRef::Name(name)) => {
                table.outline(name, &mut builder)?;
            }
            _ => return None,
        }
        builder.finish()
    }

    /// The matrix from glyph space to text space for a 1-unit font size:
    /// `1 / unitsPerEm` for an sfnt, the `FontMatrix` for a CFF.
    pub fn font_matrix(&self) -> [f64; 6] {
        match self {
            Self::Sfnt(face) => {
                let s = 1.0 / f64::from(face.units_per_em().max(1));
                [s, 0.0, 0.0, s, 0.0, 0.0]
            }
            Self::Cff(table) => {
                let m = table.matrix();
                [m.sx, m.ky, m.kx, m.sy, m.tx, m.ty].map(f64::from)
            }
            Self::Type1(table) => {
                let m = table.matrix();
                [m.sx, m.ky, m.kx, m.sy, m.tx, m.ty].map(f64::from)
            }
        }
    }

    /// The glyph a simple font's one-byte `code` draws, or `None` for no glyph.
    ///
    /// `encoding` is the font's `/Encoding` base (`None` when it has none),
    /// `explicit_base` that base only when the PDF names it (an encoding name
    /// or `/BaseEncoding`), and `symbolic` bit 3 of the descriptor's `/Flags`.
    /// An sfnt goes through `truetype_glyph_candidates`. A CFF or Type1 font
    /// goes by glyph name — `/Differences`, else the explicit base — and
    /// otherwise through its built-in encoding, the implicit base of an
    /// embedded font program.
    pub fn simple_glyph(
        &self,
        code: u8,
        encoding: Option<Encoding>,
        explicit_base: Option<Encoding>,
        differences: &[(u8, Vec<u8>)],
        symbolic: bool,
    ) -> Option<GlyphRef> {
        let name = encoding_glyph_name(code, explicit_base, differences);
        match self {
            Self::Sfnt(face) => {
                truetype_glyph_candidates(face, code, encoding, differences, symbolic)
                    .first()
                    .map(|gid| GlyphRef::Id(*gid))
            }
            Self::Cff(table) => match name {
                Some(name) => table.glyph_index_by_name(name),
                None => table.glyph_index(code),
            }
            .map(GlyphRef::Id),
            Self::Type1(table) => name
                .or_else(|| table.code_to_string(code))
                .map(|n| GlyphRef::Name(n.to_string())),
        }
    }

    /// CID to glyph ID for a CID-keyed CFF, from its charset; `None` for any
    /// other program.
    pub fn cff_cid_to_gid(&self) -> Option<HashMap<u16, u16>> {
        let Self::Cff(table) = self else { return None };
        let map: HashMap<u16, u16> = (0..table.number_of_glyphs())
            .filter_map(|gid| table.glyph_cid(GlyphId(gid)).map(|cid| (cid, gid)))
            .collect();
        (!map.is_empty()).then_some(map)
    }
}

/// Build a tiny-skia Path from a glyph outline.
/// Returns None if the glyph has no outline.
pub fn glyph_outline(face: &Face, glyph_id: ttf_parser::GlyphId) -> Option<Path> {
    let mut builder = OutlineBuilder::new();
    face.outline_glyph(glyph_id, &mut builder)?;
    builder.finish()
}

/// Get the units-per-em for a face (for coordinate normalization).
pub fn units_per_em(face: &Face) -> f64 {
    face.units_per_em() as f64
}

/// Internal outline builder that converts ttf-parser callbacks to tiny-skia paths.
struct OutlineBuilder {
    pb: PathBuilder,
    has_points: bool,
}

impl OutlineBuilder {
    fn new() -> Self {
        Self {
            pb: PathBuilder::new(),
            has_points: false,
        }
    }

    fn finish(self) -> Option<Path> {
        if self.has_points {
            self.pb.finish()
        } else {
            None
        }
    }
}

impl hayro_font::OutlineBuilder for OutlineBuilder {
    fn move_to(&mut self, x: f32, y: f32) {
        ttf_parser::OutlineBuilder::move_to(self, x, y);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        ttf_parser::OutlineBuilder::line_to(self, x, y);
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        ttf_parser::OutlineBuilder::quad_to(self, x1, y1, x, y);
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        ttf_parser::OutlineBuilder::curve_to(self, x1, y1, x2, y2, x, y);
    }

    fn close(&mut self) {
        ttf_parser::OutlineBuilder::close(self);
    }
}

impl ttf_parser::OutlineBuilder for OutlineBuilder {
    fn move_to(&mut self, x: f32, y: f32) {
        self.pb.move_to(x, y);
        self.has_points = true;
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.pb.line_to(x, y);
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.pb.quad_to(x1, y1, x, y);
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.pb.cubic_to(x1, y1, x2, y2, x, y);
    }

    fn close(&mut self) {
        self.pb.close();
    }
}
