//! CBZ (Comic Book Archive) format support.
//!
//! A CBZ file is a ZIP archive containing image files (JPEG, PNG, etc.)
//! sorted by filename. Each image represents one page.

use std::io::{Read, Cursor};
use std::path::Path;

use crate::common::{FormatDocument, FormatMetadata, FormatPage, RenderedPage};
use crate::error::FormatError;
use crate::Result;

/// A CBZ comic book document.
pub struct CbzDocument {
    /// Image entries sorted by filename.
    images: Vec<CbzImage>,
}

struct CbzImage {
    /// Filename within the archive.
    #[allow(dead_code)]
    name: String,
    /// Raw image data (JPEG or PNG).
    data: Vec<u8>,
    /// Decoded image width.
    width: u32,
    /// Decoded image height.
    height: u32,
}

impl CbzDocument {
    /// Open a CBZ file.
    pub fn open(path: &Path) -> Result<Self> {
        let data = std::fs::read(path)?;
        Self::from_bytes(&data)
    }

    /// Parse CBZ from bytes.
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        let reader = Cursor::new(data);
        let mut archive = zip::ZipArchive::new(reader)
            .map_err(|e| FormatError::Zip(format!("{e}")))?;

        let mut entries: Vec<(String, Vec<u8>)> = Vec::new();

        for i in 0..archive.len() {
            let mut file = archive.by_index(i)
                .map_err(|e| FormatError::Zip(format!("{e}")))?;
            let name = file.name().to_string();

            // Skip directories and non-image files
            if file.is_dir() || !is_image_extension(&name) {
                continue;
            }

            let mut buf = Vec::new();
            file.read_to_end(&mut buf)?;
            entries.push((name, buf));
        }

        // Sort by filename (natural sort)
        entries.sort_by(|a, b| natural_sort_key(&a.0).cmp(&natural_sort_key(&b.0)));

        // Decode image dimensions
        let mut images = Vec::new();
        for (name, data) in entries {
            let (width, height) = image_dimensions(&data).unwrap_or((0, 0));
            if width > 0 && height > 0 {
                images.push(CbzImage { name, data, width, height });
            }
        }

        if images.is_empty() {
            return Err(FormatError::Format {
                detail: "CBZ archive contains no valid images".into(),
            });
        }

        Ok(Self { images })
    }
}

impl FormatDocument for CbzDocument {
    fn metadata(&self) -> FormatMetadata {
        FormatMetadata {
            title: None,
            author: None,
            subject: None,
            creator: None,
            page_count: self.images.len(),
        }
    }

    fn page_count(&self) -> usize {
        self.images.len()
    }

    fn page(&self, index: usize) -> Result<FormatPage> {
        let img = self.images.get(index).ok_or(FormatError::OutOfRange {
            index,
            count: self.images.len(),
        })?;
        // Convert pixels to points at 72 DPI (1 pixel = 1 point)
        Ok(FormatPage {
            index,
            width_pt: img.width as f64,
            height_pt: img.height as f64,
        })
    }

    fn page_text(&self, index: usize) -> Result<String> {
        if index >= self.images.len() {
            return Err(FormatError::OutOfRange { index, count: self.images.len() });
        }
        // Images don't have text
        Ok(String::new())
    }

    fn render_page(&self, index: usize, dpi: f64) -> Result<RenderedPage> {
        let img = self.images.get(index).ok_or(FormatError::OutOfRange {
            index,
            count: self.images.len(),
        })?;
        let decoded = image::load_from_memory(&img.data)
            .map_err(|e| FormatError::Format { detail: format!("image decode: {e}") })?;

        // Scale if DPI != 72
        let scale = dpi / 72.0;
        let out_w = (img.width as f64 * scale).ceil() as u32;
        let out_h = (img.height as f64 * scale).ceil() as u32;

        let resized = if (scale - 1.0).abs() > f64::EPSILON {
            decoded.resize_exact(out_w, out_h, image::imageops::FilterType::Lanczos3)
        } else {
            decoded
        };

        let rgba = resized.to_rgba8();
        Ok(RenderedPage {
            data: rgba.into_raw(),
            width: out_w,
            height: out_h,
        })
    }

    fn render_page_png(&self, index: usize, dpi: f64) -> Result<Vec<u8>> {
        let rendered = self.render_page(index, dpi)?;
        let mut buf = Vec::new();
        let encoder = image::codecs::png::PngEncoder::new(&mut buf);
        image::ImageEncoder::write_image(
            encoder,
            &rendered.data,
            rendered.width,
            rendered.height,
            image::ExtendedColorType::Rgba8,
        ).map_err(|e| FormatError::Format { detail: format!("PNG encode: {e}") })?;
        Ok(buf)
    }

    fn to_pdf(&self) -> Result<Vec<u8>> {
        use justpdf_core::writer::{DocumentBuilder, PageBuilder};

        let mut builder = DocumentBuilder::new();

        for img in &self.images {
            let w = img.width as f64;
            let h = img.height as f64;
            let mut page = PageBuilder::new(w, h);

            // Decode to raw RGB for inline image
            let decoded = image::load_from_memory(&img.data)
                .map_err(|e| FormatError::Format { detail: format!("image decode: {e}") })?;
            let rgb = decoded.to_rgb8();
            let rgb_data = rgb.as_raw();

            page.draw_inline_image(
                img.width,
                img.height,
                8,
                "DeviceRGB",
                rgb_data,
            );

            builder.add_page(page);
        }

        Ok(builder.build()?)
    }
}

