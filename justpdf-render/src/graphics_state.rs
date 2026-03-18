use justpdf_core::color::{Color, ColorSpace};
use tiny_skia::Mask;

/// Soft mask type (Luminosity or Alpha).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoftMaskSubtype {
    Luminosity,
    Alpha,
}

/// A resolved soft mask ready for use during rendering.
/// Contains a tiny_skia::Mask derived from rendering the mask form XObject.
#[derive(Clone)]
pub struct SoftMask {
    pub mask: Mask,
    pub subtype: SoftMaskSubtype,
}

impl std::fmt::Debug for SoftMask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SoftMask")
            .field("subtype", &self.subtype)
            .field("mask_size", &"<Mask>")
            .finish()
    }
}

/// 2D affine transformation matrix [a b 0; c d 0; e f 1].
/// PDF row-vector convention: point × matrix.
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

    /// self × other
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

    pub fn transform_point(&self, x: f64, y: f64) -> (f64, f64) {
        (
            self.a * x + self.c * y + self.e,
            self.b * x + self.d * y + self.f,
        )
    }

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

    pub fn scale(sx: f64, sy: f64) -> Self {
        Self {
            a: sx,
            b: 0.0,
            c: 0.0,
            d: sy,
            e: 0.0,
            f: 0.0,
        }
    }

    /// Convert to tiny-skia Transform.
    pub fn to_skia(&self) -> tiny_skia::Transform {
        tiny_skia::Transform::from_row(
            self.a as f32,
            self.b as f32,
            self.c as f32,
            self.d as f32,
            self.e as f32,
            self.f as f32,
        )
    }

    /// Effective y-scale (for font size).
    pub fn font_size_scale(&self) -> f64 {
        (self.b * self.b + self.d * self.d).sqrt()
    }
}

/// Text state parameters (PDF spec 9.3).
#[derive(Debug, Clone)]
pub struct TextState {
    pub char_spacing: f64,
    pub word_spacing: f64,
    pub horiz_scaling: f64,
    pub leading: f64,
    pub font_name: Vec<u8>,
    pub font_size: f64,
    pub text_rise: f64,
    pub render_mode: i64,
}

impl Default for TextState {
    fn default() -> Self {
        Self {
            char_spacing: 0.0,
            word_spacing: 0.0,
            horiz_scaling: 1.0,
            leading: 0.0,
            font_name: Vec::new(),
            font_size: 12.0,
            text_rise: 0.0,
            render_mode: 0,
        }
    }
}

/// Line cap style.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineCap {
    Butt = 0,
    Round = 1,
    Square = 2,
}

/// Line join style.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineJoin {
    Miter = 0,
    Round = 1,
    Bevel = 2,
}

/// PDF blend mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PdfBlendMode {
    Normal,
    Multiply,
    Screen,
    Overlay,
    Darken,
    Lighten,
    ColorDodge,
    ColorBurn,
    HardLight,
    SoftLight,
    Difference,
    Exclusion,
    Hue,
    Saturation,
    Color,
    Luminosity,
}

impl PdfBlendMode {
    pub fn from_name(name: &[u8]) -> Self {
        match name {
            b"Multiply" => Self::Multiply,
            b"Screen" => Self::Screen,
            b"Overlay" => Self::Overlay,
            b"Darken" => Self::Darken,
            b"Lighten" => Self::Lighten,
            b"ColorDodge" => Self::ColorDodge,
            b"ColorBurn" => Self::ColorBurn,
            b"HardLight" => Self::HardLight,
            b"SoftLight" => Self::SoftLight,
            b"Difference" => Self::Difference,
            b"Exclusion" => Self::Exclusion,
            b"Hue" => Self::Hue,
            b"Saturation" => Self::Saturation,
            b"Color" => Self::Color,
            b"Luminosity" => Self::Luminosity,
            _ => Self::Normal,
        }
    }

