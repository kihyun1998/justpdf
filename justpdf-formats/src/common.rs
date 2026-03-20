use crate::Result;

/// Page information for any document format.
#[derive(Debug, Clone)]
pub struct FormatPage {
    /// 0-based page index.
    pub index: usize,
    /// Page width in points (1 point = 1/72 inch).
    pub width_pt: f64,
    /// Page height in points.
    pub height_pt: f64,
}

/// Document metadata common to all formats.
#[derive(Debug, Clone, Default)]
pub struct FormatMetadata {
    pub title: Option<String>,
    pub author: Option<String>,
    pub subject: Option<String>,
    pub creator: Option<String>,
    pub page_count: usize,
}

/// Rendered page data.
#[derive(Debug)]
pub struct RenderedPage {
    /// Raw RGBA pixel data.
    pub data: Vec<u8>,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

/// Trait for multi-format document support.
///
/// Implementations provide uniform access to pages, text extraction,
/// rendering, and PDF conversion for various document formats.
pub trait FormatDocument {
    /// Get document metadata.
    fn metadata(&self) -> FormatMetadata;

    /// Number of pages (or chapters, images, etc.).
    fn page_count(&self) -> usize;

    /// Get page info by 0-based index.
    fn page(&self, index: usize) -> Result<FormatPage>;

    /// Extract plain text from a page.
    fn page_text(&self, index: usize) -> Result<String>;

    /// Extract text from all pages.
    fn text(&self) -> Result<String> {
        let mut result = String::new();
        for i in 0..self.page_count() {
            if i > 0 {
                result.push('\n');
            }
            result.push_str(&self.page_text(i)?);
        }
        Ok(result)
    }

    /// Render a page to RGBA pixels at the given DPI.
    fn render_page(&self, index: usize, dpi: f64) -> Result<RenderedPage>;

    /// Render a page to PNG bytes.
    fn render_page_png(&self, index: usize, dpi: f64) -> Result<Vec<u8>>;

    /// Convert entire document to PDF bytes.
    fn to_pdf(&self) -> Result<Vec<u8>>;
}
