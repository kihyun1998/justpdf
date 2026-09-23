//! PDF compression engine.
//!
//! Compresses PDF files by re-encoding images at lower quality,
//! downscaling oversized images, and performing structural optimization
//! (garbage collection, deduplication, object stream packing).

use std::collections::{BTreeSet, HashMap, HashSet};

use sha2::{Digest, Sha256};

use crate::content::parse_content_stream;
use crate::error::{JustPdfError, Result};
use crate::font::subset::subset_font;
use crate::image::{self, ImageInfo};
use crate::object::{PdfDict, PdfObject};
use crate::parser::PdfDocument;
use crate::writer::clean::rewrite_references;
use crate::writer::encode::{encode_flate, encode_flate_best};
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
    /// Run structural optimization (GC + stream dedup).
    pub structural: bool,
    /// Apply FlateDecode to uncompressed streams.
    pub compress_streams: bool,
    /// Enable font subsetting (remove unused glyphs from embedded TrueType fonts).
    pub font_subsetting: bool,
    /// Remove unused resources (fonts, images, ExtGState) from page Resources.
    pub remove_unused_resources: bool,
    /// Remove metadata (XMP, StructTreeRoot, thumbnails, OutputIntents, etc).
    pub strip_metadata: bool,
    /// Remove embedded files, JavaScript, and other non-essential data.
    pub strip_extras: bool,
    /// Convert color images to grayscale (extreme preset option).
    pub grayscale: bool,
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
            font_subsetting: false,
            remove_unused_resources: false,
            strip_metadata: false,
            strip_extras: false,
            grayscale: false,
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
            font_subsetting: true,
            remove_unused_resources: true,
            strip_metadata: false,
            strip_extras: false,
            grayscale: false,
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
            font_subsetting: true,
            remove_unused_resources: true,
            strip_metadata: true,
            strip_extras: false,
            grayscale: false,
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
            font_subsetting: true,
            remove_unused_resources: true,
            strip_metadata: true,
            strip_extras: true,
            grayscale: false,
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
    pub streams_recompressed: usize,
    pub fonts_subsetted: usize,
    pub unused_resources_removed: usize,
    pub metadata_items_stripped: usize,
    pub images_grayscaled: usize,
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

    // --- Step 0.5: Collect image display sizes from CTM ---
    let image_display_sizes = if options.max_image_dpi.is_some() {
        collect_image_display_sizes(&mut modifier)
    } else {
        HashMap::new()
    };

    // --- Step 1: Image compression ---
    if options.jpeg_quality.is_some() || options.max_image_dpi.is_some() {
        recompress_images(&mut modifier, options, &mut stats, &image_display_sizes)?;
    }

    // --- Step 1.5: Grayscale conversion ---
    if options.grayscale {
        convert_images_to_grayscale(&mut modifier, &mut stats);
        rewrite_color_operators_to_gray(&mut modifier);
    }

    // --- Step 1.6: Font subsetting ---
    if options.font_subsetting {
        subset_embedded_fonts(&mut modifier, &mut stats);
    }

    // --- Step 2: Compress uncompressed streams ---
    if options.compress_streams {
        compress_raw_streams(&mut modifier);
    }

    // --- Step 2.5: Recompress FlateDecode streams at best level ---
    if options.compress_streams {
        recompress_flate_streams(&mut modifier, &mut stats);
    }

    // --- Step 2.7: Remove unused resources from page Resources ---
    if options.remove_unused_resources {
        remove_unused_resources(&mut modifier, &mut stats);
    }

    // --- Step 2.8: Strip metadata and non-essential data ---
    if options.strip_metadata || options.strip_extras {
        strip_non_essential(&mut modifier, options, &mut stats);
    }

    // --- Step 3: Dedup identical streams ---
    if options.structural {
        dedup_streams(&mut modifier, &mut stats);
    }

    // --- Step 4: Structural optimization (GC) ---
    // Note: we only run GC (garbage_collect) here, not clean_objects().
    // clean_objects() renumbers all objects sequentially, which invalidates
    // the catalog_ref stored in DocumentModifier. GC alone is safe because
    // it only removes unreachable objects without changing any object numbers.
    if options.structural {
        let obj_count_before = modifier.writer().objects.len();
        modifier.garbage_collect();
        stats.objects_removed_gc = obj_count_before.saturating_sub(modifier.writer().objects.len());
    }

    // --- Step 5: Object stream packing ---
    // Disabled: xref stream implementation has compatibility issues with some
    // PDF viewers. Needs further testing before re-enabling.
    // if options.structural {
    //     let compressed_info = pack_into_object_streams(&mut modifier);
    //     if !compressed_info.is_empty() {
    //         let output = modifier.build_with_xref_stream(&compressed_info)?;
    //         stats.compressed_size = output.len();
    //         return Ok((output, stats));
    //     }
    // }

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
    image_display_sizes: &HashMap<u32, (f64, f64)>,
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
        let display_size = image_display_sizes.get(&obj_num).copied();
        let (target_w, target_h, downscaled) =
            compute_target_dimensions_with_ctm(&decoded, options.max_image_dpi, display_size);

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
/// If `display_size` is provided (from CTM analysis), calculates actual DPI.
/// Otherwise falls back to pixel-budget heuristic.
#[allow(dead_code)]
fn compute_target_dimensions(
    decoded: &image::DecodedImage,
    max_dpi: Option<f64>,
) -> (u32, u32, bool) {
    compute_target_dimensions_with_ctm(decoded, max_dpi, None)
}

