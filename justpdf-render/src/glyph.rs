use tiny_skia::{PathBuilder, Path};
use ttf_parser::Face;

/// Build a tiny-skia Path from a glyph outline.
/// Returns None if the glyph has no outline.
pub fn glyph_outline(face: &Face, glyph_id: ttf_parser::GlyphId) -> Option<Path> {
    let mut builder = OutlineBuilder::new();
    face.outline_glyph(glyph_id, &mut builder)?;
    builder.finish()
}

/// Map a character code to a glyph ID for simple fonts (non-CID).
/// Uses the cmap table if available, otherwise uses identity mapping.
pub fn char_code_to_glyph_id(face: &Face, code: u32) -> ttf_parser::GlyphId {
    // Try direct Unicode cmap lookup
    if let Some(c) = char::from_u32(code) {
        if let Some(gid) = face.glyph_index(c) {
            return gid;
        }
    }
    // Fallback: treat code as glyph ID directly
    ttf_parser::GlyphId(code as u16)
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
