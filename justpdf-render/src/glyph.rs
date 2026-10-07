use std::collections::HashMap;

use justpdf_core::font::{Encoding, encoding_glyph_name, truetype_glyph_candidates};
use justpdf_core::ttf_parser::{self, Face, GlyphId, cff};
use tiny_skia::{Path, PathBuilder};

/// A font program glyphs are drawn from: an sfnt (TrueType or OpenType) or a
/// bare CFF (`/FontFile3 /Subtype /Type1C` or `/CIDFontType0C`).
pub enum GlyphSource<'a> {
    Sfnt(Box<Face<'a>>),
    Cff(Box<cff::Table<'a>>),
}

impl<'a> GlyphSource<'a> {
    /// Parses `data` as an sfnt, else as a bare CFF.
    pub fn parse(data: &'a [u8]) -> Option<Self> {
        match Face::parse(data, 0) {
            Ok(face) => Some(Self::Sfnt(Box::new(face))),
            Err(_) => cff::Table::parse(data).map(|t| Self::Cff(Box::new(t))),
        }
    }

    /// The outline of `glyph_id`, or `None` when it has none.
    pub fn outline(&self, glyph_id: GlyphId) -> Option<Path> {
        let mut builder = OutlineBuilder::new();
        match self {
            Self::Sfnt(face) => face.outline_glyph(glyph_id, &mut builder)?,
            Self::Cff(table) => table.outline(glyph_id, &mut builder).ok()?,
        };
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
        }
    }

    /// The glyph a simple font's one-byte `code` draws, or `None` for no glyph.
    ///
    /// `encoding` is the font's `/Encoding` base (`None` when it has none),
    /// `explicit_base` that base only when the PDF names it (an encoding name
    /// or `/BaseEncoding`), and `symbolic` bit 3 of the descriptor's `/Flags`.
    /// An sfnt goes through `truetype_glyph_candidates`. A CFF goes by glyph
    /// name — `/Differences`, else the explicit base — and otherwise through
    /// its built-in encoding, the implicit base of an embedded font program.
    pub fn simple_glyph(
        &self,
        code: u8,
        encoding: Option<Encoding>,
        explicit_base: Option<Encoding>,
        differences: &[(u8, Vec<u8>)],
        symbolic: bool,
    ) -> Option<GlyphId> {
        match self {
            Self::Sfnt(face) => {
                truetype_glyph_candidates(face, code, encoding, differences, symbolic)
                    .first()
                    .copied()
            }
            Self::Cff(table) => match encoding_glyph_name(code, explicit_base, differences) {
                Some(name) => table.glyph_index_by_name(name),
                None => table.glyph_index(code),
            },
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
