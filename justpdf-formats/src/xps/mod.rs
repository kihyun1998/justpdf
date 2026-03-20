//! XPS (XML Paper Specification) format support.
//!
//! XPS files are ZIP archives containing FixedDocumentSequence, FixedDocument,
//! and FixedPage XML files. Each FixedPage contains Path, Glyphs, and Canvas
//! elements describing the page content.

use std::io::{Cursor, Read};
use std::path::Path;

use crate::common::{FormatDocument, FormatMetadata, FormatPage, RenderedPage};
use crate::error::FormatError;
use crate::Result;

/// A parsed XPS document.
pub struct XpsDocument {
    /// Parsed pages.
    pages: Vec<XpsPage>,
    /// Document title from metadata (if available).
    title: Option<String>,
    /// Document author from metadata (if available).
    author: Option<String>,
}

/// A single XPS page.
struct XpsPage {
    /// Page width in 1/96 inch units (XPS default), stored as points (1/72 inch).
    width_pt: f64,
    /// Page height in points.
    height_pt: f64,
    /// Text content extracted from Glyphs elements.
    text: String,
    /// Raw XML content for rendering.
    xml: String,
}

impl XpsDocument {
    /// Open an XPS file.
    pub fn open(path: &Path) -> Result<Self> {
        let data = std::fs::read(path)?;
        Self::from_bytes(&data)
    }

    /// Parse XPS from bytes.
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        let reader = Cursor::new(data);
        let mut archive = zip::ZipArchive::new(reader)
            .map_err(|e| FormatError::Zip(format!("{e}")))?;

        // Step 1: Find the FixedDocumentSequence
        let fdseq_path = find_fixed_doc_sequence(&mut archive)?;

        // Step 2: Parse FixedDocumentSequence to get FixedDocument paths
        let doc_paths = parse_fixed_doc_sequence(&mut archive, &fdseq_path)?;

        // Step 3: Parse each FixedDocument to get FixedPage paths
        let mut page_paths = Vec::new();
        for doc_path in &doc_paths {
            let pages = parse_fixed_document(&mut archive, doc_path)?;
            page_paths.extend(pages);
        }

        if page_paths.is_empty() {
            return Err(FormatError::Format {
                detail: "XPS document contains no pages".into(),
            });
        }

        // Step 4: Parse each FixedPage
        let mut pages = Vec::new();
        for page_path in &page_paths {
            let page = parse_fixed_page(&mut archive, page_path)?;
            pages.push(page);
        }

        // Try to extract metadata
        let (title, author) = extract_metadata(&mut archive);

        Ok(Self {
            pages,
            title,
            author,
        })
    }
}

/// Find the FixedDocumentSequence path from the content types or relationships.
fn find_fixed_doc_sequence(
    archive: &mut zip::ZipArchive<Cursor<&[u8]>>,
) -> Result<String> {
    // Try reading _rels/.rels first
    if let Ok(rels) = read_zip_text(archive, "_rels/.rels") {
        if let Ok(doc) = roxmltree::Document::parse(&rels) {
            for node in doc.descendants() {
                if node.tag_name().name() == "Relationship" {
                    if let Some(rel_type) = node.attribute("Type") {
                        if rel_type.contains("fixeddocumentsequence")
                            || rel_type.contains("FixedDocumentSequence")
                        {
                            if let Some(target) = node.attribute("Target") {
                                return Ok(normalize_xps_path(target));
                            }
                        }
                    }
                }
            }
        }
    }

    // Fallback: look for known paths
    let known_paths = [
        "FixedDocumentSequence.fdseq",
        "FixedDocSeq.fdseq",
        "Documents/FixedDocumentSequence.fdseq",
    ];
    for path in &known_paths {
        if archive.by_name(path).is_ok() {
            return Ok(path.to_string());
        }
    }

    // Last resort: scan for .fdseq files
    for i in 0..archive.len() {
        if let Ok(file) = archive.by_index(i) {
            if file.name().ends_with(".fdseq") {
                return Ok(file.name().to_string());
            }
        }
    }

    Err(FormatError::Format {
        detail: "could not find FixedDocumentSequence in XPS archive".into(),
    })
}

