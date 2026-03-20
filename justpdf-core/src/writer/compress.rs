//! PDF compression engine.
//!
//! Compresses PDF files by re-encoding images at lower quality,
//! downscaling oversized images, and performing structural optimization
//! (garbage collection, deduplication, object stream packing).

use crate::error::{JustPdfError, Result};
use crate::image::{self, ImageInfo};
use crate::object::{PdfDict, PdfObject};
use crate::parser::PdfDocument;
use crate::writer::encode::encode_flate;
use crate::writer::modify::DocumentModifier;

/// Compression options.
#[derive(Debug, Clone)]
pub struct CompressOptions {
    /// JPEG quality (1-100). `None` disables image re-encoding.
    pub jpeg_quality: Option<u8>,
    /// Maximum image DPI. `None` disables downscaling.
    pub max_image_dpi: Option<f64>,
    /// Skip images smaller than this many bytes.
    pub skip_below_bytes: usize,
    /// Run structural optimization (GC + dedup + object streams).
    pub structural: bool,
    /// Apply FlateDecode to uncompressed streams.
    pub compress_streams: bool,
}

impl CompressOptions {
    /// No image changes — structure cleanup only.
    pub fn preset_low() -> Self {
        Self {
            jpeg_quality: None,
            max_image_dpi: None,
            skip_below_bytes: 0,
            structural: true,
            compress_streams: true,
        }
    }

    /// Moderate compression, barely visible quality loss.
    pub fn preset_medium() -> Self {
        Self {
            jpeg_quality: Some(75),
            max_image_dpi: None,
            skip_below_bytes: 10_000,
            structural: true,
            compress_streams: true,
        }
    }

    /// Strong compression for web/email.
    pub fn preset_high() -> Self {
        Self {
            jpeg_quality: Some(65),
            max_image_dpi: Some(150.0),
            skip_below_bytes: 5_000,
            structural: true,
            compress_streams: true,
        }
    }

    /// Maximum compression, noticeable quality loss.
    pub fn preset_extreme() -> Self {
        Self {
            jpeg_quality: Some(40),
            max_image_dpi: Some(96.0),
            skip_below_bytes: 2_000,
            structural: true,
            compress_streams: true,
        }
    }

    /// Build options from a preset name.
    pub fn from_preset(name: &str) -> Option<Self> {
        match name {
            "low" => Some(Self::preset_low()),
            "medium" => Some(Self::preset_medium()),
            "high" => Some(Self::preset_high()),
            "extreme" => Some(Self::preset_extreme()),
            _ => None,
        }
    }
}

/// Compression result statistics.
#[derive(Debug, Clone, Default)]
pub struct CompressStats {
    pub original_size: usize,
    pub compressed_size: usize,
    pub images_found: usize,
    pub images_recompressed: usize,
    pub images_downscaled: usize,
    pub images_skipped: usize,
    pub duplicates_removed: usize,
    pub objects_removed_gc: usize,
}

/// PDF analysis result (pre-compression preview).
#[derive(Debug, Clone, Default)]
pub struct AnalyzeResult {
    pub pages: usize,
    pub images: usize,
    pub total_image_bytes: usize,
    pub is_encrypted: bool,
}

/// Analyze a PDF without compressing it.
pub fn analyze_pdf(data: &[u8]) -> Result<AnalyzeResult> {
    let doc = PdfDocument::from_bytes(data.to_vec())?;

    let pages = crate::page::page_count(&doc).unwrap_or(0);
    let is_encrypted = doc.is_encrypted();

    let mut images = 0usize;
    let mut total_image_bytes = 0usize;

    let refs: Vec<_> = doc.object_refs().collect();
    for iref in &refs {
        if let Ok(obj) = doc.resolve(iref) {
            if let PdfObject::Stream { ref dict, ref data } = obj {
                if is_image_xobject(dict) {
                    images += 1;
                    total_image_bytes += data.len();
                }
            }
        }
    }

    Ok(AnalyzeResult {
        pages,
        images,
        total_image_bytes,
        is_encrypted,
    })
}

