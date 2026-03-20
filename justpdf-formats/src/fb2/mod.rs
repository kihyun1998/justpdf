//! FB2 (FictionBook 2) format support.
//!
//! FB2 is an XML-based eBook format. This module parses the XML structure,
//! extracts metadata and text content, and provides PDF conversion.

use crate::common::{FormatDocument, FormatMetadata, FormatPage, RenderedPage};
use crate::error::FormatError;
use crate::Result;

use justpdf_core::writer::{DocumentBuilder, PageBuilder};

/// An FB2 (FictionBook 2) document.
pub struct Fb2Document {
    /// Document title.
    title: Option<String>,
    /// Author name.
    author: Option<String>,
    /// Sections (each section = one "page").
    sections: Vec<Fb2Section>,
}

/// A section within an FB2 document.
#[derive(Debug, Clone)]
struct Fb2Section {
    /// Section title (from `<title>`).
    title: Option<String>,
    /// Paragraphs of text.
    paragraphs: Vec<String>,
}

impl Fb2Document {
    /// Parse an FB2 document from XML bytes.
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        let text = std::str::from_utf8(data).map_err(|e| FormatError::Format {
            detail: format!("invalid UTF-8 in FB2: {e}"),
        })?;
        Self::from_str(text)
    }

    /// Parse an FB2 document from an XML string.
    pub fn from_str(xml: &str) -> Result<Self> {
        let doc = roxmltree::Document::parse(xml).map_err(|e| FormatError::Xml(format!("{e}")))?;

        let root = doc.root_element();

        // Extract metadata from <description><title-info>
        let (title, author) = Self::extract_metadata(&root);

        // Extract sections from <body>
        let sections = Self::extract_body(&root);

        // Ensure at least one section
        let sections = if sections.is_empty() {
            vec![Fb2Section {
                title: None,
                paragraphs: vec![],
            }]
        } else {
            sections
        };

        Ok(Self {
            title,
            author,
            sections,
        })
    }

    /// Open an FB2 file from disk.
    pub fn open(path: &std::path::Path) -> Result<Self> {
        let data = std::fs::read(path)?;
        Self::from_bytes(&data)
    }

    fn extract_metadata(root: &roxmltree::Node) -> (Option<String>, Option<String>) {
        let mut title = None;
        let mut author = None;

        // Find <description> element
        for desc in root.children().filter(|n| n.has_tag_name("description")) {
            // Find <title-info>
            for ti in desc.children().filter(|n| n.has_tag_name("title-info")) {
                // Get <book-title>
                for bt in ti.children().filter(|n| n.has_tag_name("book-title")) {
                    let text = collect_text(&bt);
                    if !text.is_empty() {
                        title = Some(text);
                    }
                }

                // Get <author>
                for auth in ti.children().filter(|n| n.has_tag_name("author")) {
                    let mut first = String::new();
                    let mut last = String::new();
                    for child in auth.children() {
                        if child.has_tag_name("first-name") {
                            first = collect_text(&child);
                        } else if child.has_tag_name("last-name") {
                            last = collect_text(&child);
                        }
                    }
                    let name = format!("{first} {last}").trim().to_string();
                    if !name.is_empty() {
                        author = Some(name);
                    }
                }
            }
        }

        (title, author)
    }

    fn extract_body(root: &roxmltree::Node) -> Vec<Fb2Section> {
        let mut sections = Vec::new();

        for body in root.children().filter(|n| n.has_tag_name("body")) {
            Self::extract_sections(&body, &mut sections);
        }

        sections
    }

    fn extract_sections(node: &roxmltree::Node, sections: &mut Vec<Fb2Section>) {
        for child in node.children() {
            if child.has_tag_name("section") {
                let section = Self::parse_section(&child);
                sections.push(section);
            }
        }
    }

    fn parse_section(node: &roxmltree::Node) -> Fb2Section {
        let mut title = None;
        let mut paragraphs = Vec::new();

        for child in node.children() {
            if child.has_tag_name("title") {
                let text = collect_text(&child);
                if !text.is_empty() {
                    title = Some(text);
                }
            } else if child.has_tag_name("p") {
                let text = collect_text(&child);
                if !text.is_empty() {
                    paragraphs.push(text);
                }
            } else if child.has_tag_name("epigraph")
                || child.has_tag_name("poem")
                || child.has_tag_name("cite")
            {
                // Extract text from nested elements
                let text = collect_text(&child);
                if !text.is_empty() {
                    paragraphs.push(text);
                }
            } else if child.has_tag_name("section") {
                // Nested section: flatten into paragraphs of current section
                let nested = Self::parse_section(&child);
                if let Some(t) = &nested.title {
                    paragraphs.push(t.clone());
                }
                paragraphs.extend(nested.paragraphs);
            }
        }

        Fb2Section { title, paragraphs }
    }

    /// Build PDF bytes for a single section.
    fn build_section_pdf(&self, section: &Fb2Section) -> Result<Vec<u8>> {
        let page_width = 612.0;
        let page_height = 792.0;
        let margin = 72.0;
        let font_size = 10.0;
        let line_spacing = 1.2;

        let mut builder = DocumentBuilder::new();
        let font_name = builder.add_standard_font("Courier");

        let mut page = PageBuilder::new(page_width, page_height);
        page.add_font(&font_name, "Courier");
        page.begin_text();
        page.set_font(&font_name, font_size);

        let line_height = font_size * line_spacing;
        let usable_width: f64 = page_width - 2.0 * margin;
        let char_width: f64 = font_size * 0.6;
        let chars_per_line = (usable_width / char_width).floor() as usize;

        let mut y = page_height - margin - font_size;

        // Render title
        if let Some(title) = &section.title {
            page.move_to(margin, y);
            let truncated: String = title.chars().take(chars_per_line).collect();
            page.show_text(&truncated);
            y -= line_height * 2.0; // Extra spacing after title
        }

        // Render paragraphs
        for para in &section.paragraphs {
            let lines = wrap_text(para, chars_per_line);
            for line in &lines {
                if y < margin {
                    break;
                }
                page.move_to(margin, y);
                page.show_text(line);
                y -= line_height;
            }
            y -= line_height * 0.5; // Paragraph spacing
        }

        page.end_text();
        builder.add_page(page);
        Ok(builder.build()?)
    }
}