/// Parse a FixedDocumentSequence to extract FixedDocument paths.
fn parse_fixed_doc_sequence(
    archive: &mut zip::ZipArchive<Cursor<&[u8]>>,
    path: &str,
) -> Result<Vec<String>> {
    let xml = read_zip_text(archive, path)?;
    let doc = roxmltree::Document::parse(&xml)
        .map_err(|e| FormatError::Xml(format!("parsing FixedDocumentSequence: {e}")))?;

    let mut doc_paths = Vec::new();
    for node in doc.descendants() {
        if node.tag_name().name() == "DocumentReference" {
            if let Some(source) = node.attribute("Source") {
                doc_paths.push(resolve_xps_path(path, source));
            }
        }
    }

    if doc_paths.is_empty() {
        return Err(FormatError::Format {
            detail: "no DocumentReference elements in FixedDocumentSequence".into(),
        });
    }

    Ok(doc_paths)
}

/// Parse a FixedDocument to extract FixedPage paths.
fn parse_fixed_document(
    archive: &mut zip::ZipArchive<Cursor<&[u8]>>,
    path: &str,
) -> Result<Vec<String>> {
    let xml = read_zip_text(archive, path)?;
    let doc = roxmltree::Document::parse(&xml)
        .map_err(|e| FormatError::Xml(format!("parsing FixedDocument: {e}")))?;

    let mut page_paths = Vec::new();
    for node in doc.descendants() {
        if node.tag_name().name() == "PageContent" {
            if let Some(source) = node.attribute("Source") {
                page_paths.push(resolve_xps_path(path, source));
            }
        }
    }

    Ok(page_paths)
}

/// Parse a FixedPage XML file.
fn parse_fixed_page(
    archive: &mut zip::ZipArchive<Cursor<&[u8]>>,
    path: &str,
) -> Result<XpsPage> {
    let xml = read_zip_text(archive, path)?;
    let doc = roxmltree::Document::parse(&xml)
        .map_err(|e| FormatError::Xml(format!("parsing FixedPage {path}: {e}")))?;

    let root = doc.root_element();

    // Parse page dimensions (XPS uses 1/96 inch units)
    let width_96 = root
        .attribute("Width")
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(816.0); // ~8.5 inches
    let height_96 = root
        .attribute("Height")
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(1056.0); // ~11 inches

    // Convert from 1/96 inch to points (1/72 inch)
    let width_pt = width_96 * 72.0 / 96.0;
    let height_pt = height_96 * 72.0 / 96.0;

    // Extract text from Glyphs elements
    let mut text_parts = Vec::new();
    extract_glyphs_text(&root, &mut text_parts);
    let text = text_parts.join("\n");

    Ok(XpsPage {
        width_pt,
        height_pt,
        text,
        xml,
    })
}

/// Recursively extract text from Glyphs elements.
fn extract_glyphs_text(node: &roxmltree::Node<'_, '_>, text_parts: &mut Vec<String>) {
    if node.tag_name().name() == "Glyphs" {
        if let Some(unicode) = node.attribute("UnicodeString") {
            let trimmed = unicode.trim();
            if !trimmed.is_empty() {
                text_parts.push(trimmed.to_string());
            }
        }
    }
    for child in node.children() {
        if child.is_element() {
            extract_glyphs_text(&child, text_parts);
        }
    }
}

