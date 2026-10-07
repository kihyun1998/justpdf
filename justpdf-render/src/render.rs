use std::path::Path;

use justpdf_core::page::{PageInfo, collect_pages};
use justpdf_core::{JustPdfError, PdfDocument};

use crate::device::PixmapDevice;
use crate::error::{RenderError, Result};
use crate::graphics_state::Matrix;
use crate::interpreter::RenderInterpreter;
use crate::svg_device::SvgRenderer;

/// Output format for rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Png,
    Jpeg {
        quality: u8,
    },
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
        .ok_or(RenderError::Core(JustPdfError::PageOutOfRange {
            index: page_index,
            count: pages.len(),
        }))?
        .clone();

    render_page_info(doc, &page, options)
}

/// Render a page given its PageInfo.
pub fn render_page_info(
    doc: &PdfDocument,
    page: &PageInfo,
    options: &RenderOptions,
) -> Result<Vec<u8>> {
    let device = render_to_device(doc, page, options)?;
    match options.format {
        OutputFormat::Png => device.encode_png(),
        OutputFormat::Jpeg { quality } => device.encode_jpeg(quality),
        OutputFormat::RawRgba => Ok(device.raw_rgba().to_vec()),
    }
}

/// The visible box of a page placed on the output: its size after `/Rotate`,
/// in output units, and the transform from PDF user space onto it.
struct PageFrame {
    width: f64,
    height: f64,
    transform: Matrix,
}

/// The [`PageFrame`] of `page` at `scale` output units per point. The visible
/// box is the CropBox, else the MediaBox.
fn page_frame(page: &PageInfo, scale: f64) -> Result<PageFrame> {
    let visible = page.crop_box.unwrap_or(page.media_box);
    let (w, h) = (visible.width(), visible.height());
    if w <= 0.0 || h <= 0.0 {
        return Err(RenderError::InvalidDimensions {
            detail: format!("page has zero/negative size: {w}x{h}"),
        });
    }
    let (width, height) = match normalize_rotation(page.rotate) {
        90 | 270 => (h * scale, w * scale),
        _ => (w * scale, h * scale),
    };
    Ok(PageFrame {
        width,
        height,
        transform: compute_page_transform(&visible, scale, page.rotate),
    })
}

/// A pixmap device of `page`'s rotated size at `options.dpi`, with the page drawn on it.
fn render_to_device(
    doc: &PdfDocument,
    page: &PageInfo,
    options: &RenderOptions,
) -> Result<PixmapDevice> {
    let frame = page_frame(page, options.dpi / 72.0)?;
    let pixel_width = frame.width.ceil() as u32;
    let pixel_height = frame.height.ceil() as u32;

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

    let mut interpreter = RenderInterpreter::new(doc, &mut device, frame.transform);
    interpreter.render_page(page)?;
    Ok(device)
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
        .ok_or(RenderError::Core(JustPdfError::PageOutOfRange {
            index: page_index,
            count: pages.len(),
        }))?
        .clone();

    let device = render_to_device(doc, &page, options)?;
    let (width, height) = device.dimensions();
    Ok(RenderedPixmap {
        data: device.raw_rgba().to_vec(),
        width,
        height,
    })
}

/// Render a single page of a PDF document to SVG string.
///
/// `page_index` is 0-based. Returns a complete SVG XML document.
pub fn render_page_to_svg(doc: &PdfDocument, page_index: usize) -> Result<String> {
    let pages = collect_pages(doc)?;
    let page = pages
        .get(page_index)
        .ok_or(RenderError::Core(JustPdfError::PageOutOfRange {
            index: page_index,
            count: pages.len(),
        }))?
        .clone();

    // 1pt = 1 SVG unit
    let frame = page_frame(&page, 1.0)?;
    let renderer = SvgRenderer::new(doc, frame.transform, frame.width, frame.height);
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
                .ok_or(RenderError::Core(JustPdfError::PageOutOfRange {
                    index: idx,
                    count: pages.len(),
                }))?;
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

/// `/Rotate` reduced to 0..360; negative values count counterclockwise.
fn normalize_rotation(rotate: i64) -> i64 {
    rotate.rem_euclid(360)
}

/// Compute the transform from PDF user space to device (pixel) space.
///
/// `media_box` is the visible box, mapped onto the output with its top-left
/// corner at the origin after turning it `rotate` degrees clockwise. For 90 and
/// 270 the output is `media_box.height()` wide and `media_box.width()` tall.
/// Rotations other than multiples of 90 are treated as 0.
pub fn compute_page_transform(
    media_box: &justpdf_core::page::Rect,
    scale: f64,
    rotate: i64,
) -> Matrix {
    let (llx, lly, urx, ury) = (media_box.llx, media_box.lly, media_box.urx, media_box.ury);
    let s = scale;

    // Device x right, y down; each case maps the box corner that ends up
    // top-left to (0, 0).
    match normalize_rotation(rotate) {
        // x_dev = (y - lly), y_dev = (x - llx)
        90 => Matrix {
            a: 0.0,
            b: s,
            c: s,
            d: 0.0,
            e: -lly * s,
            f: -llx * s,
        },
        // x_dev = (urx - x), y_dev = (y - lly)
        180 => Matrix {
            a: -s,
            b: 0.0,
            c: 0.0,
            d: s,
            e: urx * s,
            f: -lly * s,
        },
        // x_dev = (ury - y), y_dev = (urx - x)
        270 => Matrix {
            a: 0.0,
            b: -s,
            c: -s,
            d: 0.0,
            e: ury * s,
            f: urx * s,
        },
        // x_dev = (x - llx), y_dev = (ury - y)
        _ => Matrix {
            a: s,
            b: 0.0,
            c: 0.0,
            d: -s,
            e: -llx * s,
            f: ury * s,
        },
    }
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
        assert!(
            matches!(
                results[0],
                Err(RenderError::Core(JustPdfError::PageOutOfRange {
                    index: 9999,
                    ..
                }))
            ),
            "{:?}",
            results[0]
        );
    }

    #[cfg(feature = "parallel")]
    #[test]
    fn test_parallel_render_single_page() {
        use std::path::Path;
        let pdf_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../testpdf.pdf");
        if !pdf_path.exists() {
            eprintln!("skipping: testpdf.pdf not found");
            return;
        }
        let doc = justpdf_core::PdfDocument::open(&pdf_path).expect("failed to open PDF");
        let opts = RenderOptions::default();
        // Parallel rendering with a single page should work fine.
        let results = render_pages_parallel(&doc, &[0], &opts);
        assert_eq!(results.len(), 1);
        assert!(
            results[0].is_ok(),
            "single-page parallel render failed: {:?}",
            results[0].as_ref().err()
        );
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