/// Recursively collect text content from a node and its descendants.
fn collect_text(node: &roxmltree::Node) -> String {
    let mut text = String::new();
    for child in node.children() {
        if child.is_text() {
            if let Some(t) = child.text() {
                text.push_str(t);
            }
        } else {
            text.push_str(&collect_text(&child));
            // Add space after block-like elements
            if child.has_tag_name("p")
                || child.has_tag_name("v")
                || child.has_tag_name("subtitle")
            {
                text.push(' ');
            }
        }
    }
    text.trim().to_string()
}

/// Simple word-wrapping for monospace text.
fn wrap_text(text: &str, max_chars: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut remaining = text;

    while !remaining.is_empty() {
        if remaining.len() <= max_chars {
            lines.push(remaining.to_string());
            break;
        }
        let break_at = remaining[..max_chars]
            .rfind(' ')
            .unwrap_or(max_chars);
        lines.push(remaining[..break_at].to_string());
        remaining = remaining[break_at..].trim_start();
    }

    if lines.is_empty() {
        lines.push(String::new());
    }

    lines
}

impl FormatDocument for Fb2Document {
    fn metadata(&self) -> FormatMetadata {
        FormatMetadata {
            title: self.title.clone(),
            author: self.author.clone(),
            subject: None,
            creator: Some("justpdf".to_string()),
            page_count: self.sections.len(),
        }
    }

    fn page_count(&self) -> usize {
        self.sections.len()
    }

    fn page(&self, index: usize) -> Result<FormatPage> {
        if index >= self.sections.len() {
            return Err(FormatError::OutOfRange {
                index,
                count: self.sections.len(),
            });
        }
        Ok(FormatPage {
            index,
            width_pt: 612.0,
            height_pt: 792.0,
        })
    }

    fn page_text(&self, index: usize) -> Result<String> {
        if index >= self.sections.len() {
            return Err(FormatError::OutOfRange {
                index,
                count: self.sections.len(),
            });
        }
        let section = &self.sections[index];
        let mut text = String::new();
        if let Some(title) = &section.title {
            text.push_str(title);
            text.push('\n');
        }
        for (i, para) in section.paragraphs.iter().enumerate() {
            if i > 0 || section.title.is_some() {
                text.push('\n');
            }
            text.push_str(para);
        }
        Ok(text)
    }

    fn render_page(&self, index: usize, dpi: f64) -> Result<RenderedPage> {
        if index >= self.sections.len() {
            return Err(FormatError::OutOfRange {
                index,
                count: self.sections.len(),
            });
        }
        let pdf_bytes = self.build_section_pdf(&self.sections[index])?;
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
        if index >= self.sections.len() {
            return Err(FormatError::OutOfRange {
                index,
                count: self.sections.len(),
            });
        }
        let pdf_bytes = self.build_section_pdf(&self.sections[index])?;
        let doc = justpdf_core::PdfDocument::from_bytes(pdf_bytes)?;
        let opts = justpdf_render::RenderOptions {
            dpi,
            format: justpdf_render::OutputFormat::Png,
            ..Default::default()
        };
        Ok(justpdf_render::render_page(&doc, 0, &opts)?)
    }

