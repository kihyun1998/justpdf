//! EPUB format support.
//!
//! EPUB files are ZIP archives containing XHTML content files organized
//! by an OPF (Open Packaging Format) manifest and spine.

use std::io::{Cursor, Read};
use std::path::Path;

use crate::common::{FormatDocument, FormatMetadata, FormatPage, RenderedPage};
use crate::error::FormatError;
use crate::Result;

/// A parsed EPUB document.
pub struct EpubDocument {
    /// Chapter content (plain text extracted from XHTML).
    chapters: Vec<EpubChapter>,
    /// Document title from OPF metadata.
    title: Option<String>,
    /// Document author from OPF metadata.
    author: Option<String>,
}

/// A single chapter/content file from the EPUB.
struct EpubChapter {
    /// Chapter title (from spine order, may be just a filename).
    #[allow(dead_code)]
    id: String,
    /// Extracted plain text content.
    text: String,
}

impl EpubDocument {
    /// Open an EPUB file.
    pub fn open(path: &Path) -> Result<Self> {
        let data = std::fs::read(path)?;
        Self::from_bytes(&data)
    }

    /// Parse EPUB from bytes.
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        let reader = Cursor::new(data);
        let mut archive = zip::ZipArchive::new(reader)
            .map_err(|e| FormatError::Zip(format!("{e}")))?;

        // Step 1: Parse META-INF/container.xml to find OPF path
        let opf_path = find_opf_path(&mut archive)?;

        // Step 2: Parse OPF for metadata and spine
        let opf_xml = read_zip_text(&mut archive, &opf_path)?;
        let (title, author, manifest, spine) = parse_opf(&opf_xml)?;

        // Step 3: Determine the OPF directory for resolving relative paths
        let opf_dir = if let Some(idx) = opf_path.rfind('/') {
            &opf_path[..idx + 1]
        } else {
            ""
        };

        // Step 4: Extract text from each spine item
        let mut chapters = Vec::new();
        for spine_idref in &spine {
            // Find the manifest item with this id
            if let Some((_, href)) = manifest.iter().find(|(id, _)| id == spine_idref) {
                let full_path = format!("{}{}", opf_dir, href);
                match read_zip_text(&mut archive, &full_path) {
                    Ok(xhtml) => {
                        let text = strip_xml_tags(&xhtml);
                        chapters.push(EpubChapter {
                            id: spine_idref.clone(),
                            text,
                        });
                    }
                    Err(_) => {
                        // Try without the prefix (some EPUBs use absolute paths)
                        if let Ok(xhtml) = read_zip_text(&mut archive, href) {
                            let text = strip_xml_tags(&xhtml);
                            chapters.push(EpubChapter {
                                id: spine_idref.clone(),
                                text,
                            });
                        }
                        // Skip files we can't read
                    }
                }
            }
        }

        if chapters.is_empty() {
            return Err(FormatError::Format {
                detail: "EPUB contains no readable chapters".into(),
            });
        }

        Ok(Self {
            chapters,
            title,
            author,
        })
    }
}

/// Find the OPF (rootfile) path from META-INF/container.xml.
fn find_opf_path(
    archive: &mut zip::ZipArchive<Cursor<&[u8]>>,
) -> Result<String> {
    let xml = read_zip_text(archive, "META-INF/container.xml")?;
    let doc = roxmltree::Document::parse(&xml)
        .map_err(|e| FormatError::Xml(format!("parsing container.xml: {e}")))?;

    for node in doc.descendants() {
        if node.tag_name().name() == "rootfile" {
            if let Some(path) = node.attribute("full-path") {
                return Ok(path.to_string());
            }
        }
    }

    Err(FormatError::Format {
        detail: "no rootfile found in container.xml".into(),
    })
}