/// Try to extract metadata from the XPS core properties.
fn extract_metadata(
    archive: &mut zip::ZipArchive<Cursor<&[u8]>>,
) -> (Option<String>, Option<String>) {
    let mut title = None;
    let mut author = None;

    // Try to read core properties
    let core_props_paths = [
        "docProps/core.xml",
        "metadata/core-properties/1.xml",
    ];

    for path in &core_props_paths {
        if let Ok(xml) = read_zip_text(archive, path) {
            if let Ok(doc) = roxmltree::Document::parse(&xml) {
                for node in doc.descendants() {
                    match node.tag_name().name() {
                        "title" => {
                            if let Some(t) = node.text() {
                                title = Some(t.to_string());
                            }
                        }
                        "creator" => {
                            if let Some(a) = node.text() {
                                author = Some(a.to_string());
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    (title, author)
}

/// Read a text file from the ZIP archive.
fn read_zip_text(
    archive: &mut zip::ZipArchive<Cursor<&[u8]>>,
    path: &str,
) -> Result<String> {
    let mut file = archive
        .by_name(path)
        .map_err(|e| FormatError::Zip(format!("reading {path}: {e}")))?;
    let mut content = String::new();
    file.read_to_string(&mut content)?;
    Ok(content)
}

/// Normalize an XPS path (remove leading slash).
fn normalize_xps_path(path: &str) -> String {
    path.trim_start_matches('/').to_string()
}

/// Resolve a relative path against a base path.
fn resolve_xps_path(base: &str, relative: &str) -> String {
    if !relative.starts_with('/') && !relative.starts_with("..") {
        // Relative to base directory
        if let Some(dir) = base.rfind('/') {
            return format!("{}/{}", &base[..dir], relative);
        }
    }
    normalize_xps_path(relative)
}

impl FormatDocument for XpsDocument {
    fn metadata(&self) -> FormatMetadata {
        FormatMetadata {
            title: self.title.clone(),
            author: self.author.clone(),
            subject: None,
            creator: Some("justpdf-formats/xps".to_string()),
            page_count: self.pages.len(),
        }
    }

    fn page_count(&self) -> usize {
        self.pages.len()
    }

    fn page(&self, index: usize) -> Result<FormatPage> {
        let pg = self.pages.get(index).ok_or(FormatError::OutOfRange {
            index,
            count: self.pages.len(),
        })?;
        Ok(FormatPage {
            index,
            width_pt: pg.width_pt,
            height_pt: pg.height_pt,
        })
    }

    fn page_text(&self, index: usize) -> Result<String> {
        let pg = self.pages.get(index).ok_or(FormatError::OutOfRange {
            index,
            count: self.pages.len(),
        })?;
        Ok(pg.text.clone())
    }

    fn render_page(&self, index: usize, dpi: f64) -> Result<RenderedPage> {
        let pg = self.pages.get(index).ok_or(FormatError::OutOfRange {
            index,
            count: self.pages.len(),
        })?;

        let scale = dpi / 72.0;
        let w = (pg.width_pt * scale).ceil() as u32;
        let h = (pg.height_pt * scale).ceil() as u32;

        if w == 0 || h == 0 {
            return Err(FormatError::Format {
                detail: "XPS page has zero dimensions".into(),
            });
        }

        // Create white background
        let pixels = vec![255u8; (w * h * 4) as usize];

        // v1: return white page with correct dimensions
        // Full rendering of XPS paths/glyphs would go here
        Ok(RenderedPage {
            data: pixels,
            width: w,
            height: h,
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
        )
        .map_err(|e| FormatError::Format {
            detail: format!("PNG encode: {e}"),
        })?;
        Ok(buf)
    }

    fn to_pdf(&self) -> Result<Vec<u8>> {
        use justpdf_core::writer::{DocumentBuilder, PageBuilder};

        let mut builder = DocumentBuilder::new();
        let font_name = builder.add_standard_font("Courier");

        for pg in &self.pages {
            let mut page = PageBuilder::new(pg.width_pt, pg.height_pt);

            // If there's text, render it as simple text layout
            if !pg.text.is_empty() {
                page.add_font(&font_name, "Courier");
                page.begin_text();
                page.set_font(&font_name, 10.0);

                let margin = 72.0;
                let line_height = 12.0;
                let mut y = pg.height_pt - margin;

                for line in pg.text.lines() {
                    if y < margin {
                        break;
                    }
                    page.move_to(margin, y);
                    if !line.is_empty() {
                        page.show_text(line);
                    }
                    y -= line_height;
                }

                page.end_text();
            }

            builder.add_page(page);
        }

        Ok(builder.build()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Create a minimal XPS ZIP for testing.
    fn create_test_xps() -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(Cursor::new(&mut buf));
            let opts = zip::write::SimpleFileOptions::default();

            // _rels/.rels
            zip.start_file("_rels/.rels", opts).unwrap();
            zip.write_all(br#"<?xml version="1.0" encoding="utf-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Type="http://schemas.microsoft.com/xps/2005/06/fixeddocumentsequence" Target="FixedDocumentSequence.fdseq" Id="rId1"/>
</Relationships>"#).unwrap();

            // FixedDocumentSequence
            zip.start_file("FixedDocumentSequence.fdseq", opts).unwrap();
            zip.write_all(br#"<?xml version="1.0" encoding="utf-8"?>
<FixedDocumentSequence xmlns="http://schemas.microsoft.com/xps/2005/06">
  <DocumentReference Source="Documents/1/FixedDocument.fdoc"/>
</FixedDocumentSequence>"#).unwrap();

            // FixedDocument
            zip.start_file("Documents/1/FixedDocument.fdoc", opts).unwrap();
            zip.write_all(br#"<?xml version="1.0" encoding="utf-8"?>
<FixedDocument xmlns="http://schemas.microsoft.com/xps/2005/06">
  <PageContent Source="Pages/1.fpage"/>
</FixedDocument>"#).unwrap();

            // FixedPage
            zip.start_file("Documents/1/Pages/1.fpage", opts).unwrap();
            zip.write_all(br#"<?xml version="1.0" encoding="utf-8"?>
<FixedPage xmlns="http://schemas.microsoft.com/xps/2005/06" Width="816" Height="1056">
  <Glyphs UnicodeString="Hello XPS World" OriginX="96" OriginY="96" FontRenderingEmSize="16" FontUri="/Resources/Fonts/Arial.ttf"/>
  <Glyphs UnicodeString="Second line of text" OriginX="96" OriginY="120" FontRenderingEmSize="16" FontUri="/Resources/Fonts/Arial.ttf"/>
</FixedPage>"#).unwrap();

            zip.finish().unwrap();
        }
        buf
    }

    #[test]
    fn test_parse_xps() {
        let data = create_test_xps();
        let doc = XpsDocument::from_bytes(&data).unwrap();
        assert_eq!(doc.page_count(), 1);
    }

    #[test]
    fn test_xps_page_dimensions() {
        let data = create_test_xps();
        let doc = XpsDocument::from_bytes(&data).unwrap();
        let page = doc.page(0).unwrap();
        // 816 * 72/96 = 612 points (8.5 inches)
        assert!((page.width_pt - 612.0).abs() < 0.1);
        // 1056 * 72/96 = 792 points (11 inches)
        assert!((page.height_pt - 792.0).abs() < 0.1);
    }

    #[test]
    fn test_xps_text_extraction() {
        let data = create_test_xps();
        let doc = XpsDocument::from_bytes(&data).unwrap();
        let text = doc.page_text(0).unwrap();
        assert!(text.contains("Hello XPS World"));
        assert!(text.contains("Second line of text"));
    }

    #[test]
    fn test_xps_metadata() {
        let data = create_test_xps();
        let doc = XpsDocument::from_bytes(&data).unwrap();
        let meta = doc.metadata();
        assert_eq!(meta.page_count, 1);
    }

    #[test]
    fn test_xps_render_png() {
        let data = create_test_xps();
        let doc = XpsDocument::from_bytes(&data).unwrap();
        let png = doc.render_page_png(0, 72.0).unwrap();
        assert_eq!(&png[..4], &[0x89, 0x50, 0x4E, 0x47]);
    }

    #[test]
    fn test_xps_to_pdf() {
        let data = create_test_xps();
        let doc = XpsDocument::from_bytes(&data).unwrap();
        let pdf = doc.to_pdf().unwrap();
        assert!(pdf.starts_with(b"%PDF"));
    }

    #[test]
    fn test_xps_page_out_of_range() {
        let data = create_test_xps();
        let doc = XpsDocument::from_bytes(&data).unwrap();
        assert!(doc.page(5).is_err());
        assert!(doc.page_text(5).is_err());
    }

    #[test]
    fn test_normalize_xps_path() {
        assert_eq!(normalize_xps_path("/Documents/1/doc.fdoc"), "Documents/1/doc.fdoc");
        assert_eq!(normalize_xps_path("Documents/1/doc.fdoc"), "Documents/1/doc.fdoc");
    }

    #[test]
    fn test_resolve_xps_path() {
        assert_eq!(
            resolve_xps_path("Documents/1/FixedDocument.fdoc", "Pages/1.fpage"),
            "Documents/1/Pages/1.fpage"
        );
        assert_eq!(
            resolve_xps_path("FixedDocumentSequence.fdseq", "Documents/1/FixedDocument.fdoc"),
            "Documents/1/FixedDocument.fdoc"
        );
    }

    #[test]
    fn test_xps_all_text() {
        let data = create_test_xps();
        let doc = XpsDocument::from_bytes(&data).unwrap();
        let text = doc.text().unwrap();
        assert!(text.contains("Hello XPS World"));
    }
}