    fn to_pdf(&self) -> Result<Vec<u8>> {
        let page_width = 612.0;
        let page_height = 792.0;
        let margin = 72.0;
        let font_size = 10.0;
        let line_spacing = 1.2;

        let mut builder = DocumentBuilder::new();
        let font_name = builder.add_standard_font("Courier");

        let line_height = font_size * line_spacing;
        let usable_width: f64 = page_width - 2.0 * margin;
        let char_width: f64 = font_size * 0.6;
        let chars_per_line = (usable_width / char_width).floor() as usize;

        for section in &self.sections {
            let mut page = PageBuilder::new(page_width, page_height);
            page.add_font(&font_name, "Courier");
            page.begin_text();
            page.set_font(&font_name, font_size);

            let mut y = page_height - margin - font_size;

            if let Some(title) = &section.title {
                page.move_to(margin, y);
                let truncated: String = title.chars().take(chars_per_line).collect();
                page.show_text(&truncated);
                y -= line_height * 2.0;
            }

            for para in &section.paragraphs {
                let lines = wrap_text(para, chars_per_line);
                for line in &lines {
                    if y < margin {
                        break;
                    }
                    page.move_to(margin, y);
                    page.show_text(line);
                    y -= line_height;
                }
                y -= line_height * 0.5;
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

    fn sample_fb2() -> &'static str {
        r#"<?xml version="1.0" encoding="UTF-8"?>
<FictionBook xmlns="http://www.gribuser.ru/xml/fictionbook/2.0">
  <description>
    <title-info>
      <book-title>Test Book</book-title>
      <author>
        <first-name>John</first-name>
        <last-name>Doe</last-name>
      </author>
    </title-info>
  </description>
  <body>
    <section>
      <title><p>Chapter 1</p></title>
      <p>This is the first paragraph.</p>
      <p>This is the second paragraph.</p>
    </section>
    <section>
      <title><p>Chapter 2</p></title>
      <p>Content of chapter two.</p>
    </section>
  </body>
</FictionBook>"#
    }

    #[test]
    fn test_parse_fb2_metadata() {
        let doc = Fb2Document::from_str(sample_fb2()).unwrap();
        let meta = doc.metadata();
        assert_eq!(meta.title.as_deref(), Some("Test Book"));
        assert_eq!(meta.author.as_deref(), Some("John Doe"));
        assert_eq!(meta.page_count, 2);
    }

    #[test]
    fn test_parse_fb2_sections() {
        let doc = Fb2Document::from_str(sample_fb2()).unwrap();
        assert_eq!(doc.page_count(), 2);

        let text0 = doc.page_text(0).unwrap();
        assert!(text0.contains("Chapter 1"));
        assert!(text0.contains("first paragraph"));
        assert!(text0.contains("second paragraph"));

        let text1 = doc.page_text(1).unwrap();
        assert!(text1.contains("Chapter 2"));
        assert!(text1.contains("chapter two"));
    }

    #[test]
    fn test_fb2_to_pdf() {
        let doc = Fb2Document::from_str(sample_fb2()).unwrap();
        let pdf = doc.to_pdf().unwrap();
        assert!(pdf.starts_with(b"%PDF"));
        let parsed = justpdf_core::PdfDocument::from_bytes(pdf).unwrap();
        assert_eq!(justpdf_core::page::page_count(&parsed).unwrap(), 2);
    }

    #[test]
    fn test_fb2_page_out_of_range() {
        let doc = Fb2Document::from_str(sample_fb2()).unwrap();
        assert!(doc.page(999).is_err());
        assert!(doc.page_text(999).is_err());
    }

    #[test]
    fn test_fb2_invalid_xml() {
        let result = Fb2Document::from_str("<not valid xml");
        assert!(result.is_err());
    }

    #[test]
    fn test_fb2_empty_body() {
        let xml = r#"<?xml version="1.0"?>
<FictionBook xmlns="http://www.gribuser.ru/xml/fictionbook/2.0">
  <description><title-info><book-title>Empty</book-title></title-info></description>
  <body></body>
</FictionBook>"#;
        let doc = Fb2Document::from_str(xml).unwrap();
        assert_eq!(doc.page_count(), 1);
        let text = doc.page_text(0).unwrap();
        assert!(text.is_empty());
    }

    #[test]
    fn test_fb2_render_png() {
        let doc = Fb2Document::from_str(sample_fb2()).unwrap();
        let png = doc.render_page_png(0, 72.0).unwrap();
        assert_eq!(&png[..4], &[0x89, 0x50, 0x4E, 0x47]);
    }
}