    pub fn to_skia(self) -> tiny_skia::BlendMode {
        match self {
            Self::Normal => tiny_skia::BlendMode::SourceOver,
            Self::Multiply => tiny_skia::BlendMode::Multiply,
            Self::Screen => tiny_skia::BlendMode::Screen,
            Self::Overlay => tiny_skia::BlendMode::Overlay,
            Self::Darken => tiny_skia::BlendMode::Darken,
            Self::Lighten => tiny_skia::BlendMode::Lighten,
            Self::ColorDodge => tiny_skia::BlendMode::ColorDodge,
            Self::ColorBurn => tiny_skia::BlendMode::ColorBurn,
            Self::HardLight => tiny_skia::BlendMode::HardLight,
            Self::SoftLight => tiny_skia::BlendMode::SoftLight,
            Self::Difference => tiny_skia::BlendMode::Difference,
            Self::Exclusion => tiny_skia::BlendMode::Exclusion,
            Self::Hue => tiny_skia::BlendMode::Hue,
            Self::Saturation => tiny_skia::BlendMode::Saturation,
            Self::Color => tiny_skia::BlendMode::Color,
            Self::Luminosity => tiny_skia::BlendMode::Luminosity,
        }
    }
}

/// Full graphics state for rendering.
#[derive(Debug, Clone)]
pub struct GraphicsState {
    pub ctm: Matrix,
    pub text: TextState,
    // Stroke/fill colors
    pub fill_color: Color,
    pub stroke_color: Color,
    pub fill_cs: ColorSpace,
    pub stroke_cs: ColorSpace,
    // Line drawing parameters
    pub line_width: f64,
    pub line_cap: LineCap,
    pub line_join: LineJoin,
    pub miter_limit: f64,
    pub dash_pattern: Vec<f64>,
    pub dash_phase: f64,
    // Transparency
    pub fill_alpha: f64,
    pub stroke_alpha: f64,
    pub blend_mode: PdfBlendMode,
    // Clipping
    pub has_clip: bool,
    // Soft mask (from ExtGState /SMask)
    pub soft_mask: Option<SoftMask>,
    // Fill pattern name (when color space is /Pattern)
    pub fill_pattern_name: Option<Vec<u8>>,
    // Stroke pattern name (when color space is /Pattern)
    pub stroke_pattern_name: Option<Vec<u8>>,
    // Text matrices (only valid inside BT..ET)
    pub text_matrix: Matrix,
    pub text_line_matrix: Matrix,
}

impl Default for GraphicsState {
    fn default() -> Self {
        Self {
            ctm: Matrix::identity(),
            text: TextState::default(),
            fill_color: Color::gray(0.0),
            stroke_color: Color::gray(0.0),
            fill_cs: ColorSpace::DeviceGray,
            stroke_cs: ColorSpace::DeviceGray,
            line_width: 1.0,
            line_cap: LineCap::Butt,
            line_join: LineJoin::Miter,
            miter_limit: 10.0,
            dash_pattern: Vec::new(),
            dash_phase: 0.0,
            fill_alpha: 1.0,
            stroke_alpha: 1.0,
            blend_mode: PdfBlendMode::Normal,
            has_clip: false,
            soft_mask: None,
            fill_pattern_name: None,
            stroke_pattern_name: None,
            text_matrix: Matrix::identity(),
            text_line_matrix: Matrix::identity(),
        }
    }
}

impl GraphicsState {
    pub fn fill_color_rgba(&self) -> [u8; 4] {
        let rgb = self.fill_color.to_rgb(&self.fill_cs);
        [
            (rgb[0].clamp(0.0, 1.0) * 255.0) as u8,
            (rgb[1].clamp(0.0, 1.0) * 255.0) as u8,
            (rgb[2].clamp(0.0, 1.0) * 255.0) as u8,
            (self.fill_alpha.clamp(0.0, 1.0) * 255.0) as u8,
        ]
    }

    pub fn stroke_color_rgba(&self) -> [u8; 4] {
        let rgb = self.stroke_color.to_rgb(&self.stroke_cs);
        [
            (rgb[0].clamp(0.0, 1.0) * 255.0) as u8,
            (rgb[1].clamp(0.0, 1.0) * 255.0) as u8,
            (rgb[2].clamp(0.0, 1.0) * 255.0) as u8,
            (self.stroke_alpha.clamp(0.0, 1.0) * 255.0) as u8,
        ]
    }
}