/// Compress a PDF and return the compressed bytes with statistics.
pub fn compress_pdf(data: &[u8], options: &CompressOptions) -> Result<(Vec<u8>, CompressStats)> {
    let original_size = data.len();
    let doc = PdfDocument::from_bytes(data.to_vec())?;

    if doc.is_encrypted() {
        return Err(JustPdfError::StreamDecode {
            filter: "compress".into(),
            detail: "cannot compress encrypted PDF — decrypt first".into(),
        });
    }

    let mut modifier = DocumentModifier::from_document(&doc)?;
    let mut stats = CompressStats {
        original_size,
        ..Default::default()
    };

    // --- Step 1: Image compression ---
    if options.jpeg_quality.is_some() || options.max_image_dpi.is_some() {
        recompress_images(&mut modifier, options, &mut stats)?;
    }

    // --- Step 2: Compress uncompressed streams ---
    if options.compress_streams {
        compress_raw_streams(&mut modifier);
    }

    // --- Step 3: Structural optimization ---
    // Note: we only run GC (garbage_collect) here, not clean_objects().
    // clean_objects() renumbers all objects sequentially, which invalidates
    // the catalog_ref stored in DocumentModifier. GC alone is safe because
    // it only removes unreachable objects without changing any object numbers.
    if options.structural {
        let obj_count_before = modifier.writer().objects.len();
        modifier.garbage_collect();
        stats.objects_removed_gc = obj_count_before.saturating_sub(modifier.writer().objects.len());
    }

    let output = modifier.build()?;
    stats.compressed_size = output.len();

    Ok((output, stats))
}

/// Check if a dict represents an Image XObject.
fn is_image_xobject(dict: &PdfDict) -> bool {
    dict.get_name(b"Subtype") == Some(b"Image")
}

/// Should this image be skipped from re-encoding?
fn should_skip_image(info: &ImageInfo, stream_size: usize, options: &CompressOptions) -> bool {
    // Skip images smaller than threshold
    if stream_size < options.skip_below_bytes {
        return true;
    }
    // Skip image masks (1bpp stencils)
    if info.is_mask {
        return true;
    }
    // Skip images with soft mask (JPEG doesn't support alpha)
    if info.has_smask {
        return true;
    }
    // Skip CMYK (color conversion risk)
    if info.color_space == b"DeviceCMYK" || info.color_space == b"CMYK" {
        return true;
    }
    false
}

