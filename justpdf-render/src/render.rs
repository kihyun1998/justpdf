use std::path::Path;

use justpdf_core::page::{PageInfo, collect_pages};
use justpdf_core::PdfDocument;

use crate::device::PixmapDevice;
use crate::error::{RenderError, Result};
use crate::graphics_state::Matrix;
use crate::interpreter::RenderInterpreter;
use crate::svg_device::SvgRenderer;

/// Output format for rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Png,
    Jpeg { quality: u8 },
    /// Raw RGBA pixel data (4 bytes per pixel, row-major, top-left origin).
    RawRgba,
}

pub struct RenderOptions {
    /// DPI for rendering (default: 72, which is 1:1 with PDF points).
    pub dpi: f64,
    /// Background color (RGBA). Default: white opaque.
    pub background: [u8; 4],
    /// Output format. Default: PNG.
    pub format: OutputFormat,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            dpi: 72.0,
            background: [255, 255, 255, 255],
            format: OutputFormat::Png,
        }
    }
}

/// Render a single page of a PDF document to PNG bytes.
///
/// `page_index` is 0-based.
pub fn render_page(
    doc: &PdfDocument,
    page_index: usize,
    options: &RenderOptions,
) -> Result<Vec<u8>> {
    let pages = collect_pages(doc)?;
    let page = pages
        .get(page_index)
        .ok_or_else(|| RenderError::InvalidDimensions {
            detail: format!("page index {page_index} out of range (total: {})", pages.len()),
        })?
        .clone();

    render_page_info(doc, &page, options)
}

/// Render a page given its PageInfo.
pub fn render_page_info(
    doc: &PdfDocument,
    page: &PageInfo,
    options: &RenderOptions,
) -> Result<Vec<u8>> {
    let media_box = page.crop_box.unwrap_or(page.media_box);
    let page_width = media_box.width();
    let page_height = media_box.height();

    if page_width <= 0.0 || page_height <= 0.0 {
        return Err(RenderError::InvalidDimensions {
            detail: format!("page has zero/negative size: {page_width}x{page_height}"),
        });
    }

    let scale = options.dpi / 72.0;
    let pixel_width = (page_width * scale).ceil() as u32;
    let pixel_height = (page_height * scale).ceil() as u32;

    if pixel_width == 0 || pixel_height == 0 || pixel_width > 16384 || pixel_height > 16384 {
        return Err(RenderError::InvalidDimensions {
            detail: format!("pixel dimensions out of range: {pixel_width}x{pixel_height}"),
        });
    }

    let mut device = PixmapDevice::new(pixel_width, pixel_height)?;

    // Fill background
    device.clear(tiny_skia::Color::from_rgba8(
        options.background[0],
        options.background[1],
        options.background[2],
        options.background[3],
    ));

    // Build the page transform:
    // 1. Translate so media_box origin is at (0,0)
    // 2. Flip Y axis (PDF Y goes up, pixel Y goes down)
    // 3. Scale by DPI
    let page_transform = compute_page_transform(&media_box, scale, page.rotate);

    let mut interpreter = RenderInterpreter::new(doc, &mut device, page_transform);
    interpreter.render_page(page)?;

    match options.format {
        OutputFormat::Png => device.encode_png(),
        OutputFormat::Jpeg { quality } => device.encode_jpeg(quality),
        OutputFormat::RawRgba => Ok(device.raw_rgba().to_vec()),
    }
}

/// Render a page and save to a file.
pub fn render_page_to_file(
    doc: &PdfDocument,
    page_index: usize,
    options: &RenderOptions,
    output_path: &Path,
) -> Result<()> {
    let png_data = render_page(doc, page_index, options)?;
    std::fs::write(output_path, &png_data)?;
    Ok(())
}

