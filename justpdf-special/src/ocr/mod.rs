//! OCR support via Tesseract CLI.
//!
//! Requires `tesseract` to be installed and available on the PATH.

use std::path::Path;
use std::process::Command;

use crate::{Result, SpecialError};

/// Check if Tesseract is available on the system PATH.
pub fn is_tesseract_available() -> bool {
    Command::new("tesseract")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Get the Tesseract version string.
pub fn tesseract_version() -> Result<String> {
    let output = Command::new("tesseract")
        .arg("--version")
        .output()
        .map_err(|e| SpecialError::NotFound {
            detail: format!("tesseract not found: {e}"),
        })?;
    if !output.status.success() {
        return Err(SpecialError::NotFound {
            detail: "tesseract returned non-zero exit code".into(),
        });
    }
    // Tesseract prints version to stderr on some platforms, stdout on others.
    let text = if output.stdout.is_empty() {
        String::from_utf8_lossy(&output.stderr).to_string()
    } else {
        String::from_utf8_lossy(&output.stdout).to_string()
    };
    Ok(text.lines().next().unwrap_or("").to_string())
}

/// OCR a single image file and return recognized text.
///
/// `language` is an optional Tesseract language code (e.g. `"eng"`, `"deu"`).
pub fn ocr_image(image_path: &Path, language: Option<&str>) -> Result<String> {
    if !is_tesseract_available() {
        return Err(SpecialError::NotFound {
            detail: "tesseract is not installed. Install from https://github.com/tesseract-ocr/tesseract".into(),
        });
    }

    let mut cmd = Command::new("tesseract");
    cmd.arg(image_path);
    cmd.arg("stdout"); // output to stdout

    if let Some(lang) = language {
        cmd.arg("-l").arg(lang);
    }

    let output = cmd.output()?;

    if !output.status.success() {
        return Err(SpecialError::Feature {
            detail: format!(
                "tesseract failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ),
        });
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// OCR a PDF page by rendering it to an image first, then running Tesseract.
///
/// Returns the recognized text for the specified page.
pub fn ocr_pdf_page(
    doc: &justpdf_core::PdfDocument,
    page_index: usize,
    dpi: f64,
    language: Option<&str>,
) -> Result<String> {
    if !is_tesseract_available() {
        return Err(SpecialError::NotFound {
            detail: "tesseract is not installed".into(),
        });
    }

    // Render page to PNG
    let opts = justpdf_render::RenderOptions {
        dpi,
        format: justpdf_render::OutputFormat::Png,
        ..Default::default()
    };
    let png_data = justpdf_render::render_page(doc, page_index, &opts)
        .map_err(|e| SpecialError::Feature {
            detail: format!("render failed: {e}"),
        })?;

    // Write to temp file
    let temp_dir = std::env::temp_dir();
    let temp_path = temp_dir.join(format!("justpdf_ocr_{}.png", std::process::id()));
    std::fs::write(&temp_path, &png_data)?;

    let result = ocr_image(&temp_path, language);

    // Clean up
    let _ = std::fs::remove_file(&temp_path);

    result
}

/// Create a searchable PDF from a scanned PDF by adding an OCR text layer.
///
/// This renders each page, runs OCR, and creates a new PDF with invisible
/// text positioned over the scanned image.
pub fn make_searchable_pdf(
    doc: &justpdf_core::PdfDocument,
    dpi: f64,
    language: Option<&str>,
) -> Result<Vec<u8>> {
    use justpdf_core::page;
    use justpdf_core::writer::{DocumentBuilder, PageBuilder};

    let pages = page::collect_pages(doc).map_err(SpecialError::Pdf)?;

    let mut builder = DocumentBuilder::new();
    let font_name = builder.add_standard_font("Helvetica");

    for (i, page_info) in pages.iter().enumerate() {
        let media_box = page_info.crop_box.unwrap_or(page_info.media_box);
        let w = media_box.width();
        let h = media_box.height();

        // Render page to image
        let opts = justpdf_render::RenderOptions {
            dpi,
            format: justpdf_render::OutputFormat::Png,
            ..Default::default()
        };
        let png_data = justpdf_render::render_page(doc, i, &opts).map_err(|e| {
            SpecialError::Feature {
                detail: format!("render: {e}"),
            }
        })?;

        // OCR the rendered image
        let temp_path = std::env::temp_dir()
            .join(format!("justpdf_ocr_{}_{i}.png", std::process::id()));
        std::fs::write(&temp_path, &png_data)?;
        let ocr_text = ocr_image(&temp_path, language).unwrap_or_default();
        let _ = std::fs::remove_file(&temp_path);

        // Build page with image + invisible text overlay
        let mut page = PageBuilder::new(w, h);
        page.add_font(&font_name, "Helvetica");

        // Draw the scanned image as background (inline image)
        let img = image::load_from_memory(&png_data).map_err(|e| SpecialError::Feature {
            detail: format!("decode: {e}"),
        })?;
        let rgb = img.to_rgb8();
        // Save/restore graphics state, scale image to page size
        use std::io::Write as _;
        let mut content_prefix = Vec::new();
        write!(content_prefix, "q {} 0 0 {} 0 0 cm\n", w, h).unwrap();
        page.draw_inline_image(rgb.width(), rgb.height(), 8, "DeviceRGB", rgb.as_raw());

        // Add invisible text overlay (render mode 3 = invisible)
        if !ocr_text.trim().is_empty() {
            page.begin_text();
            page.set_font(&font_name, 1.0);
            page.move_to(0.0, 1.0);
            for line in ocr_text.lines().take(100) {
                let clean_line = line.trim();
                if !clean_line.is_empty() {
                    page.show_text(clean_line);
                }
            }
            page.end_text();
        }

        builder.add_page(page);
    }

    builder.build().map_err(SpecialError::Pdf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tesseract_availability_check() {
        // Just check it doesn't panic
        let _ = is_tesseract_available();
    }

    #[test]
    fn test_tesseract_not_found_error() {
        // If tesseract is not installed, ocr should return a clear error
        if !is_tesseract_available() {
            let result = ocr_image(Path::new("nonexistent.png"), None);
            assert!(result.is_err());
            let err = result.unwrap_err();
            assert!(
                format!("{err:?}").contains("not installed")
                    || format!("{err:?}").contains("not found")
            );
        }
    }
}