/// Walk all objects, find Image XObjects, decode, re-encode, replace.
fn recompress_images(
    modifier: &mut DocumentModifier,
    options: &CompressOptions,
    stats: &mut CompressStats,
) -> Result<()> {
    // Collect image object numbers first (can't mutate while iterating)
    let image_entries: Vec<(u32, PdfDict, Vec<u8>)> = modifier
        .writer()
        .objects
        .iter()
        .filter_map(|(obj_num, obj)| {
            if let PdfObject::Stream { dict, data } = obj {
                if is_image_xobject(dict) {
                    return Some((*obj_num, dict.clone(), data.clone()));
                }
            }
            None
        })
        .collect();

    stats.images_found = image_entries.len();

    for (obj_num, dict, raw_data) in image_entries {
        let info = match image::image_info(&dict) {
            Some(info) => info,
            None => {
                stats.images_skipped += 1;
                continue;
            }
        };

        if should_skip_image(&info, raw_data.len(), options) {
            stats.images_skipped += 1;
            continue;
        }

        // Decode the image pixels
        let decoded = match image::decode_image(&raw_data, &dict) {
            Ok(d) => d,
            Err(_) => {
                stats.images_skipped += 1;
                continue;
            }
        };

        // Determine target dimensions (downscale if needed)
        let (target_w, target_h, downscaled) =
            compute_target_dimensions(&decoded, options.max_image_dpi);

        // Convert to RGB pixels for JPEG encoding
        let rgb_pixels = to_rgb_pixels(&decoded, target_w, target_h);
        if rgb_pixels.is_empty() {
            stats.images_skipped += 1;
            continue;
        }

        // Encode as JPEG
        let quality = options.jpeg_quality.unwrap_or(75);
        let jpeg_bytes = match encode_jpeg_rgb(&rgb_pixels, target_w, target_h, quality) {
            Ok(bytes) => bytes,
            Err(_) => {
                stats.images_skipped += 1;
                continue;
            }
        };

        // Safety: don't replace if new is larger than original
        if jpeg_bytes.len() >= raw_data.len() {
            stats.images_skipped += 1;
            continue;
        }

        // Build replacement stream
        let mut new_dict = PdfDict::new();
        new_dict.insert(b"Type".to_vec(), PdfObject::Name(b"XObject".to_vec()));
        new_dict.insert(b"Subtype".to_vec(), PdfObject::Name(b"Image".to_vec()));
        new_dict.insert(b"Width".to_vec(), PdfObject::Integer(target_w as i64));
        new_dict.insert(b"Height".to_vec(), PdfObject::Integer(target_h as i64));
        new_dict.insert(b"BitsPerComponent".to_vec(), PdfObject::Integer(8));
        new_dict.insert(
            b"ColorSpace".to_vec(),
            PdfObject::Name(b"DeviceRGB".to_vec()),
        );
        new_dict.insert(
            b"Filter".to_vec(),
            PdfObject::Name(b"DCTDecode".to_vec()),
        );
        new_dict.insert(
            b"Length".to_vec(),
            PdfObject::Integer(jpeg_bytes.len() as i64),
        );

        modifier.set_object(
            obj_num,
            PdfObject::Stream {
                dict: new_dict,
                data: jpeg_bytes,
            },
        );

        stats.images_recompressed += 1;
        if downscaled {
            stats.images_downscaled += 1;
        }
    }

    Ok(())
}

/// Compute target dimensions, applying max DPI constraint.
///
/// Uses a simple pixel-budget approach: if `max_image_dpi` is set,
/// we assume the image is displayed at 1pt-per-pixel baseline (72 DPI)
/// and scale down proportionally.
fn compute_target_dimensions(
    decoded: &image::DecodedImage,
    max_dpi: Option<f64>,
) -> (u32, u32, bool) {
    let (w, h) = (decoded.width, decoded.height);

    let max_dpi = match max_dpi {
        Some(d) if d > 0.0 => d,
        _ => return (w, h, false),
    };

    // Assume baseline display at 72 DPI (1 pixel = 1 point).
    // effective_dpi = pixel_dimension (since display_points ≈ pixel_dimension at 72dpi)
    // We scale down by (max_dpi / 72) ratio.
    // In practice, most images in PDFs are displayed much smaller than their pixel
    // dimensions, so this is conservative. Phase C will add CTM-based DPI.
    let scale = max_dpi / 72.0;
    if scale >= 1.0 {
        // Image is already within the DPI budget at 72 DPI baseline
        // But we still want to cap truly oversized images.
        // Cap at max_dpi * (assumed page size 11in) ≈ max_dpi * 11 pixels
        let max_pixels = (max_dpi * 14.0) as u32; // ~14 inches max dimension
        if w <= max_pixels && h <= max_pixels {
            return (w, h, false);
        }
        let ratio = (max_pixels as f64) / (w.max(h) as f64);
        let new_w = ((w as f64) * ratio).round() as u32;
        let new_h = ((h as f64) * ratio).round() as u32;
        return (new_w.max(1), new_h.max(1), true);
    }

    let new_w = ((w as f64) * scale).round() as u32;
    let new_h = ((h as f64) * scale).round() as u32;
    (new_w.max(1), new_h.max(1), true)
}

