use tiny_skia::{
    BlendMode, Color, FillRule, LineCap as SkiaLineCap, LineJoin as SkiaLineJoin, Mask,
    Paint, Pixmap, Stroke, Transform,
};

use crate::error::{RenderError, Result};
use crate::graphics_state;

/// A rendering device that draws PDF operations onto a pixel buffer.
pub struct PixmapDevice {
    pub pixmap: Pixmap,
    pub(crate) clip_mask: Option<Mask>,
}

impl PixmapDevice {
    pub fn new(width: u32, height: u32) -> Result<Self> {
        let pixmap = Pixmap::new(width, height).ok_or_else(|| RenderError::InvalidDimensions {
            detail: format!("cannot create {width}x{height} pixmap"),
        })?;
        Ok(Self {
            pixmap,
            clip_mask: None,
        })
    }

    /// Set a clipping path (replaces existing clip).
    pub fn set_clip_path(
        &mut self,
        path: &tiny_skia::Path,
        fill_rule: FillRule,
        transform: Transform,
    ) {
        let w = self.pixmap.width();
        let h = self.pixmap.height();
        if let Some(mut mask) = Mask::new(w, h) {
            mask.fill_path(path, fill_rule, true, transform);
            self.clip_mask = Some(mask);
        }
    }

    /// Intersect the current clip with another path.
    pub fn intersect_clip_path(
        &mut self,
        path: &tiny_skia::Path,
        fill_rule: FillRule,
        transform: Transform,
    ) {
        if let Some(mask) = &mut self.clip_mask {
            mask.intersect_path(path, fill_rule, true, transform);
        } else {
            self.set_clip_path(path, fill_rule, transform);
        }
    }

    /// Clear the clip mask.
    pub fn clear_clip(&mut self) {
        self.clip_mask = None;
    }

    /// Fill a path.
    pub fn fill_path(
        &mut self,
        path: &tiny_skia::Path,
        fill_rule: FillRule,
        transform: Transform,
        color: [u8; 4],
        blend_mode: BlendMode,
    ) {
        let mut paint = Paint::default();
        paint.set_color(Color::from_rgba8(color[0], color[1], color[2], color[3]));
        paint.anti_alias = true;
        paint.blend_mode = blend_mode;

        let clip = self.clip_mask.as_ref();
        self.pixmap
            .fill_path(path, &paint, fill_rule, transform, clip);
    }

    /// Stroke a path.
    pub fn stroke_path(
        &mut self,
        path: &tiny_skia::Path,
        transform: Transform,
        color: [u8; 4],
        gs: &graphics_state::GraphicsState,
        blend_mode: BlendMode,
    ) {
        let mut paint = Paint::default();
        paint.set_color(Color::from_rgba8(color[0], color[1], color[2], color[3]));
        paint.anti_alias = true;
        paint.blend_mode = blend_mode;

        let mut stroke = Stroke::default();
        stroke.width = gs.line_width as f32;
        stroke.line_cap = match gs.line_cap {
            graphics_state::LineCap::Butt => SkiaLineCap::Butt,
            graphics_state::LineCap::Round => SkiaLineCap::Round,
            graphics_state::LineCap::Square => SkiaLineCap::Square,
        };
        stroke.line_join = match gs.line_join {
            graphics_state::LineJoin::Miter => SkiaLineJoin::Miter,
            graphics_state::LineJoin::Round => SkiaLineJoin::Round,
            graphics_state::LineJoin::Bevel => SkiaLineJoin::Bevel,
        };
        stroke.miter_limit = gs.miter_limit as f32;

        if !gs.dash_pattern.is_empty() {
            let dashes: Vec<f32> = gs.dash_pattern.iter().map(|d| *d as f32).collect();
            if let Some(dash) = tiny_skia::StrokeDash::new(dashes, gs.dash_phase as f32) {
                stroke.dash = Some(dash);
            }
        }

        let clip = self.clip_mask.as_ref();
        self.pixmap
            .stroke_path(path, &paint, &stroke, transform, clip);
    }

    /// Draw an RGBA image at the given transform.
    pub fn draw_image(
        &mut self,
        image_pixmap: &tiny_skia::PixmapRef,
        transform: Transform,
        alpha: f32,
        blend_mode: BlendMode,
    ) {
        let mut paint = tiny_skia::PixmapPaint::default();
        paint.opacity = alpha;
        paint.blend_mode = blend_mode;
        paint.quality = tiny_skia::FilterQuality::Bilinear;

        let clip = self.clip_mask.as_ref();
        self.pixmap
            .draw_pixmap(0, 0, *image_pixmap, &paint, transform, clip);
    }

