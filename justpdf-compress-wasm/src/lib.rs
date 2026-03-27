use wasm_bindgen::prelude::*;
use justpdf_core::writer::compress;

/// Result of PDF compression.
#[wasm_bindgen]
pub struct CompressResult {
    data: Vec<u8>,
    original_size: u32,
    compressed_size: u32,
    images_found: u32,
    images_recompressed: u32,
    images_downscaled: u32,
    images_skipped: u32,
    duplicates_removed: u32,
    objects_removed_gc: u32,
    streams_recompressed: u32,
    fonts_subsetted: u32,
    unused_resources_removed: u32,
    metadata_items_stripped: u32,
    images_grayscaled: u32,
}

#[wasm_bindgen]
impl CompressResult {
    /// Get the compressed PDF bytes.
    pub fn data(&self) -> Vec<u8> {
        self.data.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn original_size(&self) -> u32 {
        self.original_size
    }

    #[wasm_bindgen(getter)]
    pub fn compressed_size(&self) -> u32 {
        self.compressed_size
    }

    #[wasm_bindgen(getter)]
    pub fn images_found(&self) -> u32 {
        self.images_found
    }

    #[wasm_bindgen(getter)]
    pub fn images_recompressed(&self) -> u32 {
        self.images_recompressed
    }

    #[wasm_bindgen(getter)]
    pub fn images_downscaled(&self) -> u32 {
        self.images_downscaled
    }

    #[wasm_bindgen(getter)]
    pub fn images_skipped(&self) -> u32 {
        self.images_skipped
    }

    #[wasm_bindgen(getter)]
    pub fn duplicates_removed(&self) -> u32 {
        self.duplicates_removed
    }

    #[wasm_bindgen(getter)]
    pub fn objects_removed_gc(&self) -> u32 {
        self.objects_removed_gc
    }

    #[wasm_bindgen(getter)]
    pub fn streams_recompressed(&self) -> u32 {
        self.streams_recompressed
    }

    #[wasm_bindgen(getter)]
    pub fn fonts_subsetted(&self) -> u32 {
        self.fonts_subsetted
    }

    #[wasm_bindgen(getter)]
    pub fn unused_resources_removed(&self) -> u32 {
        self.unused_resources_removed
    }

    #[wasm_bindgen(getter)]
    pub fn metadata_items_stripped(&self) -> u32 {
        self.metadata_items_stripped
    }

    #[wasm_bindgen(getter)]
    pub fn images_grayscaled(&self) -> u32 {
        self.images_grayscaled
    }

    /// Compression ratio (0.0–1.0). Lower = more compression.
    #[wasm_bindgen(getter)]
    pub fn ratio(&self) -> f64 {
        if self.original_size == 0 {
            1.0
        } else {
            self.compressed_size as f64 / self.original_size as f64
        }
    }
}

fn stats_to_result(output: Vec<u8>, stats: compress::CompressStats) -> CompressResult {
    CompressResult {
        data: output,
        original_size: stats.original_size as u32,
        compressed_size: stats.compressed_size as u32,
        images_found: stats.images_found as u32,
        images_recompressed: stats.images_recompressed as u32,
        images_downscaled: stats.images_downscaled as u32,
        images_skipped: stats.images_skipped as u32,
        duplicates_removed: stats.duplicates_removed as u32,
        objects_removed_gc: stats.objects_removed_gc as u32,
        streams_recompressed: stats.streams_recompressed as u32,
        fonts_subsetted: stats.fonts_subsetted as u32,
        unused_resources_removed: stats.unused_resources_removed as u32,
        metadata_items_stripped: stats.metadata_items_stripped as u32,
        images_grayscaled: stats.images_grayscaled as u32,
    }
}

/// Result of PDF analysis.
#[wasm_bindgen]
pub struct AnalyzeResult {
    pages: u32,
    images: u32,
    total_image_bytes: u32,
    is_encrypted: bool,
}

#[wasm_bindgen]
impl AnalyzeResult {
    #[wasm_bindgen(getter)]
    pub fn pages(&self) -> u32 {
        self.pages
    }

    #[wasm_bindgen(getter)]
    pub fn images(&self) -> u32 {
        self.images
    }

    #[wasm_bindgen(getter)]
    pub fn total_image_bytes(&self) -> u32 {
        self.total_image_bytes
    }

    #[wasm_bindgen(getter)]
    pub fn is_encrypted(&self) -> bool {
        self.is_encrypted
    }
}

/// Compress a PDF using a preset.
///
/// Presets: `"low"`, `"medium"`, `"high"`, `"extreme"`.
#[wasm_bindgen]
pub fn compress(data: &[u8], preset: &str) -> Result<CompressResult, JsValue> {
    let options = compress::CompressOptions::from_preset(preset)
        .ok_or_else(|| JsValue::from_str(&format!("unknown preset: {preset}")))?;

    let (output, stats) = compress::compress_pdf(data, &options)
        .map_err(|e| JsValue::from_str(&format!("{e}")))?;

    Ok(stats_to_result(output, stats))
}

/// Compress a PDF with custom quality and DPI settings.
#[wasm_bindgen]
pub fn compress_custom(
    data: &[u8],
    jpeg_quality: u8,
    max_dpi: f64,
) -> Result<CompressResult, JsValue> {
    let options = compress::CompressOptions {
        jpeg_quality: Some(jpeg_quality),
        max_image_dpi: if max_dpi > 0.0 { Some(max_dpi) } else { None },
        skip_below_bytes: 5_000,
        structural: true,
        compress_streams: true,
        font_subsetting: true,
        remove_unused_resources: true,
        strip_metadata: false,
        strip_extras: false,
        grayscale: false,
    };

    let (output, stats) = compress::compress_pdf(data, &options)
        .map_err(|e| JsValue::from_str(&format!("{e}")))?;

    Ok(stats_to_result(output, stats))
}

/// Compress a PDF with full control over all options.
#[wasm_bindgen]
pub fn compress_advanced(
    data: &[u8],
    jpeg_quality: i32,
    max_dpi: f64,
    font_subsetting: bool,
    remove_unused_resources: bool,
    strip_metadata: bool,
    strip_extras: bool,
    grayscale: bool,
) -> Result<CompressResult, JsValue> {
    let options = compress::CompressOptions {
        jpeg_quality: if jpeg_quality > 0 { Some(jpeg_quality as u8) } else { None },
        max_image_dpi: if max_dpi > 0.0 { Some(max_dpi) } else { None },
        skip_below_bytes: 5_000,
        structural: true,
        compress_streams: true,
        font_subsetting,
        remove_unused_resources,
        strip_metadata,
        strip_extras,
        grayscale,
    };

    let (output, stats) = compress::compress_pdf(data, &options)
        .map_err(|e| JsValue::from_str(&format!("{e}")))?;

    Ok(stats_to_result(output, stats))
}

/// Analyze a PDF without compressing it.
///
/// Returns page count, image count, total image bytes, and encryption status.
#[wasm_bindgen]
pub fn analyze(data: &[u8]) -> Result<AnalyzeResult, JsValue> {
    let result = compress::analyze_pdf(data)
        .map_err(|e| JsValue::from_str(&format!("{e}")))?;

    Ok(AnalyzeResult {
        pages: result.pages as u32,
        images: result.images as u32,
        total_image_bytes: result.total_image_bytes as u32,
        is_encrypted: result.is_encrypted,
    })
}