/// Parse OPF file for metadata, manifest, and spine.
///
/// Returns (title, author, manifest_items[(id, href)], spine_idrefs[]).
fn parse_opf(
    xml: &str,
) -> Result<(
    Option<String>,
    Option<String>,
    Vec<(String, String)>,
    Vec<String>,
)> {
    let doc = roxmltree::Document::parse(xml)
        .map_err(|e| FormatError::Xml(format!("parsing OPF: {e}")))?;

    let mut title = None;
    let mut author = None;
    let mut manifest = Vec::new();
    let mut spine = Vec::new();

    for node in doc.descendants() {
        match node.tag_name().name() {
            "title" => {
                if title.is_none() {
                    if let Some(t) = node.text() {
                        let t = t.trim();
                        if !t.is_empty() {
                            title = Some(t.to_string());
                        }
                    }
                }
            }
            "creator" => {
                if author.is_none() {
                    if let Some(a) = node.text() {
                        let a = a.trim();
                        if !a.is_empty() {
                            author = Some(a.to_string());
                        }
                    }
                }
            }
            "item" => {
                // Manifest item
                if let (Some(id), Some(href)) =
                    (node.attribute("id"), node.attribute("href"))
                {
                    manifest.push((id.to_string(), href.to_string()));
                }
            }
            "itemref" => {
                // Spine itemref
                if let Some(idref) = node.attribute("idref") {
                    spine.push(idref.to_string());
                }
            }
            _ => {}
        }
    }

    Ok((title, author, manifest, spine))
}

/// Strip XML/HTML tags from content, extracting just the text.
///
/// This is a simple approach that handles basic XHTML:
/// - Removes all tags
/// - Converts block-level elements to newlines
/// - Collapses whitespace
fn strip_xml_tags(xhtml: &str) -> String {
    // Try parsing with roxmltree first for clean extraction
    if let Ok(doc) = roxmltree::Document::parse(xhtml) {
        let mut text = String::new();
        extract_text_recursive(&doc.root(), &mut text);
        return normalize_whitespace(&text);
    }

    // Fallback: simple regex-like tag stripping
    let mut result = String::new();
    let mut in_tag = false;
    let _in_script_or_style = false;

    for ch in xhtml.chars() {
        if ch == '<' {
            in_tag = true;
            continue;
        }
        if ch == '>' {
            in_tag = false;
            continue;
        }
        if in_tag {
            continue;
        }
        if _in_script_or_style {
            continue;
        }
        result.push(ch);
    }
    normalize_whitespace(&result)
}

/// Recursively extract text from parsed XML.
fn extract_text_recursive(node: &roxmltree::Node<'_, '_>, result: &mut String) {
    if node.is_text() {
        if let Some(text) = node.text() {
            result.push_str(text);
        }
        return;
    }

    if node.is_element() {
        let tag = node.tag_name().name();
        // Skip script and style elements
        if tag == "script" || tag == "style" {
            return;
        }

        // Add newline before block-level elements
        let is_block = matches!(
            tag,
            "p" | "div" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "li" | "br"
                | "tr" | "blockquote" | "section" | "article" | "header" | "footer"
                | "pre"
        );
        if is_block && !result.is_empty() && !result.ends_with('\n') {
            result.push('\n');
        }
    }

    for child in node.children() {
        extract_text_recursive(&child, result);
    }

    if node.is_element() {
        let tag = node.tag_name().name();
        let is_block = matches!(
            tag,
            "p" | "div" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "li"
                | "tr" | "blockquote" | "section" | "article"
        );
        if is_block && !result.ends_with('\n') {
            result.push('\n');
        }
    }
}