    /// Draw a pixmap onto this device (for transparency group compositing).
    pub fn draw_pixmap(
        &mut self,
        src: &tiny_skia::PixmapRef,
        transform: Transform,
        alpha: f32,
        blend_mode: BlendMode,
    ) {
        let mut paint = tiny_skia::PixmapPaint::default();
        paint.opacity = alpha;
        paint.blend_mode = blend_mode;
        paint.quality = tiny_skia::FilterQuality::Bilinear;

        let clip = self.clip_mask.as_ref();
        self.pixmap
            .draw_pixmap(0, 0, *src, &paint, transform, clip);
    }

    /// Fill a path with a pattern (tiled pixmap).
    pub fn fill_path_with_pattern(
        &mut self,
        path: &tiny_skia::Path,
        fill_rule: FillRule,
        transform: Transform,
        pattern_pixmap: &tiny_skia::PixmapRef,
        pattern_transform: Transform,
        blend_mode: BlendMode,
    ) {
        let mut paint = Paint::default();
        paint.anti_alias = true;
        paint.blend_mode = blend_mode;

        paint.shader = tiny_skia::Pattern::new(
            *pattern_pixmap,
            tiny_skia::SpreadMode::Repeat,
            tiny_skia::FilterQuality::Bilinear,
            1.0,
            pattern_transform,
        );

        let clip = self.clip_mask.as_ref();
        self.pixmap
            .fill_path(path, &paint, fill_rule, transform, clip);
    }

    /// Stroke a path with a pattern (tiled pixmap).
    pub fn stroke_path_with_pattern(
        &mut self,
        path: &tiny_skia::Path,
        transform: Transform,
        gs: &graphics_state::GraphicsState,
        pattern_pixmap: &tiny_skia::PixmapRef,
        pattern_transform: Transform,
        blend_mode: BlendMode,
    ) {
        let mut paint = Paint::default();
        paint.anti_alias = true;
        paint.blend_mode = blend_mode;

        paint.shader = tiny_skia::Pattern::new(
            *pattern_pixmap,
            tiny_skia::SpreadMode::Repeat,
            tiny_skia::FilterQuality::Bilinear,
            1.0,
            pattern_transform,
        );

        let mut stroke = Stroke::default();
        stroke.width = gs.line_width as f32;
        stroke.line_cap = match gs.line_cap {
            graphics_state::LineCap::Butt => SkiaLineCap::Butt,
            graphics_state::LineCap::Round => SkiaLineCap::Round,
            graphics_state::LineCap::Square => SkiaLineCap::Square,
        };
        stroke.line_join = match gs.line_join {
            graphics_state::LineJoin::Miter => SkiaLineJoin::Miter,
            graphics_state::LineJoin::Round => SkiaLineJoin::Round,
            graphics_state::LineJoin::Bevel => SkiaLineJoin::Bevel,
        };
        stroke.miter_limit = gs.miter_limit as f32;

        if !gs.dash_pattern.is_empty() {
            let dashes: Vec<f32> = gs.dash_pattern.iter().map(|d| *d as f32).collect();
            if let Some(dash) = tiny_skia::StrokeDash::new(dashes, gs.dash_phase as f32) {
                stroke.dash = Some(dash);
            }
        }

        let clip = self.clip_mask.as_ref();
        self.pixmap
            .stroke_path(path, &paint, &stroke, transform, clip);
    }

    /// Fill the entire pixmap with a color.
    pub fn clear(&mut self, color: Color) {
        self.pixmap.fill(color);
    }

    /// Encode the pixmap as PNG bytes.
    pub fn encode_png(&self) -> Result<Vec<u8>> {
        self.pixmap
            .encode_png()
            .map_err(|e| RenderError::Encode {
                detail: e.to_string(),
            })
    }

    /// Get the raw RGBA pixel data.
    pub fn raw_rgba(&self) -> &[u8] {
        self.pixmap.data()
    }

    /// Get the pixmap dimensions (width, height).
    pub fn dimensions(&self) -> (u32, u32) {
        (self.pixmap.width(), self.pixmap.height())
    }

    /// Encode the pixmap as JPEG bytes.
    pub fn encode_jpeg(&self, quality: u8) -> Result<Vec<u8>> {
        let width = self.pixmap.width();
        let height = self.pixmap.height();
        let rgba_data = self.pixmap.data();

        // Convert RGBA to RGB for JPEG
        let mut rgb_data = Vec::with_capacity((width * height * 3) as usize);
        for pixel in rgba_data.chunks(4) {
            rgb_data.push(pixel[0]);
            rgb_data.push(pixel[1]);
            rgb_data.push(pixel[2]);
        }

        let mut buf = std::io::Cursor::new(Vec::new());
        let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, quality);
        image::ImageEncoder::write_image(
            encoder,
            &rgb_data,
            width,
            height,
            image::ColorType::Rgb8.into(),
        )
        .map_err(|e| RenderError::Encode {
            detail: e.to_string(),
        })?;

        Ok(buf.into_inner())
    }
}