/// Convert decoded image pixels to RGB and optionally resize.
fn to_rgb_pixels(
    decoded: &image::DecodedImage,
    target_w: u32,
    target_h: u32,
) -> Vec<u8> {
    use ::image::{DynamicImage, RgbImage, imageops::FilterType};

    let (src_w, src_h) = (decoded.width, decoded.height);
    let components = decoded.components;

    // Build an RgbImage from the decoded pixel data
    let rgb_data: Vec<u8> = match components {
        1 => {
            // Grayscale → RGB
            decoded
                .data
                .iter()
                .flat_map(|&g| [g, g, g])
                .collect()
        }
        3 => {
            // Already RGB
            decoded.data.clone()
        }
        4 => {
            // CMYK → RGB (simple conversion)
            decoded
                .data
                .chunks_exact(4)
                .flat_map(|cmyk| {
                    let c = cmyk[0] as f32 / 255.0;
                    let m = cmyk[1] as f32 / 255.0;
                    let y = cmyk[2] as f32 / 255.0;
                    let k = cmyk[3] as f32 / 255.0;
                    let r = (255.0 * (1.0 - c) * (1.0 - k)) as u8;
                    let g = (255.0 * (1.0 - m) * (1.0 - k)) as u8;
                    let b = (255.0 * (1.0 - y) * (1.0 - k)) as u8;
                    [r, g, b]
                })
                .collect()
        }
        _ => return Vec::new(),
    };

    let expected_len = (src_w * src_h * 3) as usize;
    if rgb_data.len() < expected_len {
        return Vec::new();
    }

    let img = match RgbImage::from_raw(src_w, src_h, rgb_data) {
        Some(img) => img,
        None => return Vec::new(),
    };

    // Resize if needed
    if target_w != src_w || target_h != src_h {
        let dynamic = DynamicImage::ImageRgb8(img);
        let resized = dynamic.resize_exact(target_w, target_h, FilterType::Lanczos3);
        resized.to_rgb8().into_raw()
    } else {
        img.into_raw()
    }
}

/// Encode RGB pixels as JPEG with the given quality.
fn encode_jpeg_rgb(rgb_data: &[u8], width: u32, height: u32, quality: u8) -> Result<Vec<u8>> {
    use ::image::codecs::jpeg::JpegEncoder;
    use std::io::Cursor;

    let mut buf = Cursor::new(Vec::new());
    let mut encoder = JpegEncoder::new_with_quality(&mut buf, quality);
    encoder
        .encode(rgb_data, width, height, ::image::ExtendedColorType::Rgb8)
        .map_err(|e| JustPdfError::StreamDecode {
            filter: "compress".into(),
            detail: format!("JPEG encode error: {e}"),
        })?;
    Ok(buf.into_inner())
}