/// Normalize whitespace: collapse runs, trim lines, remove excess blank lines.
fn normalize_whitespace(s: &str) -> String {
    let mut result = String::new();
    let mut prev_blank = false;

    for line in s.lines() {
        // Collapse whitespace within line
        let trimmed: String = line
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");

        if trimmed.is_empty() {
            if !prev_blank && !result.is_empty() {
                result.push('\n');
                prev_blank = true;
            }
        } else {
            result.push_str(&trimmed);
            result.push('\n');
            prev_blank = false;
        }
    }

    // Remove trailing newlines
    while result.ends_with('\n') {
        result.pop();
    }

    result
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

impl FormatDocument for EpubDocument {
    fn metadata(&self) -> FormatMetadata {
        FormatMetadata {
            title: self.title.clone(),
            author: self.author.clone(),
            subject: None,
            creator: Some("justpdf-formats/epub".to_string()),
            page_count: self.chapters.len(),
        }
    }

    fn page_count(&self) -> usize {
        self.chapters.len()
    }

    fn page(&self, index: usize) -> Result<FormatPage> {
        if index >= self.chapters.len() {
            return Err(FormatError::OutOfRange {
                index,
                count: self.chapters.len(),
            });
        }
        // Use US Letter size for all pages
        Ok(FormatPage {
            index,
            width_pt: 612.0,
            height_pt: 792.0,
        })
    }

    fn page_text(&self, index: usize) -> Result<String> {
        self.chapters
            .get(index)
            .map(|ch| ch.text.clone())
            .ok_or(FormatError::OutOfRange {
                index,
                count: self.chapters.len(),
            })
    }

    fn render_page(&self, index: usize, dpi: f64) -> Result<RenderedPage> {
        if index >= self.chapters.len() {
            return Err(FormatError::OutOfRange {
                index,
                count: self.chapters.len(),
            });
        }
        // Render text via PlainTextDocument
        let text = &self.chapters[index].text;
        let plain = crate::plaintext::PlainTextDocument::from_string(text);
        plain.render_page(0, dpi)
    }

    fn render_page_png(&self, index: usize, dpi: f64) -> Result<Vec<u8>> {
        if index >= self.chapters.len() {
            return Err(FormatError::OutOfRange {
                index,
                count: self.chapters.len(),
            });
        }
        let text = &self.chapters[index].text;
        let plain = crate::plaintext::PlainTextDocument::from_string(text);
        plain.render_page_png(0, dpi)
    }

    fn to_pdf(&self) -> Result<Vec<u8>> {
        use justpdf_core::writer::{DocumentBuilder, PageBuilder};

        let mut builder = DocumentBuilder::new();
        let font_name = builder.add_standard_font("Courier");

        if let Some(ref title) = self.title {
            builder.set_title(title);
        }
        if let Some(ref author) = self.author {
            builder.set_author(author);
        }

        let page_width: f64 = 612.0;
        let page_height: f64 = 792.0;
        let margin: f64 = 72.0;
        let font_size: f64 = 10.0;
        let line_height: f64 = 12.0;
        let usable_height = page_height - 2.0 * margin;
        let lines_per_page = usable_height.div_euclid(line_height) as usize;

        for chapter in &self.chapters {
            let lines: Vec<&str> = chapter.text.lines().collect();

            if lines.is_empty() {
                // Still add a blank page
                let page = PageBuilder::new(page_width, page_height);
                builder.add_page(page);
                continue;
            }

            // Paginate chapter text
            for chunk in lines.chunks(lines_per_page.max(1)) {
                let mut page = PageBuilder::new(page_width, page_height);
                page.add_font(&font_name, "Courier");
                page.begin_text();
                page.set_font(&font_name, font_size);

                let mut y = page_height - margin;

                for line in chunk {
                    page.move_to(margin, y);
                    if !line.is_empty() {
                        page.show_text(line);
                    }
                    y -= line_height;
                }

                page.end_text();
                builder.add_page(page);
            }
        }

        Ok(builder.build()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Create a minimal EPUB for testing.
    fn create_test_epub() -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(Cursor::new(&mut buf));
            let opts = zip::write::SimpleFileOptions::default();

            // mimetype (should be first and uncompressed in a real EPUB)
            zip.start_file("mimetype", opts).unwrap();
            zip.write_all(b"application/epub+zip").unwrap();

            // container.xml
            zip.start_file("META-INF/container.xml", opts).unwrap();
            zip.write_all(br#"<?xml version="1.0" encoding="UTF-8"?>
<container xmlns="urn:oasis:names:tc:opendocument:xmlns:container" version="1.0">
  <rootfiles>
    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>"#).unwrap();

            // OPF file
            zip.start_file("OEBPS/content.opf", opts).unwrap();
            zip.write_all(br#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>Test Book</dc:title>
    <dc:creator>Test Author</dc:creator>
  </metadata>
  <manifest>
    <item id="ch1" href="chapter1.xhtml" media-type="application/xhtml+xml"/>
    <item id="ch2" href="chapter2.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine>
    <itemref idref="ch1"/>
    <itemref idref="ch2"/>
  </spine>
</package>"#).unwrap();

            // Chapter 1
            zip.start_file("OEBPS/chapter1.xhtml", opts).unwrap();
            zip.write_all(br#"<?xml version="1.0" encoding="UTF-8"?>
<html xmlns="http://www.w3.org/1999/xhtml">
<head><title>Chapter 1</title></head>
<body>
  <h1>Chapter One</h1>
  <p>This is the first paragraph of chapter one.</p>
  <p>This is the second paragraph.</p>
</body>
</html>"#).unwrap();

            // Chapter 2
            zip.start_file("OEBPS/chapter2.xhtml", opts).unwrap();
            zip.write_all(br#"<?xml version="1.0" encoding="UTF-8"?>
<html xmlns="http://www.w3.org/1999/xhtml">
<head><title>Chapter 2</title></head>
<body>
  <h1>Chapter Two</h1>
  <p>Content of chapter two goes here.</p>
</body>
</html>"#).unwrap();

            zip.finish().unwrap();
        }
        buf
    }

    #[test]
    fn test_parse_epub() {
        let data = create_test_epub();
        let doc = EpubDocument::from_bytes(&data).unwrap();
        assert_eq!(doc.page_count(), 2);
    }

    #[test]
    fn test_epub_metadata() {
        let data = create_test_epub();
        let doc = EpubDocument::from_bytes(&data).unwrap();
        let meta = doc.metadata();
        assert_eq!(meta.title.as_deref(), Some("Test Book"));
        assert_eq!(meta.author.as_deref(), Some("Test Author"));
        assert_eq!(meta.page_count, 2);
    }

    #[test]
    fn test_epub_text_extraction() {
        let data = create_test_epub();
        let doc = EpubDocument::from_bytes(&data).unwrap();

        let ch1_text = doc.page_text(0).unwrap();
        assert!(ch1_text.contains("Chapter One"));
        assert!(ch1_text.contains("first paragraph"));

        let ch2_text = doc.page_text(1).unwrap();
        assert!(ch2_text.contains("Chapter Two"));
    }

    #[test]
    fn test_epub_to_pdf() {
        let data = create_test_epub();
        let doc = EpubDocument::from_bytes(&data).unwrap();
        let pdf = doc.to_pdf().unwrap();
        assert!(pdf.starts_with(b"%PDF"));
    }

    #[test]
    fn test_epub_page_out_of_range() {
        let data = create_test_epub();
        let doc = EpubDocument::from_bytes(&data).unwrap();
        assert!(doc.page(99).is_err());
        assert!(doc.page_text(99).is_err());
    }

    #[test]
    fn test_strip_xml_tags() {
        let xhtml = r#"<?xml version="1.0"?>
<html xmlns="http://www.w3.org/1999/xhtml">
<body><p>Hello <b>World</b></p><p>Second</p></body>
</html>"#;
        let text = strip_xml_tags(xhtml);
        assert!(text.contains("Hello World"));
        assert!(text.contains("Second"));
    }

    #[test]
    fn test_normalize_whitespace() {
        let input = "  Hello   World  \n\n\n  Second  Line  \n";
        let result = normalize_whitespace(input);
        assert_eq!(result, "Hello World\n\nSecond Line");
    }

    #[test]
    fn test_epub_all_text() {
        let data = create_test_epub();
        let doc = EpubDocument::from_bytes(&data).unwrap();
        let text = doc.text().unwrap();
        assert!(text.contains("Chapter One"));
        assert!(text.contains("Chapter Two"));
    }

    #[test]
    fn test_epub_page_info() {
        let data = create_test_epub();
        let doc = EpubDocument::from_bytes(&data).unwrap();
        let page = doc.page(0).unwrap();
        assert_eq!(page.width_pt, 612.0);
        assert_eq!(page.height_pt, 792.0);
    }
}