/// Rendered pixmap data with dimensions.
pub struct RenderedPixmap {
    /// Raw RGBA pixel data (4 bytes per pixel).
    pub data: Vec<u8>,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

/// Render a page and return the raw pixmap (RGBA data + dimensions).
pub fn render_page_to_pixmap(
    doc: &PdfDocument,
    page_index: usize,
    options: &RenderOptions,
) -> Result<RenderedPixmap> {
    let pages = collect_pages(doc)?;
    let page = pages
        .get(page_index)
        .ok_or_else(|| RenderError::InvalidDimensions {
            detail: format!("page index {page_index} out of range (total: {})", pages.len()),
        })?
        .clone();

    let media_box = page.crop_box.unwrap_or(page.media_box);
    let page_width = media_box.width();
    let page_height = media_box.height();

    if page_width <= 0.0 || page_height <= 0.0 {
        return Err(RenderError::InvalidDimensions {
            detail: format!("page has zero/negative size: {page_width}x{page_height}"),
        });
    }

    let scale = options.dpi / 72.0;
    let pixel_width = (page_width * scale).ceil() as u32;
    let pixel_height = (page_height * scale).ceil() as u32;

    if pixel_width == 0 || pixel_height == 0 || pixel_width > 16384 || pixel_height > 16384 {
        return Err(RenderError::InvalidDimensions {
            detail: format!("pixel dimensions out of range: {pixel_width}x{pixel_height}"),
        });
    }

    let mut device = PixmapDevice::new(pixel_width, pixel_height)?;

    device.clear(tiny_skia::Color::from_rgba8(
        options.background[0],
        options.background[1],
        options.background[2],
        options.background[3],
    ));

    let page_transform = compute_page_transform(&media_box, scale, page.rotate);

    let mut interpreter = RenderInterpreter::new(doc, &mut device, page_transform);
    interpreter.render_page(&page)?;

    Ok(RenderedPixmap {
        data: device.raw_rgba().to_vec(),
        width: pixel_width,
        height: pixel_height,
    })
}

/// Render a single page of a PDF document to SVG string.
///
/// `page_index` is 0-based. Returns a complete SVG XML document.
pub fn render_page_to_svg(
    doc: &PdfDocument,
    page_index: usize,
) -> Result<String> {
    let pages = collect_pages(doc)?;
    let page = pages
        .get(page_index)
        .ok_or_else(|| RenderError::InvalidDimensions {
            detail: format!("page index {page_index} out of range (total: {})", pages.len()),
        })?
        .clone();

    let media_box = page.crop_box.unwrap_or(page.media_box);
    let page_width = media_box.width();
    let page_height = media_box.height();

    if page_width <= 0.0 || page_height <= 0.0 {
        return Err(RenderError::InvalidDimensions {
            detail: format!("page has zero/negative size: {page_width}x{page_height}"),
        });
    }

    // For SVG we use scale=1.0 (1pt = 1 SVG unit), no DPI scaling
    let page_transform = compute_page_transform(&media_box, 1.0, page.rotate);

    let renderer = SvgRenderer::new(doc, page_transform, page_width, page_height);
    renderer.render_page(&page)
}

/// Render multiple pages in parallel using rayon.
///
/// Returns a `Vec<Result<Vec<u8>>>` where each entry corresponds to
/// the rendered output of the page at the given index.
/// Requires the `parallel` feature.
#[cfg(feature = "parallel")]
pub fn render_pages_parallel(
    doc: &PdfDocument,
    page_indices: &[usize],
    options: &RenderOptions,
) -> Vec<Result<Vec<u8>>> {
    use rayon::prelude::*;

    let pages = match collect_pages(doc) {
        Ok(p) => p,
        Err(e) => {
            let msg = format!("failed to collect pages: {e}");
            return page_indices
                .iter()
                .map(|_| {
                    Err(RenderError::InvalidDimensions {
                        detail: msg.clone(),
                    })
                })
                .collect();
        }
    };

    page_indices
        .par_iter()
        .map(|&idx| {
            let page = pages
                .get(idx)
                .ok_or_else(|| RenderError::InvalidDimensions {
                    detail: format!("page index {idx} out of range (total: {})", pages.len()),
                })?;
            render_page_info(doc, page, options)
        })
        .collect()
}

/// Render all pages in parallel using rayon.
///
/// Requires the `parallel` feature.
#[cfg(feature = "parallel")]
pub fn render_all_pages_parallel(
    doc: &PdfDocument,
    options: &RenderOptions,
) -> Vec<Result<Vec<u8>>> {
    let pages = match collect_pages(doc) {
        Ok(p) => p,
        Err(e) => return vec![Err(e.into())],
    };

    let indices: Vec<usize> = (0..pages.len()).collect();
    render_pages_parallel(doc, &indices, options)
}

/// Compute the transform from PDF user space to device (pixel) space.
pub fn compute_page_transform(
    media_box: &justpdf_core::page::Rect,
    scale: f64,
    rotate: i64,
) -> Matrix {
    let w = media_box.width();
    let h = media_box.height();

    // Base transform: translate origin, flip Y, scale
    // PDF: origin at lower-left, Y up
    // Pixels: origin at upper-left, Y down
    let base = match rotate % 360 {
        90 | -270 => {
            // Rotate 90°: swap width/height
            Matrix {
                a: 0.0,
                b: -scale,
                c: scale,
                d: 0.0,
                e: -media_box.lly * scale,
                f: (media_box.llx + w) * scale,
            }
        }
        180 | -180 => Matrix {
            a: -scale,
            b: 0.0,
            c: 0.0,
            d: scale,
            e: (media_box.llx + w) * scale,
            f: -media_box.lly * scale,
        },
        270 | -90 => Matrix {
            a: 0.0,
            b: scale,
            c: -scale,
            d: 0.0,
            e: (media_box.lly + h) * scale,
            f: -media_box.llx * scale,
        },
        _ => {
            // 0° rotation (default)
            Matrix {
                a: scale,
                b: 0.0,
                c: 0.0,
                d: -scale,
                e: -media_box.llx * scale,
                f: (media_box.lly + h) * scale,
            }
        }
    };

    base
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_options_default() {
        let opts = RenderOptions::default();
        assert_eq!(opts.dpi, 72.0);
        assert_eq!(opts.background, [255, 255, 255, 255]);
    }

    #[cfg(feature = "parallel")]
    #[test]
    fn test_render_pages_parallel_empty() {
        use std::path::Path;
        let pdf_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../testpdf.pdf");
        if !pdf_path.exists() {
            eprintln!("skipping: testpdf.pdf not found");
            return;
        }
        let doc = justpdf_core::PdfDocument::open(&pdf_path).expect("failed to open PDF");
        let opts = RenderOptions::default();
        // Empty indices should return empty results.
        let results = render_pages_parallel(&doc, &[], &opts);
        assert!(results.is_empty());
    }

    #[cfg(feature = "parallel")]
    #[test]
    fn test_render_pages_parallel_out_of_range() {
        use std::path::Path;
        let pdf_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../testpdf.pdf");
        if !pdf_path.exists() {
            eprintln!("skipping: testpdf.pdf not found");
            return;
        }
        let doc = justpdf_core::PdfDocument::open(&pdf_path).expect("failed to open PDF");
        let opts = RenderOptions::default();
        // Out-of-range index should produce an error.
        let results = render_pages_parallel(&doc, &[9999], &opts);
        assert_eq!(results.len(), 1);
        assert!(results[0].is_err());
    }

    #[test]
    fn test_page_transform_identity_at_72dpi() {
        let media_box = justpdf_core::page::Rect {
            llx: 0.0,
            lly: 0.0,
            urx: 100.0,
            ury: 200.0,
        };
        let t = compute_page_transform(&media_box, 1.0, 0);
        // Point (0, 200) in PDF = (0, 0) in pixels (top-left)
        let (px, py) = t.transform_point(0.0, 200.0);
        assert!((px - 0.0).abs() < 0.001);
        assert!((py - 0.0).abs() < 0.001);

        // Point (100, 0) in PDF = (100, 200) in pixels (bottom-right)
        let (px, py) = t.transform_point(100.0, 0.0);
        assert!((px - 100.0).abs() < 0.001);
        assert!((py - 200.0).abs() < 0.001);
    }
}