/// Compute target dimensions with optional CTM-derived display size.
///
/// `display_size` is `(width_points, height_points)` from the content stream CTM,
/// representing the actual display dimensions on the page.
fn compute_target_dimensions_with_ctm(
    decoded: &image::DecodedImage,
    max_dpi: Option<f64>,
    display_size: Option<(f64, f64)>,
) -> (u32, u32, bool) {
    let (w, h) = (decoded.width, decoded.height);

    let max_dpi = match max_dpi {
        Some(d) if d > 0.0 => d,
        _ => return (w, h, false),
    };

    // If we have CTM-derived display size, calculate actual DPI
    if let Some((disp_w, disp_h)) = display_size {
        if disp_w > 0.0 && disp_h > 0.0 {
            // DPI = pixels / (display_points / 72)
            let dpi_x = w as f64 / (disp_w / 72.0);
            let dpi_y = h as f64 / (disp_h / 72.0);
            let effective_dpi = dpi_x.max(dpi_y);

            if effective_dpi <= max_dpi {
                return (w, h, false); // already within budget
            }

            // Scale down to target DPI
            let scale = max_dpi / effective_dpi;
            let new_w = ((w as f64) * scale).round() as u32;
            let new_h = ((h as f64) * scale).round() as u32;
            return (new_w.max(1), new_h.max(1), true);
        }
    }

    // Fallback: pixel-budget heuristic (assumes 72 DPI baseline)
    let scale = max_dpi / 72.0;
    if scale >= 1.0 {
        let max_pixels = (max_dpi * 14.0) as u32;
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

/// Collect image display sizes from content streams using CTM analysis.
///
/// Returns a map from image XObject name → (display_width_pt, display_height_pt).
/// Parses `cm` operators to track the current transformation matrix, and extracts
/// display dimensions when `Do` is invoked on an image.
fn collect_image_display_sizes(
    modifier: &mut DocumentModifier,
) -> HashMap<u32, (f64, f64)> {
    let mut result: HashMap<u32, (f64, f64)> = HashMap::new();

    // Collect page data
    let mut pages: Vec<(Vec<u32>, HashMap<Vec<u8>, u32>)> = Vec::new();

    let page_raw: Vec<(Vec<u32>, PdfDict)> = modifier
        .writer()
        .objects
        .iter()
        .filter_map(|(_, obj)| {
            if let PdfObject::Dict(dict) = obj {
                if dict.get_name(b"Type") != Some(b"Page") {
                    return None;
                }
                let content_obj_nums = match dict.get(b"Contents") {
                    Some(PdfObject::Reference(r)) => vec![r.obj_num],
                    Some(PdfObject::Array(arr)) => arr.iter().filter_map(|o| {
                        if let PdfObject::Reference(r) = o { Some(r.obj_num) } else { None }
                    }).collect(),
                    _ => return None,
                };
                Some((content_obj_nums, dict.clone()))
            } else {
                None
            }
        })
        .collect();

    for (content_obj_nums, page_dict) in page_raw {
        // Get XObject resources: name → obj_num
        let xobject_map = extract_xobject_map(&page_dict, modifier);
        pages.push((content_obj_nums, xobject_map));
    }

    for (content_obj_nums, xobject_map) in &pages {
        let mut stream_data = Vec::new();
        for &obj_num in content_obj_nums {
            if let Some(data) = get_stream_decoded_data(obj_num, modifier) {
                stream_data.extend_from_slice(&data);
                stream_data.push(b'\n');
            }
        }

        if stream_data.is_empty() {
            continue;
        }

        let ops = match parse_content_stream(&stream_data) {
            Ok(ops) => ops,
            Err(_) => continue,
        };

        // Track CTM with graphics state stack
        // CTM is [a b c d e f] where the image unit square maps to:
        // width = sqrt(a^2 + c^2), height = sqrt(b^2 + d^2)
        let mut ctm: [f64; 6] = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0]; // identity
        let mut ctm_stack: Vec<[f64; 6]> = Vec::new();

        for op in &ops {
            match op.operator.as_slice() {
                b"q" => {
                    ctm_stack.push(ctm);
                }
                b"Q" => {
                    if let Some(saved) = ctm_stack.pop() {
                        ctm = saved;
                    }
                }
                b"cm" => {
                    if op.operands.len() >= 6 {
                        let vals: Vec<f64> = op.operands.iter().take(6).map(|o| {
                            match o {
                                crate::content::Operand::Real(v) => *v,
                                crate::content::Operand::Integer(v) => *v as f64,
                                _ => 0.0,
                            }
                        }).collect();
                        // Multiply: CTM = new_matrix * current_CTM
                        let new = [vals[0], vals[1], vals[2], vals[3], vals[4], vals[5]];
                        ctm = multiply_matrix(&new, &ctm);
                    }
                }
                b"Do" => {
                    if let Some(name) = op.operands.first().and_then(|o| o.as_name()) {
                        if let Some(&obj_num) = xobject_map.get(name) {
                            // Extract display size from CTM
                            // For images, Do maps the unit square [0,0]-[1,1] through CTM
                            let display_w = (ctm[0] * ctm[0] + ctm[2] * ctm[2]).sqrt();
                            let display_h = (ctm[1] * ctm[1] + ctm[3] * ctm[3]).sqrt();

                            // Keep the maximum display size across all usages
                            let entry = result.entry(obj_num).or_insert((0.0, 0.0));
                            if display_w > entry.0 {
                                entry.0 = display_w;
                            }
                            if display_h > entry.1 {
                                entry.1 = display_h;
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    result
}

/// Multiply two 2D transformation matrices [a b c d e f].
fn multiply_matrix(a: &[f64; 6], b: &[f64; 6]) -> [f64; 6] {
    [
        a[0] * b[0] + a[1] * b[2],
        a[0] * b[1] + a[1] * b[3],
        a[2] * b[0] + a[3] * b[2],
        a[2] * b[1] + a[3] * b[3],
        a[4] * b[0] + a[5] * b[2] + b[4],
        a[4] * b[1] + a[5] * b[3] + b[5],
    ]
}

/// Extract XObject resource name → obj_num mapping from a Page dict.
fn extract_xobject_map(page_dict: &PdfDict, modifier: &DocumentModifier) -> HashMap<Vec<u8>, u32> {
    let mut map = HashMap::new();

    let resources = match page_dict.get(b"Resources") {
        Some(PdfObject::Dict(d)) => d.clone(),
        Some(PdfObject::Reference(r)) => match find_object_dict(r.obj_num, modifier) {
            Some(d) => d,
            None => return map,
        },
        _ => return map,
    };

    let xobject_dict = match resources.get(b"XObject") {
        Some(PdfObject::Dict(d)) => d.clone(),
        Some(PdfObject::Reference(r)) => match find_object_dict(r.obj_num, modifier) {
            Some(d) => d,
            None => return map,
        },
        _ => return map,
    };

    for (name, value) in xobject_dict.iter() {
        if let PdfObject::Reference(r) = value {
            map.insert(name.clone(), r.obj_num);
        }
    }

    map
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

/// Recompress existing FlateDecode streams at maximum compression level.
///
/// Decodes each FlateDecode stream (single filter, no DecodeParms) and
/// re-encodes with `Compression::best()`. Replaces only if the result is smaller.
fn recompress_flate_streams(modifier: &mut DocumentModifier, stats: &mut CompressStats) {
    let entries: Vec<(u32, PdfDict, Vec<u8>)> = modifier
        .writer()
        .objects
        .iter()
        .filter_map(|(obj_num, obj)| {
            if let PdfObject::Stream { dict, data } = obj {
                // Only target single FlateDecode filter with no DecodeParms
                if is_single_flate(dict) && dict.get(b"DecodeParms").is_none() {
                    // Skip image XObjects — they were already handled by recompress_images
                    if is_image_xobject(dict) {
                        return None;
                    }
                    return Some((*obj_num, dict.clone(), data.clone()));
                }
            }
            None
        })
        .collect();

    for (obj_num, mut dict, compressed_data) in entries {
        // Decode the existing FlateDecode stream
        let decoded = match crate::stream::decode_stream(&compressed_data, &dict) {
            Ok(d) => d,
            Err(_) => continue,
        };

        // Re-encode at best compression level
        let recompressed = match encode_flate_best(&decoded) {
            Ok(d) => d,
            Err(_) => continue,
        };

        // Only replace if strictly smaller
        if recompressed.len() >= compressed_data.len() {
            continue;
        }

        dict.insert(
            b"Length".to_vec(),
            PdfObject::Integer(recompressed.len() as i64),
        );

        modifier.set_object(
            obj_num,
            PdfObject::Stream {
                dict,
                data: recompressed,
            },
        );

        stats.streams_recompressed += 1;
    }
}

/// Pack small non-stream objects into object streams (PDF 1.5+).
///
/// This reduces overhead by combining many small dict/integer/etc objects
/// into compressed object stream containers.
#[allow(dead_code)]
fn pack_into_object_streams(
    modifier: &mut DocumentModifier,
) -> Vec<crate::writer::object_stream::CompressedObjInfo> {
    let catalog_obj_num = modifier.catalog_ref().obj_num;

    // Find pages root obj_num from catalog
    let pages_root_obj_num = find_object_dict(catalog_obj_num, modifier)
        .and_then(|cat| match cat.get(b"Pages") {
            Some(PdfObject::Reference(r)) => Some(r.obj_num),
            _ => None,
        });

    // Pack objects
    let objects = std::mem::take(&mut modifier.writer().objects);
    match crate::writer::object_stream::pack_object_streams(
        &objects,
        100, // max objects per stream
        catalog_obj_num,
        pages_root_obj_num,
        None, // no encryption
    ) {
        Ok(result) => {
            let compressed = result.compressed;
            modifier.writer().objects = result.objects;
            // Update next_obj_num
            let max = modifier
                .writer()
                .objects
                .iter()
                .map(|(n, _)| *n)
                .max()
                .unwrap_or(0);
            modifier.writer().next_obj_num = max + 1;
            compressed
        }
        Err(_) => {
            // Restore original on failure
            modifier.writer().objects = objects;
            Vec::new()
        }
    }
}

/// Convert RGB/CMYK images to grayscale for additional size reduction.
///
/// For RGB: each pixel (3 bytes) → 1 byte grayscale using luminance formula.
/// For CMYK: each pixel (4 bytes) → 1 byte grayscale.
/// Already-grayscale images are skipped.
fn convert_images_to_grayscale(modifier: &mut DocumentModifier, stats: &mut CompressStats) {
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

    for (obj_num, dict, raw_data) in image_entries {
        let info = match image::image_info(&dict) {
            Some(info) => info,
            None => continue,
        };

        // Skip already grayscale
        if info.color_space == b"DeviceGray" || info.num_components == 1 {
            continue;
        }

        // Skip masks and SMask
        if info.is_mask || info.has_smask {
            continue;
        }

        // Decode image pixels
        let decoded = match image::decode_image(&raw_data, &dict) {
            Ok(d) => d,
            Err(_) => continue,
        };

        // Convert to grayscale
        let gray_pixels: Vec<u8> = match decoded.components {
            3 => {
                // RGB → Gray using luminance: 0.299R + 0.587G + 0.114B
                decoded.data.chunks_exact(3).map(|rgb| {
                    let r = rgb[0] as f32;
                    let g = rgb[1] as f32;
                    let b = rgb[2] as f32;
                    (0.299 * r + 0.587 * g + 0.114 * b).round() as u8
                }).collect()
            }
            4 => {
                // CMYK → Gray
                decoded.data.chunks_exact(4).map(|cmyk| {
                    let c = cmyk[0] as f32 / 255.0;
                    let m = cmyk[1] as f32 / 255.0;
                    let y = cmyk[2] as f32 / 255.0;
                    let k = cmyk[3] as f32 / 255.0;
                    let r = (1.0 - c) * (1.0 - k);
                    let g = (1.0 - m) * (1.0 - k);
                    let b = (1.0 - y) * (1.0 - k);
                    ((0.299 * r + 0.587 * g + 0.114 * b) * 255.0).round() as u8
                }).collect()
            }
            _ => continue,
        };

        // Encode as JPEG grayscale
        let jpeg_bytes = match encode_jpeg_gray(&gray_pixels, decoded.width, decoded.height, 65) {
            Ok(bytes) => bytes,
            Err(_) => continue,
        };

        // Safety: don't replace if larger
        if jpeg_bytes.len() >= raw_data.len() {
            continue;
        }

        // Build replacement stream
        let mut new_dict = PdfDict::new();
        new_dict.insert(b"Type".to_vec(), PdfObject::Name(b"XObject".to_vec()));
        new_dict.insert(b"Subtype".to_vec(), PdfObject::Name(b"Image".to_vec()));
        new_dict.insert(b"Width".to_vec(), PdfObject::Integer(decoded.width as i64));
        new_dict.insert(b"Height".to_vec(), PdfObject::Integer(decoded.height as i64));
        new_dict.insert(b"BitsPerComponent".to_vec(), PdfObject::Integer(8));
        new_dict.insert(b"ColorSpace".to_vec(), PdfObject::Name(b"DeviceGray".to_vec()));
        new_dict.insert(b"Filter".to_vec(), PdfObject::Name(b"DCTDecode".to_vec()));
        new_dict.insert(b"Length".to_vec(), PdfObject::Integer(jpeg_bytes.len() as i64));

        modifier.set_object(
            obj_num,
            PdfObject::Stream {
                dict: new_dict,
                data: jpeg_bytes,
            },
        );

        stats.images_grayscaled += 1;
    }
}

/// Encode grayscale pixels as JPEG.
fn encode_jpeg_gray(gray_data: &[u8], width: u32, height: u32, quality: u8) -> Result<Vec<u8>> {
    use ::image::codecs::jpeg::JpegEncoder;
    use std::io::Cursor;

    let mut buf = Cursor::new(Vec::new());
    let mut encoder = JpegEncoder::new_with_quality(&mut buf, quality);
    encoder
        .encode(gray_data, width, height, ::image::ExtendedColorType::L8)
        .map_err(|e| JustPdfError::StreamDecode {
            filter: "compress".into(),
            detail: format!("JPEG grayscale encode error: {e}"),
        })?;
    Ok(buf.into_inner())
}

/// Rewrite color operators in content streams to grayscale equivalents.
///
/// Converts:
/// - `r g b rg` → `gray g` (non-stroking RGB)
/// - `r g b RG` → `gray G` (stroking RGB)
/// - `c m y k k` → `gray g` (non-stroking CMYK)
/// - `c m y k K` → `gray G` (stroking CMYK)
///
/// `sc`/`SC`/`scn`/`SCN` are left unchanged. Only page `/Contents` streams are
/// rewritten, not Form XObjects.
fn rewrite_color_operators_to_gray(modifier: &mut DocumentModifier) {
    // Find all content stream objects (pages' /Contents)
    let content_obj_nums: Vec<u32> = modifier
        .writer()
        .objects
        .iter()
        .filter_map(|(_, obj)| {
            if let PdfObject::Dict(dict) = obj {
                if dict.get_name(b"Type") == Some(b"Page") {
                    return match dict.get(b"Contents") {
                        Some(PdfObject::Reference(r)) => Some(vec![r.obj_num]),
                        Some(PdfObject::Array(arr)) => Some(
                            arr.iter()
                                .filter_map(|o| {
                                    if let PdfObject::Reference(r) = o {
                                        Some(r.obj_num)
                                    } else {
                                        None
                                    }
                                })
                                .collect(),
                        ),
                        _ => None,
                    };
                }
            }
            None
        })
        .flatten()
        .collect();

    for obj_num in content_obj_nums {
        let (dict, data) = match modifier.find_object_pub(obj_num) {
            Some(PdfObject::Stream { dict, data }) => (dict.clone(), data.clone()),
            _ => continue,
        };

        let decoded = match crate::stream::decode_stream(&data, &dict) {
            Ok(d) => d,
            Err(_) => continue,
        };

        let ops = match parse_content_stream(&decoded) {
            Ok(ops) => ops,
            Err(_) => continue,
        };

        // Check if any color operators exist
        let has_color_ops = ops.iter().any(|op| {
            matches!(
                op.operator.as_slice(),
                b"rg" | b"RG" | b"k" | b"K"
            )
        });

        if !has_color_ops {
            continue;
        }

        // Rebuild content stream with rewritten operators
        let mut new_content = Vec::new();
        for op in &ops {
            match op.operator.as_slice() {
                b"rg" if op.operands.len() >= 3 => {
                    // RGB non-stroking → grayscale
                    let gray = rgb_to_gray_from_operands(&op.operands);
                    write_real(&mut new_content, gray);
                    new_content.extend_from_slice(b" g\n");
                }
                b"RG" if op.operands.len() >= 3 => {
                    // RGB stroking → grayscale
                    let gray = rgb_to_gray_from_operands(&op.operands);
                    write_real(&mut new_content, gray);
                    new_content.extend_from_slice(b" G\n");
                }
                b"k" if op.operands.len() >= 4 => {
                    // CMYK non-stroking → grayscale
                    let gray = cmyk_to_gray_from_operands(&op.operands);
                    write_real(&mut new_content, gray);
                    new_content.extend_from_slice(b" g\n");
                }
                b"K" if op.operands.len() >= 4 => {
                    // CMYK stroking → grayscale
                    let gray = cmyk_to_gray_from_operands(&op.operands);
                    write_real(&mut new_content, gray);
                    new_content.extend_from_slice(b" G\n");
                }
                _ => {
                    // Write operands then operator unchanged
                    for operand in &op.operands {
                        write_operand(&mut new_content, operand);
                        new_content.push(b' ');
                    }
                    new_content.extend_from_slice(&op.operator);
                    new_content.push(b'\n');
                }
            }
        }

        // Re-encode with FlateDecode
        let new_dict = dict.clone();
        let compressed = match encode_flate_best(&new_content) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let mut final_dict = PdfDict::new();
        // Preserve non-filter/length keys
        for (key, val) in new_dict.iter() {
            if key != b"Length" && key != b"Filter" && key != b"DecodeParms" {
                final_dict.insert(key.clone(), val.clone());
            }
        }
        final_dict.insert(
            b"Filter".to_vec(),
            PdfObject::Name(b"FlateDecode".to_vec()),
        );
        final_dict.insert(
            b"Length".to_vec(),
            PdfObject::Integer(compressed.len() as i64),
        );

        modifier.set_object(
            obj_num,
            PdfObject::Stream {
                dict: final_dict,
                data: compressed,
            },
        );
    }
}

fn operand_to_f64(op: &crate::content::Operand) -> f64 {
    match op {
        crate::content::Operand::Real(v) => *v,
        crate::content::Operand::Integer(v) => *v as f64,
        _ => 0.0,
    }
}

fn rgb_to_gray_from_operands(operands: &[crate::content::Operand]) -> f64 {
    let r = operand_to_f64(&operands[0]);
    let g = operand_to_f64(&operands[1]);
    let b = operand_to_f64(&operands[2]);
    0.299 * r + 0.587 * g + 0.114 * b
}

fn cmyk_to_gray_from_operands(operands: &[crate::content::Operand]) -> f64 {
    let c = operand_to_f64(&operands[0]);
    let m = operand_to_f64(&operands[1]);
    let y = operand_to_f64(&operands[2]);
    let k = operand_to_f64(&operands[3]);
    let r = (1.0 - c) * (1.0 - k);
    let g = (1.0 - m) * (1.0 - k);
    let b = (1.0 - y) * (1.0 - k);
    0.299 * r + 0.587 * g + 0.114 * b
}

fn write_real(buf: &mut Vec<u8>, val: f64) {
    use std::io::Write;
    if (val - val.round()).abs() < 0.0001 {
        write!(buf, "{}", val.round() as i64).unwrap();
    } else {
        write!(buf, "{:.4}", val).unwrap();
    }
}

fn write_operand(buf: &mut Vec<u8>, operand: &crate::content::Operand) {
    use std::io::Write;
    match operand {
        crate::content::Operand::Integer(v) => write!(buf, "{}", v).unwrap(),
        crate::content::Operand::Real(v) => {
            if (*v - v.round()).abs() < 0.0001 {
                write!(buf, "{}", v.round() as i64).unwrap();
            } else {
                write!(buf, "{:.4}", v).unwrap();
            }
        }
        crate::content::Operand::Bool(v) => {
            write!(buf, "{}", if *v { "true" } else { "false" }).unwrap();
        }
        crate::content::Operand::Null => buf.extend_from_slice(b"null"),
        crate::content::Operand::Name(n) => {
            buf.push(b'/');
            buf.extend_from_slice(n);
        }
        crate::content::Operand::String(s) => {
            buf.push(b'(');
            // Escape special chars
            for &byte in s.iter() {
                match byte {
                    b'(' | b')' | b'\\' => {
                        buf.push(b'\\');
                        buf.push(byte);
                    }
                    _ => buf.push(byte),
                }
            }
            buf.push(b')');
        }
        crate::content::Operand::Array(arr) => {
            buf.push(b'[');
            for (i, item) in arr.iter().enumerate() {
                if i > 0 {
                    buf.push(b' ');
                }
                write_operand(buf, item);
            }
            buf.push(b']');
        }
        crate::content::Operand::Dict(entries) => {
            buf.extend_from_slice(b"<< ");
            for (key, val) in entries {
                buf.push(b'/');
                buf.extend_from_slice(key);
                buf.push(b' ');
                write_operand(buf, val);
                buf.push(b' ');
            }
            buf.extend_from_slice(b">>");
        }
        crate::content::Operand::InlineImage { .. } => {
            // Inline images are complex — write as-is (rare in practice)
            buf.extend_from_slice(b"BI ");
        }
    }
}

/// Subset embedded TrueType fonts to contain only used glyphs.
///
/// Walks all pages to collect used character codes per font, then subsets
/// each TrueType font's FontFile2 stream using only those glyphs. Glyph IDs
/// are kept, so the font dictionaries are left as they are. A font is left
/// whole when its used codes cannot be collected or resolved with confidence:
/// it is reachable from anything other than a page's own `/Resources`, it is
/// shown with the `"` operator, or [`simple_font_glyph_ids`] /
/// [`cid_font_glyph_ids`] return `None`.
fn subset_embedded_fonts(modifier: &mut DocumentModifier, stats: &mut CompressStats) {
    // Step 1: Find all Page objects and collect content refs + resource refs.
    // We collect raw data first to avoid borrow conflicts.
    let mut page_raw: Vec<(Vec<u32>, PdfDict)> = Vec::new();

    for (_, obj) in modifier.writer().objects.iter() {
        if let PdfObject::Dict(dict) = obj {
            if dict.get_name(b"Type") != Some(b"Page") {
                continue;
            }

            let content_obj_nums = match dict.get(b"Contents") {
                Some(PdfObject::Reference(r)) => vec![r.obj_num],
                Some(PdfObject::Array(arr)) => arr
                    .iter()
                    .filter_map(|o| {
                        if let PdfObject::Reference(r) = o {
                            Some(r.obj_num)
                        } else {
                            None
                        }
                    })
                    .collect(),
                _ => continue,
            };

            page_raw.push((content_obj_nums, dict.clone()));
        }
    }

    // Now resolve font maps outside the borrow
    let mut page_data: Vec<(Vec<u32>, HashMap<Vec<u8>, u32>)> = Vec::new();
    for (content_obj_nums, page_dict) in page_raw {
        let font_map = extract_font_map(&page_dict, modifier);
        page_data.push((content_obj_nums, font_map));
    }

    // Step 2: Parse content streams and collect used char codes per font obj_num
    let mut font_char_codes: HashMap<u32, HashSet<u16>> = HashMap::new();
    let mut unsafe_fonts: HashSet<u32> = HashSet::new();

    for (content_obj_nums, font_map) in &page_data {
        // Concatenate content stream data
        let mut stream_data = Vec::new();
        for &obj_num in content_obj_nums {
            if let Some(data) = get_stream_decoded_data(obj_num, modifier) {
                stream_data.extend_from_slice(&data);
                stream_data.push(b'\n');
            }
        }

        if stream_data.is_empty() {
            continue;
        }

        // Parse and collect char codes per font name
        let ops = match parse_content_stream(&stream_data) {
            Ok(ops) => ops,
            Err(_) => continue,
        };

        let mut current_font_name: Option<Vec<u8>> = None;
        let mut saved_font_names: Vec<Option<Vec<u8>>> = Vec::new();

        for op in &ops {
            match op.operator.as_slice() {
                b"q" => saved_font_names.push(current_font_name.clone()),
                b"Q" => {
                    if let Some(name) = saved_font_names.pop() {
                        current_font_name = name;
                    }
                }
                b"Tf" => {
                    // Tf: font_name font_size
                    if let Some(name) = op.operands.first().and_then(|o| o.as_name()) {
                        current_font_name = Some(name.to_vec());
                    }
                }
                b"\"" => {
                    if let Some(&font_obj_num) = current_font_name
                        .as_ref()
                        .and_then(|name| font_map.get(name.as_slice()))
                    {
                        unsafe_fonts.insert(font_obj_num);
                    }
                }
                b"Tj" | b"'" => {
                    if let (Some(font_name), Some(s)) = (
                        &current_font_name,
                        op.operands.first().and_then(|o| o.as_str()),
                    ) {
                        if let Some(&font_obj_num) = font_map.get(font_name.as_slice()) {
                            let codes = font_char_codes.entry(font_obj_num).or_default();
                            extract_char_codes(s, font_obj_num, modifier, codes);
                        }
                    }
                }
                b"TJ" => {
                    if let (Some(font_name), Some(arr)) =
                        (&current_font_name, op.operands.first().and_then(|o| o.as_array()))
                    {
                        if let Some(&font_obj_num) = font_map.get(font_name.as_slice()) {
                            let codes = font_char_codes.entry(font_obj_num).or_default();
                            for item in arr {
                                if let Some(s) = item.as_str() {
                                    extract_char_codes(s, font_obj_num, modifier, codes);
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    // Step 3: For each font with collected char codes, find FontFile2 and subset
    let font_obj_nums: Vec<u32> = font_char_codes.keys().copied().collect();
    let candidates: HashSet<u32> = font_obj_nums.iter().copied().collect();
    unsafe_fonts.extend(fonts_reachable_outside_page_resources(
        &candidates,
        modifier,
    ));

    for font_obj_num in font_obj_nums {
        if unsafe_fonts.contains(&font_obj_num) {
            continue;
        }
        let char_codes = match font_char_codes.get(&font_obj_num) {
            Some(codes) if !codes.is_empty() => codes,
            _ => continue,
        };

        // Resolve font → FontDescriptor → FontFile2
        let fontfile2_obj_num = match find_fontfile2(font_obj_num, modifier) {
            Some(num) => num,
            None => continue,
        };

        // Get the FontFile2 stream data
        let font_data = match get_stream_raw_data(fontfile2_obj_num, modifier) {
            Some(data) => data,
            None => continue,
        };

        // Decode if compressed
        let font_dict = match find_object_dict(fontfile2_obj_num, modifier) {
            Some(d) => d,
            None => continue,
        };
        let decoded_font = match crate::stream::decode_stream(&font_data, &font_dict) {
            Ok(d) => d,
            Err(_) => continue,
        };

        let glyph_ids = if is_cid_font(font_obj_num, modifier) {
            cid_font_glyph_ids(font_obj_num, char_codes, modifier)
        } else {
            simple_font_glyph_ids(font_obj_num, char_codes, &decoded_font, modifier)
        };
        let glyph_ids = match glyph_ids {
            Some(ids) => ids,
            None => continue,
        };

        // Subset the font
        let subset_result = match subset_font(&decoded_font, &glyph_ids) {
            Some(r) => r,
            None => continue, // CFF, invalid font, etc. — keep original
        };

        // Only replace if subset is smaller
        if subset_result.data.len() >= decoded_font.len() {
            continue;
        }

        // Re-encode with FlateDecode
        let compressed = match encode_flate_best(&subset_result.data) {
            Ok(c) if c.len() < font_data.len() => c,
            _ => continue,
        };

        // Replace the FontFile2 stream
        let mut new_dict = font_dict;
        new_dict.insert(
            b"Length".to_vec(),
            PdfObject::Integer(compressed.len() as i64),
        );
        new_dict.insert(
            b"Length1".to_vec(),
            PdfObject::Integer(subset_result.data.len() as i64),
        );
        new_dict.insert(
            b"Filter".to_vec(),
            PdfObject::Name(b"FlateDecode".to_vec()),
        );

        modifier.set_object(
            fontfile2_obj_num,
            PdfObject::Stream {
                dict: new_dict,
                data: compressed,
            },
        );

        stats.fonts_subsetted += 1;
    }
}

/// Extract font resource name → font obj_num mapping from a Page dict.
fn extract_font_map(page_dict: &PdfDict, modifier: &DocumentModifier) -> HashMap<Vec<u8>, u32> {
    let mut map = HashMap::new();

    let resources = match page_dict.get(b"Resources") {
        Some(PdfObject::Dict(d)) => d.clone(),
        Some(PdfObject::Reference(r)) => match find_object_dict(r.obj_num, modifier) {
            Some(d) => d,
            None => return map,
        },
        _ => return map,
    };

    let font_dict = match resources.get(b"Font") {
        Some(PdfObject::Dict(d)) => d.clone(),
        Some(PdfObject::Reference(r)) => match find_object_dict(r.obj_num, modifier) {
            Some(d) => d,
            None => return map,
        },
        _ => return map,
    };

    for (name, value) in font_dict.iter() {
        if let PdfObject::Reference(r) = value {
            map.insert(name.clone(), r.obj_num);
        }
    }

    map
}

/// Extract character codes from a string operand, handling both 1-byte (simple)
/// and 2-byte (CID) encodings based on the font type.
fn extract_char_codes(
    string_data: &[u8],
    font_obj_num: u32,
    modifier: &DocumentModifier,
    codes: &mut HashSet<u16>,
) {
    let is_cid = is_cid_font(font_obj_num, modifier);

    if is_cid && string_data.len() >= 2 {
        // CID fonts use 2-byte big-endian character codes
        for chunk in string_data.chunks(2) {
            if chunk.len() == 2 {
                let cid = ((chunk[0] as u16) << 8) | (chunk[1] as u16);
                codes.insert(cid);
            } else {
                // Odd byte at end — treat as single byte
                codes.insert(chunk[0] as u16);
            }
        }
    } else {
        // Simple fonts use 1-byte character codes
        for &byte in string_data {
            codes.insert(byte as u16);
        }
    }
}

/// Glyph IDs a viewer may draw for `codes` in a simple TrueType font.
///
/// Keeps every glyph any lookup path reaches — each `cmap` subtable (with the
/// symbolic `0xF000`/`0xF100`/`0xF200` prefixes), the `/Differences` glyph
/// names through `post`, the WinAnsi reading of the code through the font's
/// Unicode `cmap`, and the code taken as a glyph ID — so the kept set does not
/// depend on which path a viewer takes. Returns `None` when a used code cannot
/// be resolved: a `/Differences` name found by neither `post` nor its Unicode
/// value, or a code at or above `0x80` in a non-symbolic font whose base
/// encoding is not WinAnsi.
fn simple_font_glyph_ids(
    font_obj_num: u32,
    codes: &HashSet<u16>,
    font_program: &[u8],
    modifier: &DocumentModifier,
) -> Option<Vec<u16>> {
    let face = ttf_parser::Face::parse(font_program, 0).ok()?;
    let font_dict = find_object_dict(font_obj_num, modifier)?;

    let (base_encoding, differences) = match font_dict.get(b"Encoding") {
        None => (None, Vec::new()),
        Some(PdfObject::Name(name)) => (Some(name.clone()), Vec::new()),
        Some(PdfObject::Dict(enc)) => (
            enc.get_name(b"BaseEncoding").map(|n| n.to_vec()),
            crate::font::type3::parse_encoding_differences(&font_dict),
        ),
        Some(PdfObject::Reference(r)) => {
            let enc = find_object_dict(r.obj_num, modifier)?;
            let mut inline = PdfDict::new();
            inline.insert(b"Encoding".to_vec(), PdfObject::Dict(enc.clone()));
            (
                enc.get_name(b"BaseEncoding").map(|n| n.to_vec()),
                crate::font::type3::parse_encoding_differences(&inline),
            )
        }
        Some(_) => return None,
    };
    let winansi = base_encoding.as_deref() == Some(b"WinAnsiEncoding".as_slice());

    let flags = match font_dict.get(b"FontDescriptor") {
        Some(PdfObject::Reference(r)) => find_object_dict(r.obj_num, modifier)
            .and_then(|fd| fd.get_i64(b"Flags"))
            .unwrap_or(0),
        _ => 0,
    };
    let symbolic = flags & 4 != 0;

    let mut gids: BTreeSet<u16> = BTreeSet::new();
    for &code in codes {
        let byte = u8::try_from(code).ok()?;

        if let Some(cmap) = face.tables().cmap {
            for subtable in cmap.subtables {
                for prefix in [0u32, 0xF000, 0xF100, 0xF200] {
                    if let Some(gid) = subtable.glyph_index(prefix | u32::from(byte)) {
                        gids.insert(gid.0);
                    }
                }
            }
        }
        if code < face.number_of_glyphs() {
            gids.insert(code);
        }

        if let Some((_, name)) = differences.iter().rev().find(|(c, _)| *c == byte) {
            let by_name = std::str::from_utf8(name)
                .ok()
                .and_then(|n| face.glyph_index_by_name(n));
            let by_unicode = glyph_name_to_char(name).and_then(|c| face.glyph_index(c));
            if by_name.is_none() && by_unicode.is_none() {
                return None;
            }
            gids.extend(by_name.map(|g| g.0));
            gids.extend(by_unicode.map(|g| g.0));
        } else {
            if byte >= 0x80 && !symbolic && !winansi {
                return None;
            }
            let decoded = crate::font::decode_text(&[byte], crate::font::Encoding::WinAnsiEncoding);
            let standard_quote = match byte {
                b'\'' => Some('\u{2019}'),
                b'`' => Some('\u{2018}'),
                _ => None,
            };
            for c in decoded.chars().chain(standard_quote) {
                if let Some(gid) = face.glyph_index(c) {
                    gids.insert(gid.0);
                }
            }
        }
    }

    Some(gids.into_iter().collect())
}

/// The character a glyph name stands for, where the name spells it out:
/// `uniXXXX`, `uXXXX`–`uXXXXXX`, or a single ASCII letter.
fn glyph_name_to_char(name: &[u8]) -> Option<char> {
    let name = std::str::from_utf8(name).ok()?;
    let hex = name
        .strip_prefix("uni")
        .filter(|h| h.len() == 4)
        .or_else(|| {
            name.strip_prefix('u')
                .filter(|h| (4..=6).contains(&h.len()))
        });
    if let Some(hex) = hex {
        return u32::from_str_radix(hex, 16).ok().and_then(char::from_u32);
    }
    let mut chars = name.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) if c.is_ascii_alphabetic() => Some(c),
        _ => None,
    }
}

/// Glyph IDs for `cids` in an `Identity-H`/`Identity-V` Type0 font, through the
/// descendant's `/CIDToGIDMap` (absent or `/Identity` means CID = GID).
/// Returns `None` for any other encoding, or a map that cannot be read.
fn cid_font_glyph_ids(
    font_obj_num: u32,
    cids: &HashSet<u16>,
    modifier: &DocumentModifier,
) -> Option<Vec<u16>> {
    let font_dict = find_object_dict(font_obj_num, modifier)?;
    match font_dict.get_name(b"Encoding") {
        Some(b"Identity-H") | Some(b"Identity-V") => {}
        _ => return None,
    }
    let cid_font_obj_num = match font_dict.get(b"DescendantFonts") {
        Some(PdfObject::Array(arr)) => match arr.first() {
            Some(PdfObject::Reference(r)) => r.obj_num,
            _ => return None,
        },
        _ => return None,
    };
    let cid_font_dict = find_object_dict(cid_font_obj_num, modifier)?;

    match cid_font_dict.get(b"CIDToGIDMap") {
        None => Some(cids.iter().copied().collect()),
        Some(PdfObject::Name(name)) if name == b"Identity" => Some(cids.iter().copied().collect()),
        Some(PdfObject::Reference(r)) => {
            let map = get_stream_decoded_data(r.obj_num, modifier)?;
            Some(
                cids.iter()
                    .map(|&cid| {
                        let offset = cid as usize * 2;
                        match map.get(offset..offset + 2) {
                            Some(b) => u16::from_be_bytes([b[0], b[1]]),
                            None => 0,
                        }
                    })
                    .collect(),
            )
        }
        Some(_) => None,
    }
}

/// Fonts among `candidates` that are referenced from anything other than a
/// page's own `/Resources` — a form XObject, an annotation appearance, the
/// `/Resources` a page inherits from the page tree, the AcroForm `/DR`.
///
/// The objects allowed to hold a page-owned font reference are the page itself
/// (inline `/Resources` and `/Font`), the page's indirect `/Resources`, and its
/// indirect `/Font` dictionary — each referenced only by pages (or, for a
/// `/Font` dictionary, by such `/Resources`).
fn fonts_reachable_outside_page_resources(
    candidates: &HashSet<u32>,
    modifier: &mut DocumentModifier,
) -> HashSet<u32> {
    let mut referrers: HashMap<u32, Vec<u32>> = HashMap::new();
    let mut page_dicts: Vec<(u32, PdfDict)> = Vec::new();
    for (obj_num, obj) in modifier.writer().objects.iter() {
        if let PdfObject::Dict(d) = obj
            && d.get_name(b"Type") == Some(b"Page")
        {
            page_dicts.push((*obj_num, d.clone()));
        }
        let mut refs = Vec::new();
        collect_reference_targets(obj, &mut refs);
        for target in refs {
            referrers.entry(target).or_default().push(*obj_num);
        }
    }
    let pages: HashSet<u32> = page_dicts.iter().map(|(n, _)| *n).collect();
    let referenced_only_by = |obj_num: u32, allowed: &HashSet<u32>| {
        referrers
            .get(&obj_num)
            .is_some_and(|refs| refs.iter().all(|r| allowed.contains(r)))
    };

    let mut resources_objs: HashSet<u32> = HashSet::new();
    let mut font_dict_objs: HashSet<u32> = HashSet::new();
    let mut holders: HashSet<u32> = HashSet::new();
    for (page_num, page) in &page_dicts {
        let (resources, resources_holder) = match page.get(b"Resources") {
            Some(PdfObject::Dict(d)) => (d.clone(), *page_num),
            Some(PdfObject::Reference(r)) => match find_object_dict(r.obj_num, modifier) {
                Some(d) => {
                    resources_objs.insert(r.obj_num);
                    (d, r.obj_num)
                }
                None => continue,
            },
            _ => continue,
        };
        match resources.get(b"Font") {
            Some(PdfObject::Dict(_)) => {
                holders.insert(resources_holder);
            }
            Some(PdfObject::Reference(r)) => {
                font_dict_objs.insert(r.obj_num);
            }
            _ => {}
        }
    }
    let valid_resources: HashSet<u32> = resources_objs
        .into_iter()
        .filter(|&r| referenced_only_by(r, &pages))
        .collect();
    holders.retain(|h| pages.contains(h) || valid_resources.contains(h));
    let font_dict_referrers: HashSet<u32> = pages.union(&valid_resources).copied().collect();
    holders.extend(
        font_dict_objs
            .into_iter()
            .filter(|&f| referenced_only_by(f, &font_dict_referrers)),
    );

    candidates
        .iter()
        .copied()
        .filter(|&font| !referenced_only_by(font, &holders))
        .collect()
}

/// Object numbers of every indirect reference inside `obj`.
fn collect_reference_targets(obj: &PdfObject, out: &mut Vec<u32>) {
    match obj {
        PdfObject::Reference(r) => out.push(r.obj_num),
        PdfObject::Array(arr) => arr.iter().for_each(|o| collect_reference_targets(o, out)),
        PdfObject::Dict(d) => d
            .iter()
            .for_each(|(_, o)| collect_reference_targets(o, out)),
        PdfObject::Stream { dict, .. } => dict
            .iter()
            .for_each(|(_, o)| collect_reference_targets(o, out)),
        _ => {}
    }
}

/// Check if a font object is a CID font (Type0 composite font).
fn is_cid_font(font_obj_num: u32, modifier: &DocumentModifier) -> bool {
    if let Some(dict) = find_object_dict(font_obj_num, modifier) {
        dict.get_name(b"Subtype") == Some(b"Type0")
    } else {
        false
    }
}

/// Find FontFile2 obj_num by walking Font → FontDescriptor → FontFile2.
fn find_fontfile2(font_obj_num: u32, modifier: &DocumentModifier) -> Option<u32> {
    let font_dict = find_object_dict(font_obj_num, modifier)?;

    let subtype = font_dict.get_name(b"Subtype")?;

    match subtype {
        b"TrueType" => {
            // Simple TrueType: Font → FontDescriptor → FontFile2
            let fd_obj_num = match font_dict.get(b"FontDescriptor") {
                Some(PdfObject::Reference(r)) => r.obj_num,
                _ => return None,
            };
            let fd_dict = find_object_dict(fd_obj_num, modifier)?;
            match fd_dict.get(b"FontFile2") {
                Some(PdfObject::Reference(r)) => Some(r.obj_num),
                _ => None,
            }
        }
        b"Type0" => {
            // CID font: Type0 → DescendantFonts[0] → FontDescriptor → FontFile2
            let descendants = match font_dict.get(b"DescendantFonts") {
                Some(PdfObject::Array(arr)) => arr.clone(),
                _ => return None,
            };
            let cid_font_ref = match descendants.first() {
                Some(PdfObject::Reference(r)) => r.obj_num,
                _ => return None,
            };
            let cid_font_dict = find_object_dict(cid_font_ref, modifier)?;

            // Must be CIDFontType2 (TrueType-based CID font)
            if cid_font_dict.get_name(b"Subtype") != Some(b"CIDFontType2") {
                return None;
            }

            let fd_obj_num = match cid_font_dict.get(b"FontDescriptor") {
                Some(PdfObject::Reference(r)) => r.obj_num,
                _ => return None,
            };
            let fd_dict = find_object_dict(fd_obj_num, modifier)?;
            match fd_dict.get(b"FontFile2") {
                Some(PdfObject::Reference(r)) => Some(r.obj_num),
                _ => None,
            }
        }
        _ => None,
    }
}

/// Find an object by obj_num and return its dict (if it is a Dict or Stream).
fn find_object_dict(obj_num: u32, modifier: &DocumentModifier) -> Option<PdfDict> {
    let obj = modifier.find_object_pub(obj_num)?;
    match obj {
        PdfObject::Dict(d) => Some(d.clone()),
        PdfObject::Stream { dict, .. } => Some(dict.clone()),
        _ => None,
    }
}

/// Get raw (encoded) stream data for an object.
fn get_stream_raw_data(obj_num: u32, modifier: &DocumentModifier) -> Option<Vec<u8>> {
    let obj = modifier.find_object_pub(obj_num)?;
    match obj {
        PdfObject::Stream { data, .. } => Some(data.clone()),
        _ => None,
    }
}

/// Get decoded stream data for an object (handles FlateDecode etc).
fn get_stream_decoded_data(obj_num: u32, modifier: &DocumentModifier) -> Option<Vec<u8>> {
    let obj = modifier.find_object_pub(obj_num)?;
    match obj {
        PdfObject::Stream { dict, data } => {
            crate::stream::decode_stream(data, dict).ok()
        }
        _ => None,
    }
}

/// Remove unused resources from page Resource dictionaries.
///
/// Parses each page's content streams to find actually used resource names
/// (Tf for fonts, Do for XObjects, gs for ExtGState), then removes any
/// entries in the Resources dict that are not referenced.
fn remove_unused_resources(modifier: &mut DocumentModifier, stats: &mut CompressStats) {
    // Collect page obj_nums and their data
    let mut pages: Vec<(u32, PdfDict)> = Vec::new();
    for (obj_num, obj) in modifier.writer().objects.iter() {
        if let PdfObject::Dict(dict) = obj {
            if dict.get_name(b"Type") == Some(b"Page") {
                pages.push((*obj_num, dict.clone()));
            }
        }
    }

    for (page_obj_num, page_dict) in pages {
        // Get content stream obj_nums
        let content_obj_nums = match page_dict.get(b"Contents") {
            Some(PdfObject::Reference(r)) => vec![r.obj_num],
            Some(PdfObject::Array(arr)) => arr
                .iter()
                .filter_map(|o| {
                    if let PdfObject::Reference(r) = o { Some(r.obj_num) } else { None }
                })
                .collect(),
            _ => continue,
        };

        // Decode and concatenate content streams
        let mut stream_data = Vec::new();
        for &obj_num in &content_obj_nums {
            if let Some(data) = get_stream_decoded_data(obj_num, modifier) {
                stream_data.extend_from_slice(&data);
                stream_data.push(b'\n');
            }
        }

        if stream_data.is_empty() {
            continue;
        }

        // Parse content stream to collect used resource names
        let ops = match parse_content_stream(&stream_data) {
            Ok(ops) => ops,
            Err(_) => continue,
        };

        let mut used_fonts: HashSet<Vec<u8>> = HashSet::new();
        let mut used_xobjects: HashSet<Vec<u8>> = HashSet::new();
        let mut used_extgstate: HashSet<Vec<u8>> = HashSet::new();

        for op in &ops {
            match op.operator.as_slice() {
                b"Tf" => {
                    if let Some(name) = op.operands.first().and_then(|o| o.as_name()) {
                        used_fonts.insert(name.to_vec());
                    }
                }
                b"Do" => {
                    if let Some(name) = op.operands.first().and_then(|o| o.as_name()) {
                        used_xobjects.insert(name.to_vec());
                    }
                }
                b"gs" => {
                    if let Some(name) = op.operands.first().and_then(|o| o.as_name()) {
                        used_extgstate.insert(name.to_vec());
                    }
                }
                _ => {}
            }
        }

        // Recurse into Form XObjects to collect their resource usage too
        let xobject_map = extract_xobject_map(&page_dict, modifier);
        let mut form_xobjects_to_check: Vec<Vec<u8>> = used_xobjects.iter().cloned().collect();
        let mut checked: HashSet<Vec<u8>> = HashSet::new();

        while let Some(xobj_name) = form_xobjects_to_check.pop() {
            if !checked.insert(xobj_name.clone()) {
                continue;
            }
            if let Some(&xobj_obj_num) = xobject_map.get(&xobj_name) {
                if let Some(PdfObject::Stream { dict, data }) =
                    modifier.find_object_pub(xobj_obj_num).cloned()
                {
                    // Only process Form XObjects (not images)
                    if dict.get_name(b"Subtype") != Some(b"Form") {
                        continue;
                    }
                    // Decode and parse the Form XObject's content stream
                    if let Ok(form_data) = crate::stream::decode_stream(&data, &dict) {
                        if let Ok(form_ops) = parse_content_stream(&form_data) {
                            for op in &form_ops {
                                match op.operator.as_slice() {
                                    b"Tf" => {
                                        if let Some(n) = op.operands.first().and_then(|o| o.as_name()) {
                                            used_fonts.insert(n.to_vec());
                                        }
                                    }
                                    b"Do" => {
                                        if let Some(n) = op.operands.first().and_then(|o| o.as_name()) {
                                            used_xobjects.insert(n.to_vec());
                                            form_xobjects_to_check.push(n.to_vec());
                                        }
                                    }
                                    b"gs" => {
                                        if let Some(n) = op.operands.first().and_then(|o| o.as_name()) {
                                            used_extgstate.insert(n.to_vec());
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            }
        }

        // Get resources dict (resolve if indirect)
        let (resources_obj_num, resources_dict) = match page_dict.get(b"Resources") {
            Some(PdfObject::Dict(d)) => (None, d.clone()),
            Some(PdfObject::Reference(r)) => {
                match find_object_dict(r.obj_num, modifier) {
                    Some(d) => (Some(r.obj_num), d),
                    None => continue,
                }
            }
            _ => continue,
        };

        let mut modified = false;
        let mut new_resources = resources_dict.clone();

        // Clean Font sub-dict
        if let Some(cleaned) =
            clean_resource_subdict(&resources_dict, b"Font", &used_fonts, modifier)
        {
            let removed = count_removed(&resources_dict, b"Font", &cleaned, modifier);
            stats.unused_resources_removed += removed;
            new_resources.insert(b"Font".to_vec(), cleaned);
            modified = true;
        }

        // Clean XObject sub-dict
        if let Some(cleaned) =
            clean_resource_subdict(&resources_dict, b"XObject", &used_xobjects, modifier)
        {
            let removed = count_removed(&resources_dict, b"XObject", &cleaned, modifier);
            stats.unused_resources_removed += removed;
            new_resources.insert(b"XObject".to_vec(), cleaned);
            modified = true;
        }

        // Clean ExtGState sub-dict
        if let Some(cleaned) =
            clean_resource_subdict(&resources_dict, b"ExtGState", &used_extgstate, modifier)
        {
            let removed = count_removed(&resources_dict, b"ExtGState", &cleaned, modifier);
            stats.unused_resources_removed += removed;
            new_resources.insert(b"ExtGState".to_vec(), cleaned);
            modified = true;
        }

        if !modified {
            continue;
        }

        // Update the resources
        if let Some(res_obj_num) = resources_obj_num {
            // Resources is a separate object
            modifier.set_object(res_obj_num, PdfObject::Dict(new_resources));
        } else {
            // Resources is inline in the page dict — update the page
            let mut new_page = page_dict.clone();
            new_page.insert(b"Resources".to_vec(), PdfObject::Dict(new_resources));
            modifier.set_object(page_obj_num, PdfObject::Dict(new_page));
        }
    }
}

/// Clean a resource sub-dictionary by removing entries not in `used_names`.
/// Returns the new PdfObject for the sub-dict, or None if nothing changed.
fn clean_resource_subdict(
    resources: &PdfDict,
    key: &[u8],
    used_names: &HashSet<Vec<u8>>,
    modifier: &DocumentModifier,
) -> Option<PdfObject> {
    let subdict = match resources.get(key) {
        Some(PdfObject::Dict(d)) => d.clone(),
        Some(PdfObject::Reference(r)) => find_object_dict(r.obj_num, modifier)?,
        _ => return None,
    };

    let original_count = subdict.len();
    if original_count == 0 {
        return None;
    }

    let mut new_dict = PdfDict::new();
    for (name, value) in subdict.iter() {
        if used_names.contains(name) {
            new_dict.insert(name.clone(), value.clone());
        }
    }

    if new_dict.len() == original_count {
        return None; // nothing removed
    }

    Some(PdfObject::Dict(new_dict))
}

/// Count how many entries were removed from a resource sub-dict.
fn count_removed(
    resources: &PdfDict,
    key: &[u8],
    new_value: &PdfObject,
    modifier: &DocumentModifier,
) -> usize {
    let old_count = match resources.get(key) {
        Some(PdfObject::Dict(d)) => d.len(),
        Some(PdfObject::Reference(r)) => {
            find_object_dict(r.obj_num, modifier).map(|d| d.len()).unwrap_or(0)
        }
        _ => 0,
    };
    let new_count = match new_value {
        PdfObject::Dict(d) => d.len(),
        _ => 0,
    };
    old_count.saturating_sub(new_count)
}

/// Strip metadata, structure trees, thumbnails, and other non-essential data.
///
/// What gets removed depends on the options:
/// - `strip_metadata`: XMP metadata, StructTreeRoot, thumbnails, OutputIntents,
///   PieceInfo, LastModified
/// - `strip_extras`: embedded files, JavaScript/actions (in addition to above)
fn strip_non_essential(
    modifier: &mut DocumentModifier,
    options: &CompressOptions,
    stats: &mut CompressStats,
) {
    let catalog_obj_num = modifier.catalog_ref().obj_num;

    // Get catalog dict
    let catalog = match find_object_dict(catalog_obj_num, modifier) {
        Some(d) => d,
        None => return,
    };

    let mut new_catalog = catalog.clone();
    let mut changed = false;

    // Keys to remove from Catalog when strip_metadata is enabled
    if options.strip_metadata {
        let metadata_keys: &[&[u8]] = &[
            b"Metadata",       // XMP metadata stream
            b"StructTreeRoot", // Structure tree (accessibility)
            b"OutputIntents",  // Output intent / ICC profiles
            b"PieceInfo",      // App-specific data
            b"MarkInfo",       // Marked content info (related to StructTreeRoot)
        ];
        for key in metadata_keys {
            if new_catalog.remove(key).is_some() {
                stats.metadata_items_stripped += 1;
                changed = true;
            }
        }
    }

    // Keys to remove from Catalog when strip_extras is enabled
    if options.strip_extras {
        // Remove EmbeddedFiles and JavaScript from Names dict
        if let Some(PdfObject::Reference(names_ref)) = new_catalog.get(b"Names") {
            let names_obj_num = names_ref.obj_num;
            if let Some(mut names_dict) = find_object_dict(names_obj_num, modifier) {
                let extra_keys: &[&[u8]] = &[b"EmbeddedFiles", b"JavaScript"];
                let mut names_changed = false;
                for key in extra_keys {
                    if names_dict.remove(key).is_some() {
                        stats.metadata_items_stripped += 1;
                        names_changed = true;
                    }
                }
                if names_changed {
                    modifier.set_object(names_obj_num, PdfObject::Dict(names_dict));
                }
            }
        } else if let Some(PdfObject::Dict(names_dict)) = new_catalog.get(b"Names") {
            let mut nd = names_dict.clone();
            let extra_keys: &[&[u8]] = &[b"EmbeddedFiles", b"JavaScript"];
            for key in extra_keys {
                if nd.remove(key).is_some() {
                    stats.metadata_items_stripped += 1;
                    changed = true;
                }
            }
            new_catalog.insert(b"Names".to_vec(), PdfObject::Dict(nd));
        }
    }

    if changed {
        modifier.set_object(catalog_obj_num, PdfObject::Dict(new_catalog));
    }

    // Strip page-level items: thumbnails (/Thumb) and page actions (/AA)
    let mut page_updates: Vec<(u32, PdfDict)> = Vec::new();

    for (obj_num, obj) in modifier.writer().objects.iter() {
        if let PdfObject::Dict(dict) = obj {
            if dict.get_name(b"Type") != Some(b"Page") {
                continue;
            }
            let mut new_dict = dict.clone();
            let mut page_changed = false;

            if options.strip_metadata {
                if new_dict.remove(b"Thumb").is_some() {
                    stats.metadata_items_stripped += 1;
                    page_changed = true;
                }
                if new_dict.remove(b"PieceInfo").is_some() {
                    stats.metadata_items_stripped += 1;
                    page_changed = true;
                }
                if new_dict.remove(b"LastModified").is_some() {
                    stats.metadata_items_stripped += 1;
                    page_changed = true;
                }
            }

            if options.strip_extras {
                if new_dict.remove(b"AA").is_some() {
                    stats.metadata_items_stripped += 1;
                    page_changed = true;
                }
            }

            if page_changed {
                page_updates.push((*obj_num, new_dict));
            }
        }
    }

    for (obj_num, dict) in page_updates {
        modifier.set_object(obj_num, PdfObject::Dict(dict));
    }
}

/// Deduplicate streams with identical data using SHA-256 hashing.
///
/// For each pair of streams with identical data, keeps the first and rewrites
/// all references to the duplicate to point to the first. GC will then remove
/// the orphaned duplicate objects.
fn dedup_streams(modifier: &mut DocumentModifier, stats: &mut CompressStats) {
    // Phase 1: Hash all stream data
    let mut hash_to_first: HashMap<[u8; 32], u32> = HashMap::new();
    let mut remap: HashMap<u32, u32> = HashMap::new();

    for (obj_num, obj) in modifier.writer().objects.iter() {
        if let PdfObject::Stream { data, .. } = obj {
            let mut hasher = Sha256::new();
            hasher.update(data);
            let hash: [u8; 32] = hasher.finalize().into();

            match hash_to_first.get(&hash) {
                Some(&first_num) if first_num != *obj_num => {
                    remap.insert(*obj_num, first_num);
                }
                _ => {
                    hash_to_first.insert(hash, *obj_num);
                }
            }
        }
    }

    if remap.is_empty() {
        return;
    }

    stats.duplicates_removed = remap.len();

    // Phase 2: Rewrite all references
    for (_, obj) in modifier.writer().objects.iter_mut() {
        rewrite_references(obj, &remap);
    }

    // Phase 3: Remove the duplicate objects (GC will handle this,
    // but we can also remove them directly for immediate effect)
    modifier
        .writer()
        .objects
        .retain(|(obj_num, _)| !remap.contains_key(obj_num));
}

/// Check if a stream dict has exactly one FlateDecode filter.
fn is_single_flate(dict: &PdfDict) -> bool {
    match dict.get(b"Filter") {
        Some(PdfObject::Name(name)) => name == b"FlateDecode" || name == b"Fl",
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::object::IndirectRef;
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
        // 1000×1000 keeps the JPEG stream above preset_medium's
        // skip_below_bytes (10_000) so re-encoding is exercised.
        let pdf = create_pdf_with_jpeg(1000, 1000, 95);
        let original_size = pdf.len();

        let (compressed, stats) =
            compress_pdf(&pdf, &CompressOptions::preset_medium()).unwrap();

        assert!(compressed.starts_with(b"%PDF"));
        assert_eq!(stats.original_size, original_size);
        assert!(stats.images_found >= 1);
        // medium preset must re-encode the image (q95 source → q75 target)
        assert!(
            stats.images_recompressed > 0,
            "expected medium preset to re-encode the image"
        );
        assert!(stats.compressed_size < original_size);
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
        // high preset must re-encode (and likely downscale) the large image
        assert!(
            stats.images_recompressed > 0,
            "expected high preset to re-encode the large image"
        );
        assert!(stats.compressed_size < original_size);
    }

    #[test]
    fn test_compress_extreme() {
        // 1000×1000 keeps the JPEG stream above preset_extreme's
        // skip_below_bytes (2_000) and provides headroom for compression.
        let pdf = create_pdf_with_jpeg(1000, 1000, 95);
        let original_size = pdf.len();

        let (compressed, stats) =
            compress_pdf(&pdf, &CompressOptions::preset_extreme()).unwrap();

        assert!(compressed.starts_with(b"%PDF"));
        // extreme preset must re-encode the image
        assert!(
            stats.images_recompressed > 0,
            "expected extreme preset to re-encode the image"
        );
        // External baseline: compressed bytes must be smaller than input bytes
        assert!(
            compressed.len() < original_size,
            "compressed {} should be smaller than input {}",
            compressed.len(),
            original_size,
        );
        // Internal stat consistency
        assert!(stats.compressed_size < stats.original_size);
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

    // ── Phase B: Flate recompression tests ──────────────────────────

    /// Create a text PDF (~1000 chars per page) whose FlateDecode streams are
    /// stored at compression level 1, with no DecodeParms and no images.
    /// Returns the PDF and the number of streams stored at level 1.
    fn create_fast_flate_text_pdf(num_pages: usize) -> (Vec<u8>, usize) {
        use std::io::Write;

        let mut doc = DocumentBuilder::new();
        let font = doc.add_standard_font("Helvetica");
        for i in 0..num_pages {
            let mut page = PageBuilder::new(612.0, 792.0);
            page.add_font(&font, "Helvetica");
            page.begin_text();
            page.set_font(&font, 12.0);
            page.move_to(72.0, 720.0);
            page.show_text(&format!("Test page {} lorem ipsum dolor sit amet. ", i + 1).repeat(25));
            page.end_text();
            doc.add_page(page);
        }

        let parsed = PdfDocument::from_bytes(doc.build().unwrap()).unwrap();
        let mut modifier = DocumentModifier::from_document(&parsed).unwrap();
        let flate_streams: Vec<(u32, PdfDict, Vec<u8>)> = modifier
            .writer()
            .objects
            .iter()
            .filter_map(|(obj_num, obj)| match obj {
                PdfObject::Stream { dict, data }
                    if dict.get(b"Filter") == Some(&PdfObject::Name(b"FlateDecode".to_vec())) =>
                {
                    Some((*obj_num, dict.clone(), data.clone()))
                }
                _ => None,
            })
            .collect();
        assert!(
            !flate_streams.is_empty(),
            "fixture must contain FlateDecode streams"
        );
        for (_, dict, _) in &flate_streams {
            assert!(
                dict.get(b"DecodeParms").is_none(),
                "fixture stream has DecodeParms"
            );
            assert_ne!(
                dict.get(b"Subtype"),
                Some(&PdfObject::Name(b"Image".to_vec())),
                "fixture stream is an image"
            );
        }
        let fast_count = flate_streams.len();

        for (obj_num, dict, data) in flate_streams {
            let decoded = crate::stream::decode_stream(&data, &dict).unwrap();
            let mut encoder =
                flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
            encoder.write_all(&decoded).unwrap();
            let fast = encoder.finish().unwrap();
            modifier.set_object(obj_num, PdfObject::Stream { dict, data: fast });
        }
        (modifier.build().unwrap(), fast_count)
    }

    /// B-T1: FlateDecode streams recompressed at best level → output ≤ original.
    #[test]
    fn test_recompress_flate_output_not_larger() {
        let (pdf, fast_count) = create_fast_flate_text_pdf(5);
        let original_size = pdf.len();

        let (compressed, stats) = compress_pdf(&pdf, &CompressOptions::preset_low()).unwrap();

        // Output must be valid and no larger than original
        assert!(compressed.starts_with(b"%PDF"));
        assert!(compressed.len() <= original_size);
        assert!(
            stats.streams_recompressed > 0,
            "expected at least one stream recompression"
        );
        assert_eq!(
            stats.streams_recompressed, fast_count,
            "every level-1 stream should be recompressed"
        );
    }

    /// B-T2: Recompression roundtrip — decoded content identical after recompression.
    #[test]
    fn test_recompress_flate_roundtrip_identical() {
        let (pdf, fast_count) = create_fast_flate_text_pdf(3);

        let (compressed, stats) = compress_pdf(&pdf, &CompressOptions::preset_low()).unwrap();
        assert_eq!(
            stats.streams_recompressed, fast_count,
            "the roundtrip must go through a recompressed stream"
        );

        // Re-parse and extract text to verify content is preserved
        let reparsed = PdfDocument::from_bytes(compressed).unwrap();
        let pages = crate::page::collect_pages(&reparsed).unwrap();
        assert_eq!(pages.len(), 3);

        // Extract text from each page and verify it matches
        for (i, page) in pages.iter().enumerate() {
            let text = crate::text::extract_page_text_string(&reparsed, page).unwrap_or_default();
            let expected = format!("Test page {}", i + 1);
            assert!(
                text.contains(&expected),
                "Page {} text should contain '{}', got '{}'",
                i + 1,
                expected,
                text
            );
        }
    }

    /// B-T3: Already best-compressed streams → size change is zero or minimal.
    #[test]
    fn test_recompress_flate_already_best_no_growth() {
        let (pdf, fast_count) = create_fast_flate_text_pdf(3);

        // First pass: recompress to best
        let (pass1, stats1) = compress_pdf(&pdf, &CompressOptions::preset_low()).unwrap();
        let pass1_size = pass1.len();

        // Second pass: recompress again — should not grow
        let (pass2, stats2) = compress_pdf(&pass1, &CompressOptions::preset_low()).unwrap();

        // Allow tiny variance from re-serialization
        let tolerance = (pass1_size as f64 * 0.01) as usize + 10;
        assert!(
            pass2.len() <= pass1_size + tolerance,
            "Second recompression should not increase size significantly: {} > {} + {}",
            pass2.len(),
            pass1_size,
            tolerance,
        );

        // First pass recompresses; second pass finds every stream already at best
        assert_eq!(
            stats1.streams_recompressed, fast_count,
            "first pass must recompress for the second pass to mean anything"
        );
        assert_eq!(
            stats2.streams_recompressed, 0,
            "second pass should recompress nothing (already at best)"
        );
    }

    /// B-T4: Integration — text PDF with FlateDecode streams shows measurable improvement.
    #[test]
    fn test_recompress_flate_text_pdf_improvement() {
        let (pdf, fast_count) = create_fast_flate_text_pdf(20);
        let original_size = pdf.len();

        let (compressed, stats) = compress_pdf(&pdf, &CompressOptions::preset_low()).unwrap();

        assert!(compressed.starts_with(b"%PDF"));
        assert_eq!(stats.original_size, original_size);

        // The compressed output should be valid and re-parseable
        let reparsed = PdfDocument::from_bytes(compressed.clone()).unwrap();
        let pages = crate::page::collect_pages(&reparsed).unwrap();
        assert_eq!(pages.len(), 20);

        // Every content stream is recompressed and the output is smaller
        assert_eq!(
            stats.streams_recompressed, fast_count,
            "expected content streams to be recompressed"
        );
        assert!(
            compressed.len() < original_size,
            "Compressed size {} should be < original size {}",
            compressed.len(),
            original_size,
        );
    }

    // ── Phase C: Stream dedup tests ─────────────────────────────────

    /// Helper: create a PDF with two identical JPEG images embedded separately.
    fn create_pdf_with_duplicate_images(width: u32, height: u32, quality: u8) -> Vec<u8> {
        let jpeg_data = create_test_jpeg(width, height, quality);

        let mut doc = DocumentBuilder::new();
        let font = doc.add_standard_font("Helvetica");

        // Embed the same JPEG data twice — creates two separate stream objects
        let (_name1, img_ref1) =
            crate::writer::document::embed_jpeg(&mut doc, &jpeg_data).unwrap();
        let (_name2, img_ref2) =
            crate::writer::document::embed_jpeg(&mut doc, &jpeg_data).unwrap();

        let mut page = PageBuilder::new(612.0, 792.0);
        page.add_font(&font, "Helvetica");
        page.add_image("Im1", img_ref1);
        page.add_image("Im2", img_ref2);
        page.begin_text();
        page.set_font(&font, 12.0);
        page.move_to(72.0, 720.0);
        page.show_text("Duplicate images test");
        page.end_text();
        page.draw_image("Im1", 72.0, 400.0, width as f64, height as f64);
        page.draw_image("Im2", 72.0, 100.0, width as f64, height as f64);
        doc.add_page(page);

        doc.build().unwrap()
    }

    /// C-T1: Two identical images → dedup consolidates to one, references intact.
    #[test]
    fn test_dedup_identical_images() {
        let pdf = create_pdf_with_duplicate_images(100, 100, 90);
        let original_size = pdf.len();

        let (compressed, stats) = compress_pdf(&pdf, &CompressOptions::preset_low()).unwrap();

        assert!(compressed.starts_with(b"%PDF"));
        assert!(
            stats.duplicates_removed >= 1,
            "Should dedup at least 1 duplicate stream, got {}",
            stats.duplicates_removed,
        );
        assert!(
            compressed.len() < original_size,
            "Dedup should reduce size: {} >= {}",
            compressed.len(),
            original_size,
        );
    }

    /// C-T2: Two identical fonts → dedup consolidates (font streams are streams too).
    #[test]
    fn test_dedup_identical_font_streams() {
        // Text PDFs with the same font on every page will have identical
        // content stream patterns. Dedup should detect these.
        let pdf = create_text_pdf(5);
        let (compressed, stats) = compress_pdf(&pdf, &CompressOptions::preset_low()).unwrap();

        assert!(compressed.starts_with(b"%PDF"));
        // Content streams may or may not be identical depending on text,
        // but the dedup code should at least not break anything
        let _ = stats.duplicates_removed;
    }

    /// C-T3: Different streams → no false dedup.
    #[test]
    fn test_dedup_different_streams_no_false_positive() {
        // Create two images with different data
        let jpeg1 = create_test_jpeg(100, 100, 90);  // red
        let mut jpeg2_pixels = vec![0u8; 100 * 100 * 3];
        for pixel in jpeg2_pixels.chunks_exact_mut(3) {
            pixel[0] = 0;
            pixel[1] = 0;
            pixel[2] = 255; // blue
        }
        let jpeg2 = {
            use ::image::codecs::jpeg::JpegEncoder;
            use std::io::Cursor;
            let mut buf = Cursor::new(Vec::new());
            let mut enc = JpegEncoder::new_with_quality(&mut buf, 90);
            enc.encode(&jpeg2_pixels, 100, 100, ::image::ExtendedColorType::Rgb8).unwrap();
            buf.into_inner()
        };

        let mut doc = DocumentBuilder::new();
        let font = doc.add_standard_font("Helvetica");
        let (_, ref1) = crate::writer::document::embed_jpeg(&mut doc, &jpeg1).unwrap();
        let (_, ref2) = crate::writer::document::embed_jpeg(&mut doc, &jpeg2).unwrap();

        let mut page = PageBuilder::new(612.0, 792.0);
        page.add_font(&font, "Helvetica");
        page.add_image("Im1", ref1);
        page.add_image("Im2", ref2);
        page.draw_image("Im1", 72.0, 400.0, 100.0, 100.0);
        page.draw_image("Im2", 72.0, 100.0, 100.0, 100.0);
        doc.add_page(page);
        let pdf = doc.build().unwrap();

        let (compressed, stats) = compress_pdf(&pdf, &CompressOptions::preset_low()).unwrap();

        // The two different images must survive in the COMPRESSED output —
        // analyze the result, not the input, so a buggy dedup that merges
        // distinct streams gets caught.
        let analysis = analyze_pdf(&compressed).unwrap();
        assert_eq!(analysis.images, 2);
        assert_eq!(stats.duplicates_removed, 0);
    }

    /// C-T4: Dedup + re-parse → page count and text extraction intact.
    #[test]
    fn test_dedup_roundtrip_valid() {
        let pdf = create_pdf_with_duplicate_images(100, 100, 90);

        let (compressed, stats) = compress_pdf(&pdf, &CompressOptions::preset_low()).unwrap();

        // Re-parse
        let reparsed = PdfDocument::from_bytes(compressed).unwrap();
        let pages = crate::page::collect_pages(&reparsed).unwrap();
        assert_eq!(pages.len(), 1);

        // Text should still be extractable
        let text = crate::text::extract_page_text_string(&reparsed, &pages[0]).unwrap_or_default();
        assert!(
            text.contains("Duplicate images test"),
            "Text should be preserved after dedup, got: '{}'",
            text,
        );
    }

    // ── Phase D: Font subsetting tests ──────────────────────────────

    /// D-T1: Standard fonts (not embedded) → subsetting safely skipped.
    #[test]
    fn test_subset_skips_standard_fonts() {
        let pdf = create_text_pdf(3);

        let options = CompressOptions::preset_medium(); // font_subsetting: true
        let (compressed, stats) = compress_pdf(&pdf, &options).unwrap();

        // Standard fonts have no FontFile2, so subsetting should be skipped
        assert_eq!(stats.fonts_subsetted, 0);
        assert!(compressed.starts_with(b"%PDF"));

        // Text should be preserved
        let reparsed = PdfDocument::from_bytes(compressed).unwrap();
        let pages = crate::page::collect_pages(&reparsed).unwrap();
        assert_eq!(pages.len(), 3);
    }

    /// D-T2: Font subsetting disabled → no fonts subsetted even with embedded fonts.
    #[test]
    fn test_subset_disabled() {
        let pdf = create_text_pdf(2);

        let options = CompressOptions::preset_low(); // font_subsetting: false
        assert!(!options.font_subsetting);

        let (_, stats) = compress_pdf(&pdf, &options).unwrap();
        assert_eq!(stats.fonts_subsetted, 0);
    }

    /// D-T3: Medium preset has font_subsetting enabled.
    #[test]
    fn test_subset_preset_config() {
        assert!(!CompressOptions::preset_low().font_subsetting);
        assert!(CompressOptions::preset_medium().font_subsetting);
        assert!(CompressOptions::preset_high().font_subsetting);
        assert!(CompressOptions::preset_extreme().font_subsetting);
    }

    /// Noto Sans Regular (SIL OFL 1.1): an unmodified TrueType font with `glyf` outlines.
    const NOTO_SANS_REGULAR: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/NotoSans-Regular.ttf"
    ));

    /// Create a one-page PDF that draws `text` with `ttf_bytes` embedded as a
    /// simple TrueType font.
    fn create_pdf_with_truetype_font(ttf_bytes: &[u8], text: &str) -> Vec<u8> {
        let mut doc = DocumentBuilder::new();
        let font = doc.embed_truetype_font(ttf_bytes).unwrap();
        let font_ref = doc.font_ref(&font).unwrap();

        let mut page = PageBuilder::new(612.0, 792.0);
        page.add_font_ref(&font, font_ref);
        page.begin_text();
        page.set_font(&font, 24.0);
        page.move_to(72.0, 720.0);
        page.show_text(text);
        page.end_text();
        doc.add_page(page);

        doc.build().unwrap()
    }

    /// Decoded font programs reached through FontDescriptor → FontFile2 in `pdf`.
    fn fontfile2_programs(pdf: &[u8]) -> Vec<Vec<u8>> {
        let doc = PdfDocument::from_bytes(pdf.to_vec()).unwrap();
        let mut programs = Vec::new();
        for r in doc.object_refs().collect::<Vec<_>>() {
            if let Ok(PdfObject::Dict(fd)) = doc.resolve(&r)
                && let Some(PdfObject::Reference(ff2)) = fd.get(b"FontFile2")
                && let Ok(PdfObject::Stream { dict, data }) = doc.resolve(ff2)
            {
                programs.push(doc.decode_stream(&dict, &data).unwrap());
            }
        }
        programs
    }

    /// Outline of the glyph that `face`'s `cmap` maps `c` to, as path commands.
    fn glyph_outline_for_char(face: &ttf_parser::Face, c: char) -> Option<Vec<String>> {
        struct Recorder(Vec<String>);
        impl ttf_parser::OutlineBuilder for Recorder {
            fn move_to(&mut self, x: f32, y: f32) {
                self.0.push(format!("M {x} {y}"));
            }
            fn line_to(&mut self, x: f32, y: f32) {
                self.0.push(format!("L {x} {y}"));
            }
            fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
                self.0.push(format!("Q {x1} {y1} {x} {y}"));
            }
            fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
                self.0.push(format!("C {x1} {y1} {x2} {y2} {x} {y}"));
            }
            fn close(&mut self) {
                self.0.push("Z".to_string());
            }
        }

        let gid = face.glyph_index(c)?;
        let mut recorder = Recorder(Vec::new());
        face.outline_glyph(gid, &mut recorder)?;
        Some(recorder.0)
    }

    /// The TrueType fixture is the pinned upstream file.
    #[test]
    fn test_truetype_fixture_is_pinned() {
        let hex: String = Sha256::digest(NOTO_SANS_REGULAR)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        assert_eq!(
            hex,
            "f3961a9cde016d41a4879aecda1474d3a36d6bf54fa0e4643de029cc2248b0e8"
        );
    }

    /// D-T4: An embedded TrueType font is subsetted — counted in the stats, and
    /// the embedded font program keeps fewer glyph outlines than the original.
    #[test]
    fn test_subset_truetype_font_actually_subsets() {
        let pdf = create_pdf_with_truetype_font(NOTO_SANS_REGULAR, "Hello");
        assert_eq!(fontfile2_programs(&pdf), vec![NOTO_SANS_REGULAR.to_vec()]);

        let options = CompressOptions::preset_medium();
        assert!(options.font_subsetting);

        let (compressed, stats) = compress_pdf(&pdf, &options).unwrap();
        assert_eq!(stats.fonts_subsetted, 1);

        let programs = fontfile2_programs(&compressed);
        assert_eq!(programs.len(), 1);
        let original = ttf_parser::Face::parse(NOTO_SANS_REGULAR, 0).unwrap();
        let subset = ttf_parser::Face::parse(&programs[0], 0).unwrap();
        let outlines = |face: &ttf_parser::Face| {
            (0..face.number_of_glyphs())
                .filter(|&gid| face.glyph_bounding_box(ttf_parser::GlyphId(gid)).is_some())
                .count()
        };
        assert!(
            outlines(&subset) < outlines(&original),
            "subset keeps {} of {} glyph outlines",
            outlines(&subset),
            outlines(&original)
        );

        let reparsed = PdfDocument::from_bytes(compressed).unwrap();
        let pages = crate::page::collect_pages(&reparsed).unwrap();
        assert_eq!(pages.len(), 1);
        let text = crate::text::extract_page_text_string(&reparsed, &pages[0]).unwrap();
        assert!(text.contains("Hello"), "extracted text: {text:?}");
    }

    /// D-T4b: After subsetting, every drawn character still resolves through the
    /// font's `cmap` to the outline it had in the original font.
    #[test]
    fn test_subset_truetype_font_keeps_glyph_outlines() {
        let text = "Hello";
        let pdf = create_pdf_with_truetype_font(NOTO_SANS_REGULAR, text);

        let (compressed, stats) = compress_pdf(&pdf, &CompressOptions::preset_medium()).unwrap();
        assert_eq!(stats.fonts_subsetted, 1);

        let programs = fontfile2_programs(&compressed);
        assert_eq!(programs.len(), 1);
        let original = ttf_parser::Face::parse(NOTO_SANS_REGULAR, 0).unwrap();
        let subset = ttf_parser::Face::parse(&programs[0], 0).unwrap();

        for c in text.chars() {
            let expected = glyph_outline_for_char(&original, c);
            assert!(expected.is_some(), "fixture has no outline for {c:?}");
            assert_eq!(
                glyph_outline_for_char(&subset, c),
                expected,
                "outline for {c:?} changed after subsetting"
            );
        }
    }

    /// Outline of glyph `gid` in `face`, as path commands.
    fn glyph_outline_for_gid(face: &ttf_parser::Face, gid: u16) -> Option<Vec<String>> {
        struct Recorder(Vec<String>);
        impl ttf_parser::OutlineBuilder for Recorder {
            fn move_to(&mut self, x: f32, y: f32) {
                self.0.push(format!("M {x} {y}"));
            }
            fn line_to(&mut self, x: f32, y: f32) {
                self.0.push(format!("L {x} {y}"));
            }
            fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
                self.0.push(format!("Q {x1} {y1} {x} {y}"));
            }
            fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
                self.0.push(format!("C {x1} {y1} {x2} {y2} {x} {y}"));
            }
            fn close(&mut self) {
                self.0.push("Z".to_string());
            }
        }

        let mut recorder = Recorder(Vec::new());
        face.outline_glyph(ttf_parser::GlyphId(gid), &mut recorder)?;
        Some(recorder.0)
    }

    /// A one-page PDF with Noto Sans embedded as simple TrueType font `/F1`,
    /// opened for editing.
    struct TrueTypePdf {
        modifier: DocumentModifier,
        pages: u32,
        page: u32,
        contents: u32,
        font: u32,
    }

    impl TrueTypePdf {
        fn new() -> Self {
            let pdf = create_pdf_with_truetype_font(NOTO_SANS_REGULAR, "x");
            let doc = PdfDocument::from_bytes(pdf).unwrap();
            let mut modifier = DocumentModifier::from_document(&doc).unwrap();
            let find = |modifier: &mut DocumentModifier, pred: &dyn Fn(&PdfDict) -> bool| {
                modifier
                    .writer()
                    .objects
                    .iter()
                    .find_map(|(num, obj)| match obj {
                        PdfObject::Dict(d) if pred(d) => Some(*num),
                        _ => None,
                    })
                    .unwrap()
            };
            let pages = find(&mut modifier, &|d| d.get_name(b"Type") == Some(b"Pages"));
            let page = find(&mut modifier, &|d| d.get_name(b"Type") == Some(b"Page"));
            let font = find(&mut modifier, &|d| {
                d.get_name(b"Subtype") == Some(b"TrueType")
            });
            let contents = match find_object_dict(page, &modifier).unwrap().get(b"Contents") {
                Some(PdfObject::Reference(r)) => r.obj_num,
                other => panic!("unexpected /Contents {other:?}"),
            };
            Self {
                modifier,
                pages,
                page,
                contents,
                font,
            }
        }

        fn dict(&self, obj_num: u32) -> PdfDict {
            find_object_dict(obj_num, &self.modifier).unwrap()
        }

        fn set_dict(&mut self, obj_num: u32, dict: PdfDict) {
            self.modifier.set_object(obj_num, PdfObject::Dict(dict));
        }

        fn set_content(&mut self, content: &[u8]) {
            let (dict, data) = crate::writer::encode::make_stream(content, false);
            self.modifier
                .set_object(self.contents, PdfObject::Stream { dict, data });
        }

        fn font_ref(&self) -> PdfObject {
            PdfObject::Reference(IndirectRef {
                obj_num: self.font,
                gen_num: 0,
            })
        }

        fn build(self) -> Vec<u8> {
            self.modifier.build().unwrap()
        }
    }

    /// Compress `pdf` with font subsetting on; returns the stats and the
    /// original and output font programs.
    fn compress_and_fonts(pdf: &[u8]) -> (CompressStats, Vec<u8>, Vec<u8>) {
        let before = fontfile2_programs(pdf);
        assert_eq!(before.len(), 1);
        let (compressed, stats) = compress_pdf(pdf, &CompressOptions::preset_medium()).unwrap();
        let after = fontfile2_programs(&compressed);
        assert_eq!(after.len(), 1);
        (stats, before[0].clone(), after[0].clone())
    }

    /// Assert that `c`, looked up through each font's own `cmap`, has the same
    /// outline in `subset` as in `original`.
    fn assert_char_outline_kept(original: &[u8], subset: &[u8], c: char) {
        let original = ttf_parser::Face::parse(original, 0).unwrap();
        let subset = ttf_parser::Face::parse(subset, 0).unwrap();
        let expected = glyph_outline_for_char(&original, c);
        assert!(expected.is_some(), "fixture has no outline for {c:?}");
        assert_eq!(
            glyph_outline_for_char(&subset, c),
            expected,
            "outline for {c:?}"
        );
    }

    #[test]
    fn test_subset_differences_keeps_glyph_of_mapped_name() {
        let mut pdf = TrueTypePdf::new();
        let mut font = pdf.dict(pdf.font);
        let mut encoding = PdfDict::new();
        encoding.insert(
            b"BaseEncoding".to_vec(),
            PdfObject::Name(b"WinAnsiEncoding".to_vec()),
        );
        encoding.insert(
            b"Differences".to_vec(),
            PdfObject::Array(vec![PdfObject::Integer(72), PdfObject::Name(b"o".to_vec())]),
        );
        font.insert(b"Encoding".to_vec(), PdfObject::Dict(encoding));
        pdf.set_dict(pdf.font, font);
        pdf.set_content(b"BT /F1 24 Tf 72 720 Td (H) Tj ET");

        let (stats, original, subset) = compress_and_fonts(&pdf.build());
        assert_eq!(stats.fonts_subsetted, 1);
        assert_char_outline_kept(&original, &subset, 'o');
    }

    #[test]
    fn test_subset_skips_font_with_unresolvable_glyph_name() {
        let mut pdf = TrueTypePdf::new();
        let mut font = pdf.dict(pdf.font);
        let mut encoding = PdfDict::new();
        encoding.insert(
            b"Differences".to_vec(),
            PdfObject::Array(vec![
                PdfObject::Integer(72),
                PdfObject::Name(b"notAGlyphInThisFont".to_vec()),
            ]),
        );
        font.insert(b"Encoding".to_vec(), PdfObject::Dict(encoding));
        pdf.set_dict(pdf.font, font);
        pdf.set_content(b"BT /F1 24 Tf 72 720 Td (H) Tj ET");

        let (stats, original, subset) = compress_and_fonts(&pdf.build());
        assert_eq!(stats.fonts_subsetted, 0);
        assert_eq!(subset, original);
    }

    #[test]
    fn test_subset_winansi_high_code_keeps_glyph() {
        let mut pdf = TrueTypePdf::new();
        pdf.set_content(b"BT /F1 24 Tf 72 720 Td (H) Tj <E980> Tj ET");

        let (stats, original, subset) = compress_and_fonts(&pdf.build());
        assert_eq!(stats.fonts_subsetted, 1);
        assert_char_outline_kept(&original, &subset, '\u{e9}');
        assert_char_outline_kept(&original, &subset, '\u{20ac}');
    }

    #[test]
    fn test_subset_skips_high_code_outside_winansi() {
        let mut pdf = TrueTypePdf::new();
        let mut font = pdf.dict(pdf.font);
        font.insert(
            b"Encoding".to_vec(),
            PdfObject::Name(b"MacRomanEncoding".to_vec()),
        );
        pdf.set_dict(pdf.font, font);
        pdf.set_content(b"BT /F1 24 Tf 72 720 Td (H) Tj <8E> Tj ET");

        let (stats, original, subset) = compress_and_fonts(&pdf.build());
        assert_eq!(stats.fonts_subsetted, 0);
        assert_eq!(subset, original);
    }

    #[test]
    fn test_subset_skips_font_used_by_form_xobject() {
        let mut pdf = TrueTypePdf::new();
        let mut xobject_fonts = PdfDict::new();
        xobject_fonts.insert(b"F1".to_vec(), pdf.font_ref());
        let mut xobject_resources = PdfDict::new();
        xobject_resources.insert(b"Font".to_vec(), PdfObject::Dict(xobject_fonts));
        let (mut form, data) = crate::writer::encode::make_stream(b"BT /F1 24 Tf (o) Tj ET", false);
        form.insert(b"Type".to_vec(), PdfObject::Name(b"XObject".to_vec()));
        form.insert(b"Subtype".to_vec(), PdfObject::Name(b"Form".to_vec()));
        form.insert(
            b"BBox".to_vec(),
            PdfObject::Array(vec![
                PdfObject::Integer(0),
                PdfObject::Integer(0),
                PdfObject::Integer(612),
                PdfObject::Integer(792),
            ]),
        );
        form.insert(b"Resources".to_vec(), PdfObject::Dict(xobject_resources));
        let form_ref = pdf
            .modifier
            .add_object(PdfObject::Stream { dict: form, data });

        let mut page = pdf.dict(pdf.page);
        let mut resources = match page.get(b"Resources") {
            Some(PdfObject::Dict(d)) => d.clone(),
            other => panic!("unexpected /Resources {other:?}"),
        };
        let mut xobjects = PdfDict::new();
        xobjects.insert(b"X1".to_vec(), PdfObject::Reference(form_ref));
        resources.insert(b"XObject".to_vec(), PdfObject::Dict(xobjects));
        page.insert(b"Resources".to_vec(), PdfObject::Dict(resources));
        pdf.set_dict(pdf.page, page);
        pdf.set_content(b"BT /F1 24 Tf 72 720 Td (H) Tj ET /X1 Do");

        let (stats, original, subset) = compress_and_fonts(&pdf.build());
        assert_eq!(stats.fonts_subsetted, 0);
        assert_eq!(subset, original);
    }

    #[test]
    fn test_subset_font_in_indirect_page_resources_is_subsetted() {
        let mut pdf = TrueTypePdf::new();
        let mut page = pdf.dict(pdf.page);
        let mut resources = match page.remove(b"Resources") {
            Some(PdfObject::Dict(d)) => d,
            other => panic!("unexpected /Resources {other:?}"),
        };
        let fonts = resources.remove(b"Font").unwrap();
        let fonts_ref = pdf.modifier.add_object(fonts);
        resources.insert(b"Font".to_vec(), PdfObject::Reference(fonts_ref));
        let resources_ref = pdf.modifier.add_object(PdfObject::Dict(resources));
        page.insert(b"Resources".to_vec(), PdfObject::Reference(resources_ref));
        pdf.set_dict(pdf.page, page);
        pdf.set_content(b"BT /F1 24 Tf 72 720 Td (Hello) Tj ET");

        let (stats, original, subset) = compress_and_fonts(&pdf.build());
        assert_eq!(stats.fonts_subsetted, 1);
        assert_char_outline_kept(&original, &subset, 'H');
    }

    /// Page 1 reaches `/F1` only through `/Resources` inherited from the page
    /// tree and draws "o"; page 2 has its own `/Resources` and draws "H" with the
    /// same font object.
    #[test]
    fn test_subset_skips_font_in_inherited_resources() {
        let mut pdf = TrueTypePdf::new();
        let mut page = pdf.dict(pdf.page);
        let resources = page.remove(b"Resources").unwrap();
        pdf.set_dict(pdf.page, page.clone());
        pdf.set_content(b"BT /F1 24 Tf 72 720 Td (o) Tj ET");

        let (content_dict, content_data) =
            crate::writer::encode::make_stream(b"BT /F1 24 Tf 72 720 Td (H) Tj ET", false);
        let content2 = pdf.modifier.add_object(PdfObject::Stream {
            dict: content_dict,
            data: content_data,
        });
        let mut page2 = page;
        page2.insert(b"Contents".to_vec(), PdfObject::Reference(content2));
        page2.insert(b"Resources".to_vec(), resources.clone());
        let page2_ref = pdf.modifier.add_object(PdfObject::Dict(page2));

        let mut pages = pdf.dict(pdf.pages);
        pages.insert(b"Resources".to_vec(), resources);
        let mut kids = match pages.get(b"Kids") {
            Some(PdfObject::Array(k)) => k.clone(),
            other => panic!("unexpected /Kids {other:?}"),
        };
        kids.push(PdfObject::Reference(page2_ref));
        pages.insert(b"Kids".to_vec(), PdfObject::Array(kids));
        pages.insert(b"Count".to_vec(), PdfObject::Integer(2));
        pdf.set_dict(pdf.pages, pages);

        let (stats, original, subset) = compress_and_fonts(&pdf.build());
        assert_eq!(stats.fonts_subsetted, 0);
        assert_eq!(subset, original);
    }

    #[test]
    fn test_subset_skips_font_shown_by_quote_operator() {
        let mut pdf = TrueTypePdf::new();
        pdf.set_content(b"BT /F1 24 Tf 72 720 Td 30 TL (H) Tj 0 0 (o) \" ET");

        let (stats, original, subset) = compress_and_fonts(&pdf.build());
        assert_eq!(stats.fonts_subsetted, 0);
        assert_eq!(subset, original);
    }

    #[test]
    fn test_subset_font_restored_by_grestore_keeps_glyphs() {
        let mut doc = DocumentBuilder::new();
        let f1 = doc.embed_truetype_font(NOTO_SANS_REGULAR).unwrap();
        let f1_ref = doc.font_ref(&f1).unwrap();
        let f2 = doc.embed_truetype_font(NOTO_SANS_REGULAR).unwrap();
        let f2_ref = doc.font_ref(&f2).unwrap();
        let mut page = PageBuilder::new(612.0, 792.0);
        page.add_font_ref(&f1, f1_ref);
        page.add_font_ref(&f2, f2_ref);
        doc.add_page(page);
        let pdf = doc.build().unwrap();

        let doc = PdfDocument::from_bytes(pdf).unwrap();
        let mut modifier = DocumentModifier::from_document(&doc).unwrap();
        let contents = modifier
            .writer()
            .objects
            .iter()
            .find_map(|(_, obj)| match obj {
                PdfObject::Dict(d) if d.get_name(b"Type") == Some(b"Page") => {
                    match d.get(b"Contents") {
                        Some(PdfObject::Reference(r)) => Some(r.obj_num),
                        _ => None,
                    }
                }
                _ => None,
            })
            .unwrap();
        let content = format!(
            "BT /{f1} 24 Tf 72 720 Td (e) Tj ET q BT /{f2} 24 Tf 72 690 Td (o) Tj ET Q BT 72 660 Td (H) Tj ET"
        );
        let (dict, data) = crate::writer::encode::make_stream(content.as_bytes(), false);
        modifier.set_object(contents, PdfObject::Stream { dict, data });
        let pdf = modifier.build().unwrap();

        let (compressed, stats) = compress_pdf(&pdf, &CompressOptions::preset_medium()).unwrap();
        assert_eq!(stats.fonts_subsetted, 2);
        let reparsed = PdfDocument::from_bytes(compressed.clone()).unwrap();
        let pages = crate::page::collect_pages(&reparsed).unwrap();
        let page_dict = match reparsed.resolve(&pages[0].page_ref) {
            Ok(PdfObject::Dict(d)) => d,
            other => panic!("unexpected page {other:?}"),
        };
        let font_dict = match page_dict.get(b"Resources") {
            Some(PdfObject::Dict(r)) => match r.get(b"Font") {
                Some(PdfObject::Dict(f)) => f.clone(),
                other => panic!("unexpected /Font {other:?}"),
            },
            other => panic!("unexpected /Resources {other:?}"),
        };
        let program = |name: &str| -> Vec<u8> {
            let font = match font_dict.get(name.as_bytes()) {
                Some(PdfObject::Reference(r)) => r.clone(),
                other => panic!("unexpected font {other:?}"),
            };
            let resolve_dict = |r: &IndirectRef| match reparsed.resolve(r) {
                Ok(PdfObject::Dict(d)) => d,
                other => panic!("unexpected {other:?}"),
            };
            let fd = match resolve_dict(&font).get(b"FontDescriptor") {
                Some(PdfObject::Reference(r)) => r.clone(),
                other => panic!("unexpected /FontDescriptor {other:?}"),
            };
            let ff2 = match resolve_dict(&fd).get(b"FontFile2") {
                Some(PdfObject::Reference(r)) => r.clone(),
                other => panic!("unexpected /FontFile2 {other:?}"),
            };
            match reparsed.resolve(&ff2) {
                Ok(PdfObject::Stream { dict, data }) => {
                    reparsed.decode_stream(&dict, &data).unwrap()
                }
                other => panic!("unexpected font file {other:?}"),
            }
        };
        assert_char_outline_kept(NOTO_SANS_REGULAR, &program(&f1), 'H');
        assert_char_outline_kept(NOTO_SANS_REGULAR, &program(&f1), 'e');
        assert_char_outline_kept(NOTO_SANS_REGULAR, &program(&f2), 'o');
    }

    /// Create a one-page PDF that draws the glyphs of `text` with Noto Sans
    /// embedded as an Identity-H CIDFontType2 font; `edit_cid_font` may change
    /// the descendant CIDFont dictionary, and `content_cids` maps each glyph ID
    /// to the CID written in the content stream.
    fn create_pdf_with_cid_font(
        text: &str,
        content_cids: impl Fn(u16) -> u16,
        edit_cid_font: impl FnOnce(&mut PdfDict, &mut crate::writer::PdfWriter),
        encoding: &[u8],
    ) -> Vec<u8> {
        let face = ttf_parser::Face::parse(NOTO_SANS_REGULAR, 0).unwrap();
        let used: Vec<(char, u16)> = text
            .chars()
            .map(|c| (c, face.glyph_index(c).unwrap().0))
            .collect();

        let mut writer = crate::writer::PdfWriter::new();
        let pages_num = writer.alloc_object_num();
        let font_ref =
            crate::font::cjk::build_cid_font(&mut writer, "NotoSans", NOTO_SANS_REGULAR, &used)
                .unwrap();

        let type0 = writer
            .objects
            .iter()
            .find_map(|(num, obj)| match obj {
                PdfObject::Dict(d) if *num == font_ref.obj_num => Some(d.clone()),
                _ => None,
            })
            .unwrap();
        let mut type0 = type0;
        type0.insert(b"Encoding".to_vec(), PdfObject::Name(encoding.to_vec()));
        writer.set_object(font_ref.obj_num, PdfObject::Dict(type0.clone()));
        let cid_font_num = match type0.get(b"DescendantFonts") {
            Some(PdfObject::Array(arr)) => match arr.first() {
                Some(PdfObject::Reference(r)) => r.obj_num,
                other => panic!("unexpected descendant {other:?}"),
            },
            other => panic!("unexpected /DescendantFonts {other:?}"),
        };
        let mut cid_font = writer
            .objects
            .iter()
            .find_map(|(num, obj)| match obj {
                PdfObject::Dict(d) if *num == cid_font_num => Some(d.clone()),
                _ => None,
            })
            .unwrap();
        edit_cid_font(&mut cid_font, &mut writer);
        writer.set_object(cid_font_num, PdfObject::Dict(cid_font));

        let hex: String = used
            .iter()
            .map(|&(_, gid)| format!("{:04X}", content_cids(gid)))
            .collect();
        let content = format!("BT /F1 24 Tf 72 720 Td <{hex}> Tj ET");
        let (content_dict, content_data) =
            crate::writer::encode::make_stream(content.as_bytes(), false);
        let content_ref = writer.add_object(PdfObject::Stream {
            dict: content_dict,
            data: content_data,
        });

        let pages_ref = IndirectRef {
            obj_num: pages_num,
            gen_num: 0,
        };
        let mut fonts = PdfDict::new();
        fonts.insert(b"F1".to_vec(), PdfObject::Reference(font_ref));
        let mut resources = PdfDict::new();
        resources.insert(b"Font".to_vec(), PdfObject::Dict(fonts));
        let mut page = PdfDict::new();
        page.insert(b"Type".to_vec(), PdfObject::Name(b"Page".to_vec()));
        page.insert(b"Parent".to_vec(), PdfObject::Reference(pages_ref.clone()));
        page.insert(
            b"MediaBox".to_vec(),
            PdfObject::Array(vec![
                PdfObject::Integer(0),
                PdfObject::Integer(0),
                PdfObject::Integer(612),
                PdfObject::Integer(792),
            ]),
        );
        page.insert(b"Contents".to_vec(), PdfObject::Reference(content_ref));
        page.insert(b"Resources".to_vec(), PdfObject::Dict(resources));
        let page_ref = writer.add_object(PdfObject::Dict(page));

        let mut pages = PdfDict::new();
        pages.insert(b"Type".to_vec(), PdfObject::Name(b"Pages".to_vec()));
        pages.insert(
            b"Kids".to_vec(),
            PdfObject::Array(vec![PdfObject::Reference(page_ref)]),
        );
        pages.insert(b"Count".to_vec(), PdfObject::Integer(1));
        writer.set_object(pages_num, PdfObject::Dict(pages));

        let mut catalog = PdfDict::new();
        catalog.insert(b"Type".to_vec(), PdfObject::Name(b"Catalog".to_vec()));
        catalog.insert(b"Pages".to_vec(), PdfObject::Reference(pages_ref));
        let catalog_ref = writer.add_object(PdfObject::Dict(catalog));
        writer.write_to_bytes(&catalog_ref).unwrap()
    }

    /// Assert that every glyph of `text` keeps its outline at its glyph ID.
    fn assert_gid_outlines_kept(original: &[u8], subset: &[u8], text: &str) {
        let original = ttf_parser::Face::parse(original, 0).unwrap();
        let subset = ttf_parser::Face::parse(subset, 0).unwrap();
        for c in text.chars() {
            let gid = original.glyph_index(c).unwrap().0;
            let expected = glyph_outline_for_gid(&original, gid);
            assert!(expected.is_some(), "fixture has no outline for {c:?}");
            assert_eq!(
                glyph_outline_for_gid(&subset, gid),
                expected,
                "outline for {c:?}"
            );
        }
    }

    #[test]
    fn test_subset_cid_identity_keeps_glyph_outlines() {
        let pdf = create_pdf_with_cid_font("Hello", |gid| gid, |_, _| {}, b"Identity-H");

        let (stats, original, subset) = compress_and_fonts(&pdf);
        assert_eq!(stats.fonts_subsetted, 1);
        assert!(subset.len() < original.len());
        assert_gid_outlines_kept(&original, &subset, "Hello");
    }

    #[test]
    fn test_subset_cid_to_gid_map_stream_keeps_glyph_outlines() {
        let face = ttf_parser::Face::parse(NOTO_SANS_REGULAR, 0).unwrap();
        let gids: Vec<u16> = "Helo"
            .chars()
            .map(|c| face.glyph_index(c).unwrap().0)
            .collect();
        let cid_of = move |gid: u16| gids.iter().position(|&g| g == gid).unwrap() as u16 + 1;
        let face_gids: Vec<u16> = "Helo"
            .chars()
            .map(|c| face.glyph_index(c).unwrap().0)
            .collect();
        let pdf = create_pdf_with_cid_font(
            "Hello",
            cid_of,
            move |cid_font, writer| {
                let mut map = vec![0u8; 2 * (face_gids.len() + 1)];
                for (i, gid) in face_gids.iter().enumerate() {
                    map[2 * (i + 1)..2 * (i + 2)].copy_from_slice(&gid.to_be_bytes());
                }
                let (dict, data) = crate::writer::encode::make_stream(&map, true);
                let map_ref = writer.add_object(PdfObject::Stream { dict, data });
                cid_font.insert(b"CIDToGIDMap".to_vec(), PdfObject::Reference(map_ref));
            },
            b"Identity-H",
        );

        let (stats, original, subset) = compress_and_fonts(&pdf);
        assert_eq!(stats.fonts_subsetted, 1);
        assert_gid_outlines_kept(&original, &subset, "Hello");
    }

    #[test]
    fn test_subset_skips_cid_font_with_non_identity_encoding() {
        let pdf = create_pdf_with_cid_font("Hello", |gid| gid, |_, _| {}, b"UniGB-UCS2-H");

        let (stats, original, subset) = compress_and_fonts(&pdf);
        assert_eq!(stats.fonts_subsetted, 0);
        assert_eq!(subset, original);
    }

    /// D-T5: CFF font should be skipped (subset_font returns None for CFF).
    #[test]
    fn test_subset_skips_non_truetype() {
        // Standard fonts are Type1, not TrueType → find_fontfile2 returns None
        let pdf = create_text_pdf(1);

        let options = CompressOptions::preset_high();
        let (compressed, stats) = compress_pdf(&pdf, &options).unwrap();

        // No TrueType fonts to subset
        assert_eq!(stats.fonts_subsetted, 0);
        assert!(compressed.starts_with(b"%PDF"));
    }

    // ── Phase E: Unused resource removal tests ──────────────────────

    /// Helper: create a PDF with an extra unused font in Resources.
    fn create_pdf_with_unused_font() -> Vec<u8> {
        let mut doc = DocumentBuilder::new();
        let font1 = doc.add_standard_font("Helvetica");
        let _font2 = doc.add_standard_font("Courier"); // added but not used in content

        let mut page = PageBuilder::new(612.0, 792.0);
        page.add_font(&font1, "Helvetica");
        // Note: font2 is NOT added to the page, so it won't be in Resources.
        // To truly test this, we need to manually add an unused font to Resources.
        page.begin_text();
        page.set_font(&font1, 12.0);
        page.move_to(72.0, 720.0);
        page.show_text("Only using Helvetica");
        page.end_text();
        doc.add_page(page);

        doc.build().unwrap()
    }

    /// E-T1: Unused resource removal doesn't break valid PDF.
    #[test]
    fn test_remove_unused_resources_valid() {
        let pdf = create_text_pdf(3);

        let options = CompressOptions::preset_medium();
        let (compressed, stats) = compress_pdf(&pdf, &options).unwrap();

        assert!(compressed.starts_with(b"%PDF"));

        // Re-parse to verify validity
        let reparsed = PdfDocument::from_bytes(compressed).unwrap();
        let pages = crate::page::collect_pages(&reparsed).unwrap();
        assert_eq!(pages.len(), 3);
    }

    /// E-T2: Text extraction intact after unused resource removal.
    #[test]
    fn test_remove_unused_resources_text_preserved() {
        let pdf = create_text_pdf(2);

        let options = CompressOptions::preset_medium();
        let (compressed, _) = compress_pdf(&pdf, &options).unwrap();

        let reparsed = PdfDocument::from_bytes(compressed).unwrap();
        let pages = crate::page::collect_pages(&reparsed).unwrap();

        for (i, page) in pages.iter().enumerate() {
            let text = crate::text::extract_page_text_string(&reparsed, page).unwrap_or_default();
            assert!(
                text.contains(&format!("Test page {}", i + 1)),
                "Page {} text should be preserved",
                i + 1,
            );
        }
    }

    /// E-T3: Remove unused resources disabled for low preset.
    #[test]
    fn test_remove_unused_resources_disabled_low() {
        let pdf = create_text_pdf(1);
        let options = CompressOptions::preset_low();
        assert!(!options.remove_unused_resources);

        let (_, stats) = compress_pdf(&pdf, &options).unwrap();
        assert_eq!(stats.unused_resources_removed, 0);
    }

    /// E-T4: PDF with images — used images preserved after resource cleanup.
    #[test]
    fn test_remove_unused_resources_images_intact() {
        let pdf = create_pdf_with_jpeg(100, 100, 90);

        let options = CompressOptions::preset_medium();
        let (compressed, _) = compress_pdf(&pdf, &options).unwrap();

        let reparsed = PdfDocument::from_bytes(compressed.clone()).unwrap();
        let pages = crate::page::collect_pages(&reparsed).unwrap();
        assert_eq!(pages.len(), 1);

        // Image should still be there
        let analysis = analyze_pdf(&compressed).unwrap();
        assert!(analysis.images >= 1);
    }

    // ── Phase F: Metadata/structure removal tests ───────────────────

    /// F-T1: Low preset — no metadata stripped.
    #[test]
    fn test_strip_metadata_low_noop() {
        let pdf = create_text_pdf(1);
        let options = CompressOptions::preset_low();
        assert!(!options.strip_metadata);
        assert!(!options.strip_extras);

        let (_, stats) = compress_pdf(&pdf, &options).unwrap();
        assert_eq!(stats.metadata_items_stripped, 0);
    }

    /// F-T2: High preset — metadata stripped, output valid.
    #[test]
    fn test_strip_metadata_high_valid() {
        let pdf = create_text_pdf(3);
        let options = CompressOptions::preset_high();
        assert!(options.strip_metadata);

        let (compressed, _) = compress_pdf(&pdf, &options).unwrap();
        assert!(compressed.starts_with(b"%PDF"));

        let reparsed = PdfDocument::from_bytes(compressed).unwrap();
        let pages = crate::page::collect_pages(&reparsed).unwrap();
        assert_eq!(pages.len(), 3);
    }

    /// F-T3: Extreme preset — extras stripped too, output valid.
    #[test]
    fn test_strip_extras_extreme_valid() {
        let pdf = create_text_pdf(2);
        let options = CompressOptions::preset_extreme();
        assert!(options.strip_metadata);
        assert!(options.strip_extras);

        let (compressed, _) = compress_pdf(&pdf, &options).unwrap();
        assert!(compressed.starts_with(b"%PDF"));

        let reparsed = PdfDocument::from_bytes(compressed).unwrap();
        let pages = crate::page::collect_pages(&reparsed).unwrap();
        assert_eq!(pages.len(), 2);
    }

    /// F-T4: Text preserved after metadata stripping.
    #[test]
    fn test_strip_metadata_text_preserved() {
        let pdf = create_text_pdf(2);
        let options = CompressOptions::preset_extreme();

        let (compressed, _) = compress_pdf(&pdf, &options).unwrap();

        let reparsed = PdfDocument::from_bytes(compressed).unwrap();
        let pages = crate::page::collect_pages(&reparsed).unwrap();

        for (i, page) in pages.iter().enumerate() {
            let text = crate::text::extract_page_text_string(&reparsed, page).unwrap_or_default();
            assert!(
                text.contains(&format!("Test page {}", i + 1)),
                "Text should survive metadata stripping"
            );
        }
    }

    /// F-T5: Preset strip_metadata/strip_extras config is correct.
    #[test]
    fn test_strip_preset_config() {
        assert!(!CompressOptions::preset_low().strip_metadata);
        assert!(!CompressOptions::preset_low().strip_extras);
        assert!(!CompressOptions::preset_medium().strip_metadata);
        assert!(!CompressOptions::preset_medium().strip_extras);
        assert!(CompressOptions::preset_high().strip_metadata);
        assert!(!CompressOptions::preset_high().strip_extras);
        assert!(CompressOptions::preset_extreme().strip_metadata);
        assert!(CompressOptions::preset_extreme().strip_extras);
    }

    /// F-T6: Image PDF with high preset — images preserved after stripping.
    #[test]
    fn test_strip_metadata_images_preserved() {
        let pdf = create_pdf_with_jpeg(100, 100, 90);
        let options = CompressOptions::preset_high();

        let (compressed, _) = compress_pdf(&pdf, &options).unwrap();

        let analysis = analyze_pdf(&compressed).unwrap();
        assert!(analysis.images >= 1);
        assert_eq!(analysis.pages, 1);
    }

    // ── Phase G: Grayscale conversion tests ─────────────────────────

    /// G-T1: RGB image → grayscale, size reduced.
    #[test]
    fn test_grayscale_conversion_reduces_size() {
        let pdf = create_pdf_with_jpeg(200, 200, 95);
        let original_size = pdf.len();

        let mut options = CompressOptions::preset_extreme();
        options.grayscale = true;

        let (compressed, stats) = compress_pdf(&pdf, &options).unwrap();

        assert!(compressed.starts_with(b"%PDF"));
        if stats.images_grayscaled > 0 {
            assert!(
                compressed.len() < original_size,
                "Grayscale should reduce size: {} >= {}",
                compressed.len(),
                original_size,
            );
        }
    }

    /// G-T2: Grayscale conversion output is valid and re-parseable.
    #[test]
    fn test_grayscale_roundtrip_valid() {
        let pdf = create_pdf_with_jpeg(100, 100, 90);

        let mut options = CompressOptions::preset_medium();
        options.grayscale = true;

        let (compressed, _) = compress_pdf(&pdf, &options).unwrap();

        let reparsed = PdfDocument::from_bytes(compressed).unwrap();
        let pages = crate::page::collect_pages(&reparsed).unwrap();
        assert_eq!(pages.len(), 1);
    }

    /// G-T3: grayscale=false → no conversion.
    #[test]
    fn test_grayscale_disabled() {
        let pdf = create_pdf_with_jpeg(100, 100, 90);

        let options = CompressOptions::preset_extreme(); // grayscale defaults to false
        assert!(!options.grayscale);

        let (_, stats) = compress_pdf(&pdf, &options).unwrap();
        assert_eq!(stats.images_grayscaled, 0);
    }

    /// G-T4: Text-only PDF with grayscale=true → no crash, no changes.
    #[test]
    fn test_grayscale_text_only_noop() {
        let pdf = create_text_pdf(2);

        let mut options = CompressOptions::preset_low();
        options.grayscale = true;

        let (compressed, stats) = compress_pdf(&pdf, &options).unwrap();

        assert!(compressed.starts_with(b"%PDF"));
        assert_eq!(stats.images_grayscaled, 0);
    }

    // ── Phase H: Object stream packing tests ────────────────────────
    // NOTE: Object stream packing is implemented (pack_object_streams) but is
    // disabled in compress_pdf (see Step 5: xref stream viewer compatibility).
    // These tests verify the packing function works in isolation.

    /// H-T1: pack_object_streams correctly packs eligible objects.
    #[test]
    fn test_pack_object_streams_basic() {
        use crate::object::IndirectRef;

        let objects = vec![
            (1, PdfObject::Dict({
                let mut d = PdfDict::new();
                d.insert(b"Type".to_vec(), PdfObject::Name(b"Catalog".to_vec()));
                d.insert(b"Pages".to_vec(), PdfObject::Reference(IndirectRef { obj_num: 2, gen_num: 0 }));
                d
            })),
            (2, PdfObject::Dict({
                let mut d = PdfDict::new();
                d.insert(b"Type".to_vec(), PdfObject::Name(b"Pages".to_vec()));
                d
            })),
            (3, PdfObject::Integer(42)),  // eligible
            (4, PdfObject::String(b"hello".to_vec())),  // eligible
            (5, PdfObject::Integer(99)),  // eligible
        ];

        let packed = crate::writer::object_stream::pack_object_streams(
            &objects, 100, 1, Some(2), None
        ).unwrap();

        // Catalog (1) and Pages (2) should remain unpacked
        // Objects 3, 4, 5 should be packed into an object stream
        assert!(packed.objects.iter().any(|(n, _)| *n == 1)); // catalog
        assert!(packed.objects.iter().any(|(n, _)| *n == 2)); // pages
        // Should have fewer individual objects (3 packed into 1 stream)
        assert!(packed.objects.len() < objects.len());
    }

    /// H-T2: Object stream has correct /Type /ObjStm metadata.
    #[test]
    fn test_pack_object_streams_metadata() {
        use crate::object::IndirectRef;

        let objects = vec![
            (1, PdfObject::Dict({
                let mut d = PdfDict::new();
                d.insert(b"Type".to_vec(), PdfObject::Name(b"Catalog".to_vec()));
                d
            })),
            (2, PdfObject::Integer(42)),
            (3, PdfObject::Integer(99)),
        ];

        let packed = crate::writer::object_stream::pack_object_streams(
            &objects, 100, 1, None, None
        ).unwrap();

        // Find the object stream
        let objstm = packed.objects.iter().find(|(_, obj)| {
            if let PdfObject::Stream { dict, .. } = obj {
                dict.get_name(b"Type") == Some(b"ObjStm")
            } else {
                false
            }
        });
        assert!(objstm.is_some(), "Should have created an ObjStm");

        if let Some((_, PdfObject::Stream { dict, .. })) = objstm {
            assert_eq!(dict.get_name(b"Type"), Some(b"ObjStm".as_slice()));
            assert!(dict.get_i64(b"N").unwrap() >= 2); // at least 2 packed objects
            assert!(dict.get_i64(b"First").is_some());
        }
    }

    /// H-T3: Streams are NOT eligible for packing.
    #[test]
    fn test_pack_object_streams_skips_streams() {
        use crate::object::IndirectRef;

        let objects = vec![
            (1, PdfObject::Dict({
                let mut d = PdfDict::new();
                d.insert(b"Type".to_vec(), PdfObject::Name(b"Catalog".to_vec()));
                d
            })),
            (2, PdfObject::Stream {
                dict: PdfDict::new(),
                data: vec![1, 2, 3],
            }),
        ];

        let packed = crate::writer::object_stream::pack_object_streams(
            &objects, 100, 1, None, None
        ).unwrap();

        // Both objects should remain (stream is ineligible, catalog is excluded)
        assert_eq!(packed.objects.len(), 2);
    }

    // ── Phase J: DPI precision (CTM-based) tests ────────────────────

    /// J-T1: CTM-based DPI calculation with known display size.
    #[test]
    fn test_ctm_dpi_calculation() {
        let decoded = image::DecodedImage {
            width: 4000,
            height: 4000,
            components: 3,
            bpc: 8,
            data: vec![],
            source_format: image::ImageFormat::Raw,
        };
        // Image displayed at 200x200 pt → DPI = 4000 / (200/72) = 1440
        let (w, h, scaled) =
            compute_target_dimensions_with_ctm(&decoded, Some(150.0), Some((200.0, 200.0)));

        assert!(scaled, "Should downscale from 1440 DPI to 150 DPI");
        // Expected scale = 150/1440 ≈ 0.104
        assert!(w < 4000);
        assert!(h < 4000);
        // At 150 DPI target: pixels ≈ 150 * (200/72) ≈ 417
        assert!(w > 300 && w < 600, "Width should be ~417, got {}", w);
    }

    /// J-T2: Full-page image DPI calculation.
    #[test]
    fn test_ctm_full_page_image() {
        let decoded = image::DecodedImage {
            width: 2550,  // 300 DPI * 8.5 inches
            height: 3300, // 300 DPI * 11 inches
            components: 3,
            bpc: 8,
            data: vec![],
            source_format: image::ImageFormat::Raw,
        };
        // Full page: 612x792 pt (8.5x11 in)
        let (w, h, scaled) =
            compute_target_dimensions_with_ctm(&decoded, Some(150.0), Some((612.0, 792.0)));

        assert!(scaled, "Should downscale from 300 DPI to 150 DPI");
        // At 150 DPI: w ≈ 1275, h ≈ 1650
        assert!(w < 2550 && w > 1000, "Width should be ~1275, got {}", w);
        assert!(h < 3300 && h > 1200, "Height should be ~1650, got {}", h);
    }

    /// J-T3: Image already within DPI budget → no downscale.
    #[test]
    fn test_ctm_within_budget() {
        let decoded = image::DecodedImage {
            width: 100,
            height: 100,
            components: 3,
            bpc: 8,
            data: vec![],
            source_format: image::ImageFormat::Raw,
        };
        // 100px displayed at 100pt → DPI = 100 / (100/72) = 72
        let (w, h, scaled) =
            compute_target_dimensions_with_ctm(&decoded, Some(150.0), Some((100.0, 100.0)));

        assert!(!scaled, "72 DPI is within 150 DPI budget");
        assert_eq!(w, 100);
        assert_eq!(h, 100);
    }

    /// J-T4: No display size → fallback to pixel-budget heuristic.
    #[test]
    fn test_ctm_fallback_no_display_size() {
        let decoded = image::DecodedImage {
            width: 1000,
            height: 500,
            components: 3,
            bpc: 8,
            data: vec![],
            source_format: image::ImageFormat::Raw,
        };
        // Without CTM, fallback should match old behavior
        let (w1, h1, _) = compute_target_dimensions_with_ctm(&decoded, Some(150.0), None);
        let (w2, h2, _) = compute_target_dimensions(&decoded, Some(150.0));
        assert_eq!(w1, w2);
        assert_eq!(h1, h2);
    }

    /// J-T5: Matrix multiplication correctness.
    #[test]
    fn test_multiply_matrix() {
        // Identity * anything = anything
        let identity = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
        let m = [2.0, 0.0, 0.0, 3.0, 10.0, 20.0];
        let result = multiply_matrix(&m, &identity);
        assert!((result[0] - 2.0).abs() < 1e-10);
        assert!((result[3] - 3.0).abs() < 1e-10);
        assert!((result[4] - 10.0).abs() < 1e-10);
        assert!((result[5] - 20.0).abs() < 1e-10);
    }

    // ── Serialization safety: special characters in names ───────────

    /// Regression test: font names with spaces must survive compress roundtrip.
    /// This was the bug that caused Korean text to disappear —
    /// "Pretendard Black" was serialized as `/Pretendard Black` instead of
    /// `/Pretendard#20Black`, breaking the parser.
    #[test]
    fn test_compress_font_name_with_space_roundtrip() {
        use crate::object::IndirectRef;

        // Build a minimal PDF with a font whose BaseFont contains a space
        let mut writer = crate::writer::PdfWriter::new();

        // Font dict with space in name
        let mut font_dict = PdfDict::new();
        font_dict.insert(b"Type".to_vec(), PdfObject::Name(b"Font".to_vec()));
        font_dict.insert(b"Subtype".to_vec(), PdfObject::Name(b"Type1".to_vec()));
        font_dict.insert(
            b"BaseFont".to_vec(),
            PdfObject::Name(b"Pretendard Black".to_vec()),
        );
        let font_ref = writer.add_object(PdfObject::Dict(font_dict));

        // Resources
        let mut font_res = PdfDict::new();
        font_res.insert(b"F1".to_vec(), PdfObject::Reference(font_ref.clone()));
        let mut resources = PdfDict::new();
        resources.insert(b"Font".to_vec(), PdfObject::Dict(font_res));

        // Content stream
        let content = b"BT /F1 12 Tf (Hello) Tj ET";
        let (cs_dict, cs_data) = crate::writer::encode::make_stream(content, true);
        let cs_ref = writer.add_object(PdfObject::Stream { dict: cs_dict, data: cs_data });

        // Page
        let mut page = PdfDict::new();
        page.insert(b"Type".to_vec(), PdfObject::Name(b"Page".to_vec()));
        page.insert(b"Resources".to_vec(), PdfObject::Dict(resources));
        page.insert(b"Contents".to_vec(), PdfObject::Reference(cs_ref));
        page.insert(
            b"MediaBox".to_vec(),
            PdfObject::Array(vec![
                PdfObject::Integer(0), PdfObject::Integer(0),
                PdfObject::Integer(612), PdfObject::Integer(792),
            ]),
        );
        let page_ref = writer.add_object(PdfObject::Dict(page));

        // Pages
        let mut pages = PdfDict::new();
        pages.insert(b"Type".to_vec(), PdfObject::Name(b"Pages".to_vec()));
        pages.insert(b"Kids".to_vec(), PdfObject::Array(vec![PdfObject::Reference(page_ref)]));
        pages.insert(b"Count".to_vec(), PdfObject::Integer(1));
        let pages_ref = writer.add_object(PdfObject::Dict(pages));

        // Catalog
        let mut catalog = PdfDict::new();
        catalog.insert(b"Type".to_vec(), PdfObject::Name(b"Catalog".to_vec()));
        catalog.insert(b"Pages".to_vec(), PdfObject::Reference(pages_ref));
        let catalog_ref = writer.add_object(PdfObject::Dict(catalog));

        let pdf = crate::writer::serialize::serialize_pdf(
            &writer.objects, (1, 7), &catalog_ref, None,
        ).unwrap();

        // Compress with low preset (triggers serialize roundtrip)
        let (compressed, _) = compress_pdf(&pdf, &CompressOptions::preset_low()).unwrap();

        // Re-parse and verify the font name survived
        let reparsed = PdfDocument::from_bytes(compressed).unwrap();
        let pages = crate::page::collect_pages(&reparsed).unwrap();
        assert_eq!(pages.len(), 1);

        // Find the font object and check BaseFont
        let page_obj = reparsed.resolve(&pages[0].page_ref).unwrap();
        if let PdfObject::Dict(pd) = &page_obj {
            let res = match pd.get(b"Resources") {
                Some(PdfObject::Dict(d)) => d.clone(),
                Some(PdfObject::Reference(r)) => {
                    if let PdfObject::Dict(d) = reparsed.resolve(r).unwrap() { d } else { panic!() }
                }
                _ => panic!("No Resources"),
            };
            let fonts = match res.get(b"Font") {
                Some(PdfObject::Dict(d)) => d.clone(),
                Some(PdfObject::Reference(r)) => {
                    if let PdfObject::Dict(d) = reparsed.resolve(r).unwrap() { d } else { panic!() }
                }
                _ => panic!("No Font dict"),
            };
            let f1_ref = match fonts.get(b"F1") {
                Some(PdfObject::Reference(r)) => r.clone(),
                _ => panic!("No F1 font"),
            };
            let font = reparsed.resolve(&f1_ref).unwrap();
            if let PdfObject::Dict(fd) = &font {
                let basefont = fd.get_name(b"BaseFont").expect("BaseFont should exist");
                assert_eq!(
                    basefont, b"Pretendard Black",
                    "Font name with space must survive compression roundtrip"
                );
            } else {
                panic!("Font should be Dict");
            }
        }
    }

    /// Compression with string containing parentheses must not corrupt data.
    #[test]
    fn test_compress_string_with_special_chars() {
        let pdf = create_text_pdf(1);
        let (compressed, _) = compress_pdf(&pdf, &CompressOptions::preset_low()).unwrap();

        // Must produce valid PDF
        assert!(compressed.starts_with(b"%PDF"));
        let reparsed = PdfDocument::from_bytes(compressed).unwrap();
        let pages = crate::page::collect_pages(&reparsed).unwrap();
        assert_eq!(pages.len(), 1);
    }

    // ── Preset identity (issue #6) ──────────────────────────────────
    //
    // Each preset promises a tier of behavior to its users. These tests
    // pin those promises down by checking externally-visible effects, not
    // internal flag values, so the tests survive implementation changes.

    /// `low` preset is non-destructive: page count, image count, and text
    /// are preserved, no images re-encoded, no metadata stripped, and the
    /// output stays within a small size envelope of the input.
    #[test]
    fn test_preset_low_identity() {
        let pdf = create_pdf_with_jpeg(200, 200, 95);
        let original_size = pdf.len();
        let original_analysis = analyze_pdf(&pdf).unwrap();

        let (compressed, stats) =
            compress_pdf(&pdf, &CompressOptions::preset_low()).unwrap();

        assert!(compressed.starts_with(b"%PDF"));
        // Size envelope: low must not blow up the file
        assert!(
            compressed.len() <= original_size * 105 / 100,
            "low must stay within +5% of input ({} vs {})",
            compressed.len(),
            original_size,
        );
        // Page and image count preserved
        let new_analysis = analyze_pdf(&compressed).unwrap();
        assert_eq!(new_analysis.pages, original_analysis.pages);
        assert_eq!(new_analysis.images, original_analysis.images);
        // low does not touch images or metadata
        assert_eq!(stats.images_recompressed, 0, "low must not re-encode images");
        assert_eq!(stats.metadata_items_stripped, 0, "low must not strip metadata");
        assert_eq!(stats.images_grayscaled, 0, "low must not grayscale images");
    }

    /// `medium` preset adds image re-encoding on top of low's promises.
    /// Input must be above preset_medium's skip_below_bytes threshold (10_000).
    #[test]
    fn test_preset_medium_identity() {
        let pdf = create_pdf_with_jpeg(1000, 1000, 95);
        let original_size = pdf.len();
        let original_analysis = analyze_pdf(&pdf).unwrap();

        let (compressed, stats) =
            compress_pdf(&pdf, &CompressOptions::preset_medium()).unwrap();

        assert!(compressed.starts_with(b"%PDF"));
        // Re-encoding must yield a smaller file
        assert!(
            compressed.len() < original_size,
            "medium must shrink the file ({} vs {})",
            compressed.len(),
            original_size,
        );
        // Page and image count preserved
        let new_analysis = analyze_pdf(&compressed).unwrap();
        assert_eq!(new_analysis.pages, original_analysis.pages);
        assert_eq!(new_analysis.images, original_analysis.images);
        // medium's defining promise: at least one image re-encoded
        assert!(
            stats.images_recompressed > 0,
            "medium must re-encode at least one image"
        );
        // medium does not strip metadata or grayscale
        assert_eq!(stats.metadata_items_stripped, 0, "medium must not strip metadata");
        assert_eq!(stats.images_grayscaled, 0, "medium must not grayscale images");
    }

    /// Like `create_pdf_with_jpeg` but renders the image at a smaller
    /// display size so its effective DPI exceeds preset_high's 150 DPI cap.
    fn create_pdf_with_jpeg_at(
        image_w: u32,
        image_h: u32,
        quality: u8,
        draw_w_pt: f64,
        draw_h_pt: f64,
    ) -> Vec<u8> {
        let jpeg_data = create_test_jpeg(image_w, image_h, quality);
        let mut doc = DocumentBuilder::new();
        let font = doc.add_standard_font("Helvetica");
        let (_img_name, img_ref) =
            crate::writer::document::embed_jpeg(&mut doc, &jpeg_data).unwrap();
        let mut page = PageBuilder::new(612.0, 792.0);
        page.add_font(&font, "Helvetica");
        page.add_image("Im1", img_ref);
        page.draw_image("Im1", 72.0, 400.0, draw_w_pt, draw_h_pt);
        doc.add_page(page);
        doc.build().unwrap()
    }

    /// `high` preset adds downscaling on top of medium's promises.
    /// Input image is large enough (2000×2000) and rendered at a small display
    /// size (200×200 pt → ~720 DPI effective) so high's 150 DPI cap triggers.
    #[test]
    fn test_preset_high_identity() {
        let pdf = create_pdf_with_jpeg_at(2000, 2000, 95, 200.0, 200.0);
        let original_size = pdf.len();
        let original_analysis = analyze_pdf(&pdf).unwrap();

        let (compressed, stats) =
            compress_pdf(&pdf, &CompressOptions::preset_high()).unwrap();

        assert!(compressed.starts_with(b"%PDF"));
        assert!(
            compressed.len() < original_size,
            "high must shrink the file ({} vs {})",
            compressed.len(),
            original_size,
        );
        // Page and image count preserved
        let new_analysis = analyze_pdf(&compressed).unwrap();
        assert_eq!(new_analysis.pages, original_analysis.pages);
        assert_eq!(new_analysis.images, original_analysis.images);
        // Re-encoding still happens (carried from medium)
        assert!(stats.images_recompressed > 0);
        // high's defining promise: at least one image downscaled
        assert!(
            stats.images_downscaled > 0,
            "high must downscale at least one image"
        );
    }

    /// Like `create_pdf_with_jpeg` but adds an XMP metadata stream in the
    /// catalog so preset_extreme has something to strip. (The strip pass
    /// targets the catalog `/Metadata` key, not the trailer Info dict.)
    fn create_pdf_with_metadata_and_jpeg(image_w: u32, image_h: u32, quality: u8) -> Vec<u8> {
        let jpeg_data = create_test_jpeg(image_w, image_h, quality);
        let mut doc = DocumentBuilder::new();
        doc.set_xmp_metadata(
            "Identity Test PDF",
            "Compression Test Author",
            "Preset identity verification",
            "compress.rs test suite",
        );
        let font = doc.add_standard_font("Helvetica");
        let (_img_name, img_ref) =
            crate::writer::document::embed_jpeg(&mut doc, &jpeg_data).unwrap();
        let mut page = PageBuilder::new(612.0, 792.0);
        page.add_font(&font, "Helvetica");
        page.add_image("Im1", img_ref);
        page.draw_image("Im1", 72.0, 400.0, image_w as f64, image_h as f64);
        doc.add_page(page);
        doc.build().unwrap()
    }

    /// `extreme` preset adds metadata stripping and/or grayscale conversion
    /// on top of medium's image re-encoding. Either signal is sufficient
    /// because extreme's character is "drop everything non-essential".
    #[test]
    fn test_preset_extreme_identity() {
        let pdf = create_pdf_with_metadata_and_jpeg(1000, 1000, 95);
        let original_size = pdf.len();
        let original_analysis = analyze_pdf(&pdf).unwrap();

        let (compressed, stats) =
            compress_pdf(&pdf, &CompressOptions::preset_extreme()).unwrap();

        assert!(compressed.starts_with(b"%PDF"));
        assert!(
            compressed.len() < original_size,
            "extreme must shrink the file ({} vs {})",
            compressed.len(),
            original_size,
        );
        // Page and image count preserved
        let new_analysis = analyze_pdf(&compressed).unwrap();
        assert_eq!(new_analysis.pages, original_analysis.pages);
        assert_eq!(new_analysis.images, original_analysis.images);
        // Re-encoding still happens (carried from medium)
        assert!(stats.images_recompressed > 0);
        // extreme's defining promise: aggressive stripping — either metadata
        // items removed or images grayscaled (or both).
        assert!(
            stats.metadata_items_stripped > 0 || stats.images_grayscaled > 0,
            "extreme must strip metadata or grayscale images (got stripped={}, grayscaled={})",
            stats.metadata_items_stripped,
            stats.images_grayscaled,
        );
    }
}
