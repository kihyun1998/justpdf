//! Plain text to PDF conversion.

use crate::common::{FormatDocument, FormatMetadata, FormatPage, RenderedPage};
use crate::Result;
use crate::error::FormatError;

use justpdf_core::writer::{DocumentBuilder, PageBuilder};

/// A plain text document.
pub struct PlainTextDocument {
    /// Lines of text, grouped by page.
    pages: Vec<Vec<String>>,
    /// Page width in points.
    page_width: f64,
    /// Page height in points.
    page_height: f64,
    /// Font size in points.
    font_size: f64,
    /// Line spacing multiplier.
    line_spacing: f64,
    /// Margin in points.
    margin: f64,
}

impl PlainTextDocument {
    /// Create a plain text document from a string.
    ///
    /// The text is automatically wrapped and paginated using US Letter size
    /// (612 x 792 pt) with 72pt margins and 10pt Courier font.
    pub fn from_string(text: &str) -> Self {
        Self::from_string_with_options(text, 612.0, 792.0, 10.0, 1.2, 72.0)
    }

    /// Create with custom page dimensions and font settings.
    pub fn from_string_with_options(
        text: &str,
        page_width: f64,
        page_height: f64,
        font_size: f64,
        line_spacing: f64,
        margin: f64,
    ) -> Self {
        let usable_width = page_width - 2.0 * margin;
        let usable_height = page_height - 2.0 * margin;
        let line_height = font_size * line_spacing;

        // Approximate chars per line for Courier (monospace, 0.6 * font_size per char)
        let char_width = font_size * 0.6;
        let chars_per_line = (usable_width / char_width).floor() as usize;
        let lines_per_page = (usable_height / line_height).floor() as usize;

        // Wrap and paginate
        let mut all_lines = Vec::new();
        for line in text.lines() {
            if line.is_empty() {
                all_lines.push(String::new());
            } else {
                // Word wrap
                let mut remaining = line;
                while !remaining.is_empty() {
                    if remaining.len() <= chars_per_line {
                        all_lines.push(remaining.to_string());
                        break;
                    }
                    // Find break point
                    let break_at = remaining[..chars_per_line]
                        .rfind(' ')
                        .unwrap_or(chars_per_line);
                    all_lines.push(remaining[..break_at].to_string());
                    remaining = remaining[break_at..].trim_start();
                }
            }
        }

        // Split into pages
        let pages: Vec<Vec<String>> = all_lines
            .chunks(lines_per_page.max(1))
            .map(|chunk| chunk.to_vec())
            .collect();

        // Ensure at least one page
        let pages = if pages.is_empty() {
            vec![vec![]]
        } else {
            pages
        };

        Self {
            pages,
            page_width,
            page_height,
            font_size,
            line_spacing,
            margin,
        }
    }

    /// Load from a file.
    pub fn open(path: &std::path::Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)?;
        Ok(Self::from_string(&text))
    }

    fn build_page_pdf(&self, page_lines: &[String]) -> Result<Vec<u8>> {
        let mut builder = DocumentBuilder::new();
        let font_name = builder.add_standard_font("Courier");

        let mut page = PageBuilder::new(self.page_width, self.page_height);
        page.add_font(&font_name, "Courier");
        page.begin_text();
        page.set_font(&font_name, self.font_size);

        let line_height = self.font_size * self.line_spacing;
        let mut y = self.page_height - self.margin - self.font_size;

        for line in page_lines {
            page.move_to(self.margin, y);
            if !line.is_empty() {
                page.show_text(line);
            }
            y -= line_height;
        }

        page.end_text();
        builder.add_page(page);
        Ok(builder.build()?)
    }
}

impl FormatDocument for PlainTextDocument {
    fn metadata(&self) -> FormatMetadata {
        FormatMetadata {
            title: None,
            author: None,
            subject: None,
            creator: Some("justpdf".to_string()),
            page_count: self.pages.len(),
        }
    }

    fn page_count(&self) -> usize {
        self.pages.len()
    }

    fn page(&self, index: usize) -> Result<FormatPage> {
        if index >= self.pages.len() {
            return Err(FormatError::OutOfRange { index, count: self.pages.len() });
        }
        Ok(FormatPage {
            index,
            width_pt: self.page_width,
            height_pt: self.page_height,
        })
    }

    fn page_text(&self, index: usize) -> Result<String> {
        if index >= self.pages.len() {
            return Err(FormatError::OutOfRange { index, count: self.pages.len() });
        }
        Ok(self.pages[index].join("\n"))
    }