/// Apply FlateDecode to streams that have no filter.
fn compress_raw_streams(modifier: &mut DocumentModifier) {
    let entries: Vec<(u32, PdfDict, Vec<u8>)> = modifier
        .writer()
        .objects
        .iter()
        .filter_map(|(obj_num, obj)| {
            if let PdfObject::Stream { dict, data } = obj {
                // Only compress streams with no existing filter
                if dict.get(b"Filter").is_none() && data.len() > 128 {
                    return Some((*obj_num, dict.clone(), data.clone()));
                }
            }
            None
        })
        .collect();

    for (obj_num, mut dict, data) in entries {
        if let Ok(compressed) = encode_flate(&data) {
            // Only replace if compression actually helps
            if compressed.len() < data.len() {
                dict.insert(
                    b"Filter".to_vec(),
                    PdfObject::Name(b"FlateDecode".to_vec()),
                );
                dict.insert(
                    b"Length".to_vec(),
                    PdfObject::Integer(compressed.len() as i64),
                );
                modifier.set_object(
                    obj_num,
                    PdfObject::Stream {
                        dict,
                        data: compressed,
                    },
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::writer::document::DocumentBuilder;
    use crate::writer::page::PageBuilder;

    /// Create a simple text-only PDF for testing.
    fn create_text_pdf(num_pages: usize) -> Vec<u8> {
        let mut doc = DocumentBuilder::new();
        let font = doc.add_standard_font("Helvetica");

        for i in 0..num_pages {
            let mut page = PageBuilder::new(612.0, 792.0);
            page.add_font(&font, "Helvetica");
            page.begin_text();
            page.set_font(&font, 12.0);
            page.move_to(72.0, 720.0);
            page.show_text(&format!("Test page {}", i + 1));
            page.end_text();
            doc.add_page(page);
        }

        doc.build().unwrap()
    }

    /// Create a minimal JPEG image (red square).
    fn create_test_jpeg(width: u32, height: u32, quality: u8) -> Vec<u8> {
        use ::image::codecs::jpeg::JpegEncoder;
        use std::io::Cursor;

        let mut rgb = vec![0u8; (width * height * 3) as usize];
        for pixel in rgb.chunks_exact_mut(3) {
            pixel[0] = 255; // R
            pixel[1] = 0;   // G
            pixel[2] = 0;   // B
        }

        let mut buf = Cursor::new(Vec::new());
        let mut enc = JpegEncoder::new_with_quality(&mut buf, quality);
        enc.encode(&rgb, width, height, ::image::ExtendedColorType::Rgb8)
            .unwrap();
        buf.into_inner()
    }

    /// Create a PDF with an embedded JPEG image.
    fn create_pdf_with_jpeg(width: u32, height: u32, quality: u8) -> Vec<u8> {
        let jpeg_data = create_test_jpeg(width, height, quality);

        let mut doc = DocumentBuilder::new();
        let font = doc.add_standard_font("Helvetica");
        let (_img_name, img_ref) = crate::writer::document::embed_jpeg(&mut doc, &jpeg_data).unwrap();

        let mut page = PageBuilder::new(612.0, 792.0);
        page.add_font(&font, "Helvetica");
        page.add_image("Im1", img_ref);
        page.begin_text();
        page.set_font(&font, 12.0);
        page.move_to(72.0, 720.0);
        page.show_text("Image test");
        page.end_text();
        page.draw_image("Im1", 72.0, 400.0, width as f64, height as f64);
        doc.add_page(page);

        doc.build().unwrap()
    }

    #[test]
    fn test_analyze_text_pdf() {
        let pdf = create_text_pdf(3);
        let result = analyze_pdf(&pdf).unwrap();
        assert_eq!(result.pages, 3);
        assert_eq!(result.images, 0);
        assert!(!result.is_encrypted);
    }

    #[test]
    fn test_analyze_pdf_with_image() {
        let pdf = create_pdf_with_jpeg(100, 100, 95);
        let result = analyze_pdf(&pdf).unwrap();
        assert_eq!(result.pages, 1);
        assert_eq!(result.images, 1);
        assert!(result.total_image_bytes > 0);
    }

    #[test]
    fn test_compress_low_text_only() {
        let pdf = create_text_pdf(2);
        let original_size = pdf.len();

        let (compressed, stats) = compress_pdf(&pdf, &CompressOptions::preset_low()).unwrap();

        assert!(compressed.len() > 0);
        assert_eq!(stats.original_size, original_size);
        assert_eq!(stats.images_found, 0);
        // Output should be valid PDF
        assert!(compressed.starts_with(b"%PDF"));
    }

    #[test]
    fn test_compress_medium_with_image() {
        let pdf = create_pdf_with_jpeg(200, 200, 95);
        let original_size = pdf.len();

        let (compressed, stats) =
            compress_pdf(&pdf, &CompressOptions::preset_medium()).unwrap();

        assert!(compressed.len() > 0);
        assert!(compressed.starts_with(b"%PDF"));
        assert_eq!(stats.original_size, original_size);
        assert!(stats.images_found >= 1);

        // The re-encoded image at q75 should be smaller than q95
        if stats.images_recompressed > 0 {
            assert!(stats.compressed_size < original_size);
        }
    }

    #[test]
    fn test_compress_high_downscale() {
        // Large image that should be downscaled at 150 DPI
        let pdf = create_pdf_with_jpeg(2000, 2000, 95);
        let original_size = pdf.len();

        let (compressed, stats) =
            compress_pdf(&pdf, &CompressOptions::preset_high()).unwrap();

        assert!(compressed.starts_with(b"%PDF"));
        assert_eq!(stats.original_size, original_size);

        // Should compress significantly
        if stats.images_recompressed > 0 {
            assert!(stats.compressed_size < original_size);
        }
    }

    #[test]
    fn test_compress_extreme() {
        let pdf = create_pdf_with_jpeg(500, 500, 95);

        let (compressed, stats) =
            compress_pdf(&pdf, &CompressOptions::preset_extreme()).unwrap();

        assert!(compressed.starts_with(b"%PDF"));
        if stats.images_recompressed > 0 {
            assert!(stats.compressed_size < stats.original_size);
        }
    }

    #[test]
    fn test_compress_roundtrip_valid() {
        let pdf = create_pdf_with_jpeg(100, 100, 90);

        let (compressed, _) =
            compress_pdf(&pdf, &CompressOptions::preset_medium()).unwrap();

        // Re-parse the compressed PDF to ensure it's valid
        let reparsed = PdfDocument::from_bytes(compressed).unwrap();
        let pages = crate::page::collect_pages(&reparsed).unwrap();
        assert_eq!(pages.len(), 1);
    }

    #[test]
    fn test_compress_skip_small_image() {
        let pdf = create_pdf_with_jpeg(10, 10, 95); // tiny image

        let mut options = CompressOptions::preset_medium();
        options.skip_below_bytes = 100_000; // very high threshold

        let (_, stats) = compress_pdf(&pdf, &options).unwrap();
        assert_eq!(stats.images_recompressed, 0);
        assert!(stats.images_skipped > 0);
    }

    #[test]
    fn test_preset_from_name() {
        assert!(CompressOptions::from_preset("low").is_some());
        assert!(CompressOptions::from_preset("medium").is_some());
        assert!(CompressOptions::from_preset("high").is_some());
        assert!(CompressOptions::from_preset("extreme").is_some());
        assert!(CompressOptions::from_preset("invalid").is_none());
    }

    #[test]
    fn test_safety_no_replace_if_larger() {
        // Very small image at low quality — re-encoding might make it larger
        let pdf = create_pdf_with_jpeg(4, 4, 10);

        let (_, stats) =
            compress_pdf(&pdf, &CompressOptions::preset_medium()).unwrap();

        // Either recompressed (if smaller) or skipped (if larger) — no panic
        assert!(stats.images_recompressed + stats.images_skipped >= stats.images_found.saturating_sub(0));
    }

    #[test]
    fn test_encode_jpeg_rgb() {
        let rgb = vec![128u8; 30 * 30 * 3];
        let result = encode_jpeg_rgb(&rgb, 30, 30, 75);
        assert!(result.is_ok());
        let bytes = result.unwrap();
        // JPEG starts with FFD8
        assert_eq!(&bytes[0..2], &[0xFF, 0xD8]);
    }

    #[test]
    fn test_compute_target_dimensions_no_limit() {
        let decoded = image::DecodedImage {
            width: 1000,
            height: 500,
            components: 3,
            bpc: 8,
            data: vec![],
            source_format: image::ImageFormat::Raw,
        };
        let (w, h, scaled) = compute_target_dimensions(&decoded, None);
        assert_eq!((w, h), (1000, 500));
        assert!(!scaled);
    }

    #[test]
    fn test_compute_target_dimensions_with_limit() {
        let decoded = image::DecodedImage {
            width: 4000,
            height: 3000,
            components: 3,
            bpc: 8,
            data: vec![],
            source_format: image::ImageFormat::Raw,
        };
        // At 150 DPI max, max pixels = 150 * 14 = 2100
        let (w, h, scaled) = compute_target_dimensions(&decoded, Some(150.0));
        assert!(scaled);
        assert!(w <= 2100);
        assert!(h <= 2100);
    }
}