fn is_image_extension(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.ends_with(".jpg")
        || lower.ends_with(".jpeg")
        || lower.ends_with(".png")
        || lower.ends_with(".gif")
        || lower.ends_with(".bmp")
        || lower.ends_with(".webp")
}

fn image_dimensions(data: &[u8]) -> Option<(u32, u32)> {
    // Try to get dimensions without full decode
    if let Ok(reader) = image::ImageReader::new(Cursor::new(data)).with_guessed_format() {
        if let Ok((w, h)) = reader.into_dimensions() {
            return Some((w, h));
        }
    }
    None
}

/// Generate a sort key for natural filename sorting.
/// Pads numeric sequences so "page2" sorts before "page10".
fn natural_sort_key(s: &str) -> String {
    let mut result = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c.is_ascii_digit() {
            let mut num = String::from(c);
            while let Some(&next) = chars.peek() {
                if next.is_ascii_digit() {
                    num.push(chars.next().unwrap());
                } else {
                    break;
                }
            }
            // Pad to 20 digits
            result.push_str(&format!("{:0>20}", num));
        } else {
            result.push(c.to_ascii_lowercase());
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_natural_sort_key() {
        assert!(natural_sort_key("page2") < natural_sort_key("page10"));
        assert!(natural_sort_key("001") < natural_sort_key("002"));
        assert!(natural_sort_key("a1b") < natural_sort_key("a2b"));
    }

    #[test]
    fn test_is_image_extension() {
        assert!(is_image_extension("page.jpg"));
        assert!(is_image_extension("PAGE.PNG"));
        assert!(is_image_extension("image.jpeg"));
        assert!(!is_image_extension("readme.txt"));
        assert!(!is_image_extension("comic.xml"));
    }

    #[test]
    fn test_cbz_from_bytes_creates_zip() {
        // Create a minimal CBZ (ZIP with one PNG image)
        use std::io::Write;
        let mut buf = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(Cursor::new(&mut buf));
            let options = zip::write::SimpleFileOptions::default();
            zip.start_file("page001.png", options).unwrap();
            // Write a minimal valid 1x1 PNG
            let png_data = create_minimal_png();
            zip.write_all(&png_data).unwrap();
            zip.finish().unwrap();
        }

        let doc = CbzDocument::from_bytes(&buf).unwrap();
        assert_eq!(doc.page_count(), 1);
        assert_eq!(doc.page(0).unwrap().width_pt, 1.0);
        assert_eq!(doc.page(0).unwrap().height_pt, 1.0);
    }

    #[test]
    fn test_cbz_page_text_empty() {
        let mut buf = Vec::new();
        {
            use std::io::Write;
            let mut zip = zip::ZipWriter::new(Cursor::new(&mut buf));
            let options = zip::write::SimpleFileOptions::default();
            zip.start_file("img.png", options).unwrap();
            zip.write_all(&create_minimal_png()).unwrap();
            zip.finish().unwrap();
        }
        let doc = CbzDocument::from_bytes(&buf).unwrap();
        let text = doc.page_text(0).unwrap();
        assert!(text.is_empty());
    }

    #[test]
    fn test_cbz_no_images() {
        let mut buf = Vec::new();
        {
            use std::io::Write;
            let mut zip = zip::ZipWriter::new(Cursor::new(&mut buf));
            let options = zip::write::SimpleFileOptions::default();
            zip.start_file("readme.txt", options).unwrap();
            zip.write_all(b"not an image").unwrap();
            zip.finish().unwrap();
        }
        assert!(CbzDocument::from_bytes(&buf).is_err());
    }

    #[test]
    fn test_cbz_render_png() {
        let mut buf = Vec::new();
        {
            use std::io::Write;
            let mut zip = zip::ZipWriter::new(Cursor::new(&mut buf));
            let options = zip::write::SimpleFileOptions::default();
            zip.start_file("img.png", options).unwrap();
            zip.write_all(&create_minimal_png()).unwrap();
            zip.finish().unwrap();
        }
        let doc = CbzDocument::from_bytes(&buf).unwrap();
        let png = doc.render_page_png(0, 72.0).unwrap();
        assert_eq!(&png[..4], &[0x89, 0x50, 0x4E, 0x47]);
    }

    #[test]
    fn test_cbz_to_pdf() {
        let mut buf = Vec::new();
        {
            use std::io::Write;
            let mut zip = zip::ZipWriter::new(Cursor::new(&mut buf));
            let options = zip::write::SimpleFileOptions::default();
            zip.start_file("img.png", options).unwrap();
            zip.write_all(&create_minimal_png()).unwrap();
            zip.finish().unwrap();
        }
        let doc = CbzDocument::from_bytes(&buf).unwrap();
        let pdf = doc.to_pdf().unwrap();
        assert!(pdf.starts_with(b"%PDF"));
    }

    /// Create a minimal valid 1x1 white PNG.
    fn create_minimal_png() -> Vec<u8> {
        let mut buf = Vec::new();
        let encoder = image::codecs::png::PngEncoder::new(&mut buf);
        image::ImageEncoder::write_image(
            encoder,
            &[255u8, 255, 255, 255], // 1 pixel RGBA white
            1, 1,
            image::ExtendedColorType::Rgba8,
        ).unwrap();
        buf
    }
}