    fn render_page(&self, index: usize, dpi: f64) -> Result<RenderedPage> {
        if index >= self.pages.len() {
            return Err(FormatError::OutOfRange { index, count: self.pages.len() });
        }
        let pdf_bytes = self.build_page_pdf(&self.pages[index])?;
        let doc = justpdf_core::PdfDocument::from_bytes(pdf_bytes)?;
        let opts = justpdf_render::RenderOptions {
            dpi,
            format: justpdf_render::OutputFormat::RawRgba,
            ..Default::default()
        };
        let pixmap = justpdf_render::render_page_to_pixmap(&doc, 0, &opts)?;
        Ok(RenderedPage {
            data: pixmap.data,
            width: pixmap.width,
            height: pixmap.height,
        })
    }

    fn render_page_png(&self, index: usize, dpi: f64) -> Result<Vec<u8>> {
        if index >= self.pages.len() {
            return Err(FormatError::OutOfRange { index, count: self.pages.len() });
        }
        let pdf_bytes = self.build_page_pdf(&self.pages[index])?;
        let doc = justpdf_core::PdfDocument::from_bytes(pdf_bytes)?;
        let opts = justpdf_render::RenderOptions {
            dpi,
            format: justpdf_render::OutputFormat::Png,
            ..Default::default()
        };
        Ok(justpdf_render::render_page(&doc, 0, &opts)?)
    }

    fn to_pdf(&self) -> Result<Vec<u8>> {
        let mut builder = DocumentBuilder::new();
        let font_name = builder.add_standard_font("Courier");

        for page_lines in &self.pages {
            let mut page = PageBuilder::new(self.page_width, self.page_height);
            page.add_font(&font_name, "Courier");
            page.begin_text();
            page.set_font(&font_name, self.font_size);

            let line_height = self.font_size * self.line_spacing;
            let mut y = self.page_height - self.margin - self.font_size;

            for line in page_lines {
                page.move_to(self.margin, y);
                if !line.is_empty() {
                    page.show_text(line);
                }
                y -= line_height;
            }

            page.end_text();
            builder.add_page(page);
        }

        Ok(builder.build()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_text() {
        let doc = PlainTextDocument::from_string("");
        assert_eq!(doc.page_count(), 1);
        let text = doc.page_text(0).unwrap();
        assert!(text.is_empty());
    }

    #[test]
    fn test_single_line() {
        let doc = PlainTextDocument::from_string("Hello, World!");
        assert_eq!(doc.page_count(), 1);
        let text = doc.page_text(0).unwrap();
        assert!(text.contains("Hello, World!"));
    }

    #[test]
    fn test_multiple_lines() {
        let text = "Line 1\nLine 2\nLine 3";
        let doc = PlainTextDocument::from_string(text);
        assert_eq!(doc.page_count(), 1);
        let extracted = doc.page_text(0).unwrap();
        assert!(extracted.contains("Line 1"));
        assert!(extracted.contains("Line 3"));
    }

    #[test]
    fn test_pagination() {
        // Create enough lines to fill multiple pages
        let lines: Vec<String> = (0..200).map(|i| format!("Line number {i}")).collect();
        let text = lines.join("\n");
        let doc = PlainTextDocument::from_string(&text);
        assert!(doc.page_count() > 1, "should have multiple pages");
    }

    #[test]
    fn test_to_pdf() {
        let doc = PlainTextDocument::from_string("Hello PDF");
        let pdf = doc.to_pdf().unwrap();
        assert!(pdf.starts_with(b"%PDF"));
        // Verify it's a valid PDF
        let parsed = justpdf_core::PdfDocument::from_bytes(pdf).unwrap();
        assert_eq!(justpdf_core::page::page_count(&parsed).unwrap(), 1);
    }

    #[test]
    fn test_render_png() {
        let doc = PlainTextDocument::from_string("Render test");
        let png = doc.render_page_png(0, 72.0).unwrap();
        assert_eq!(&png[..4], &[0x89, 0x50, 0x4E, 0x47]); // PNG magic
    }

    #[test]
    fn test_page_out_of_range() {
        let doc = PlainTextDocument::from_string("Hello");
        assert!(doc.page(999).is_err());
        assert!(doc.page_text(999).is_err());
    }

    #[test]
    fn test_metadata() {
        let doc = PlainTextDocument::from_string("Hello");
        let meta = doc.metadata();
        assert_eq!(meta.page_count, 1);
        assert_eq!(meta.creator.as_deref(), Some("justpdf"));
    }

    #[test]
    fn test_format_document_text() {
        let doc = PlainTextDocument::from_string("Page1 content");
        let all_text = doc.text().unwrap();
        assert!(all_text.contains("Page1 content"));
    }

    #[test]
    fn test_word_wrap() {
        // Create a very long line that must be wrapped
        let long_line = "a ".repeat(200);
        let doc = PlainTextDocument::from_string(&long_line);
        let text = doc.page_text(0).unwrap();
        // The text should be split across multiple lines
        assert!(text.lines().count() > 1);
    }
}
