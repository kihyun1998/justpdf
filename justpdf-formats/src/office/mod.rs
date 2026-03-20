//! Office document text extraction (DOCX, XLSX, PPTX).
//!
//! Extracts plain text from Microsoft Office Open XML formats.
//! These are ZIP archives containing XML files with the document content.

use std::io::{Cursor, Read};
use std::path::Path;

use crate::common::{FormatDocument, FormatMetadata, FormatPage, RenderedPage};
use crate::error::FormatError;
use crate::Result;

/// Supported Office document types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OfficeType {
    Docx,
    Xlsx,
    Pptx,
}

/// An Office document with extracted text.
pub struct OfficeDocument {
    /// Document type.
    doc_type: OfficeType,
    /// Extracted text pages. For DOCX, the entire document is one "page".
    /// For XLSX, each sheet is a "page". For PPTX, each slide is a "page".
    pages: Vec<String>,
    /// Document title from metadata.
    title: Option<String>,
    /// Document author from metadata.
    author: Option<String>,
}

impl OfficeDocument {
    /// Open an Office file. The type is detected from the file extension.
    pub fn open(path: &Path) -> Result<Self> {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_default();
        let doc_type = match ext.as_str() {
            "docx" => OfficeType::Docx,
            "xlsx" => OfficeType::Xlsx,
            "pptx" => OfficeType::Pptx,
            _ => {
                return Err(FormatError::UnsupportedFormat { extension: ext });
            }
        };
        let data = std::fs::read(path)?;
        Self::from_bytes(&data, doc_type)
    }

    /// Parse Office document from bytes.
    pub fn from_bytes(data: &[u8], doc_type: OfficeType) -> Result<Self> {
        let reader = Cursor::new(data);
        let mut archive = zip::ZipArchive::new(reader)
            .map_err(|e| FormatError::Zip(format!("{e}")))?;

        let (title, author) = extract_office_metadata(&mut archive);

        let pages = match doc_type {
            OfficeType::Docx => extract_docx_text(&mut archive)?,
            OfficeType::Xlsx => extract_xlsx_text(&mut archive)?,
            OfficeType::Pptx => extract_pptx_text(&mut archive)?,
        };

        // Ensure at least one page
        let pages = if pages.is_empty() {
            vec![String::new()]
        } else {
            pages
        };

        Ok(Self {
            doc_type,
            pages,
            title,
            author,
        })
    }
}

/// Extract metadata from docProps/core.xml.
fn extract_office_metadata(
    archive: &mut zip::ZipArchive<Cursor<&[u8]>>,
) -> (Option<String>, Option<String>) {
    let mut title = None;
    let mut author = None;

    if let Ok(xml) = read_zip_text(archive, "docProps/core.xml") {
        if let Ok(doc) = roxmltree::Document::parse(&xml) {
            for node in doc.descendants() {
                match node.tag_name().name() {
                    "title" => {
                        if let Some(t) = node.text() {
                            let t = t.trim();
                            if !t.is_empty() {
                                title = Some(t.to_string());
                            }
                        }
                    }
                    "creator" => {
                        if let Some(a) = node.text() {
                            let a = a.trim();
                            if !a.is_empty() {
                                author = Some(a.to_string());
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    (title, author)
}

/// Extract text from a DOCX file.
///
/// DOCX structure: word/document.xml contains paragraphs (<w:p>) with runs
/// (<w:r>) containing text (<w:t>).
fn extract_docx_text(
    archive: &mut zip::ZipArchive<Cursor<&[u8]>>,
) -> Result<Vec<String>> {
    let xml = read_zip_text(archive, "word/document.xml")?;
    let doc = roxmltree::Document::parse(&xml)
        .map_err(|e| FormatError::Xml(format!("parsing document.xml: {e}")))?;

    let mut paragraphs = Vec::new();

    for node in doc.descendants() {
        // Match paragraph elements (w:p)
        if node.tag_name().name() == "p"
            && is_word_namespace(node.tag_name().namespace())
        {
            let mut para_text = String::new();
            extract_docx_paragraph_text(&node, &mut para_text);
            paragraphs.push(para_text);
        }
    }

    // All paragraphs form one logical page
    Ok(vec![paragraphs.join("\n")])
}

/// Extract text from a paragraph element.
fn extract_docx_paragraph_text(node: &roxmltree::Node<'_, '_>, result: &mut String) {
    for child in node.children() {
        if !child.is_element() {
            continue;
        }
        match child.tag_name().name() {
            // Run element (w:r)
            "r" if is_word_namespace(child.tag_name().namespace()) => {
                for run_child in child.children() {
                    if run_child.is_element()
                        && run_child.tag_name().name() == "t"
                        && is_word_namespace(run_child.tag_name().namespace())
                    {
                        if let Some(text) = run_child.text() {
                            result.push_str(text);
                        }
                    }
                    // Handle tab characters
                    if run_child.is_element()
                        && run_child.tag_name().name() == "tab"
                        && is_word_namespace(run_child.tag_name().namespace())
                    {
                        result.push('\t');
                    }
                    // Handle line breaks
                    if run_child.is_element()
                        && run_child.tag_name().name() == "br"
                        && is_word_namespace(run_child.tag_name().namespace())
                    {
                        result.push('\n');
                    }
                }
            }
            // Hyperlink
            "hyperlink" => {
                extract_docx_paragraph_text(&child, result);
            }
            _ => {}
        }
    }
}

fn is_word_namespace(ns: Option<&str>) -> bool {
    match ns {
        Some(ns) => {
            ns.contains("wordprocessingml")
                || ns.contains("schemas.openxmlformats.org/wordprocessingml")
        }
        None => true, // Allow unnamespaced elements too
    }
}

/// Extract text from an XLSX file.
///
/// XLSX structure:
/// - xl/sharedStrings.xml contains the shared string table
/// - xl/worksheets/sheet*.xml contains cell data referencing the string table
fn extract_xlsx_text(
    archive: &mut zip::ZipArchive<Cursor<&[u8]>>,
) -> Result<Vec<String>> {
    // Step 1: Parse shared strings table
    let shared_strings = parse_shared_strings(archive)?;

    // Step 2: Find worksheet files
    let mut sheet_names: Vec<String> = Vec::new();
    for i in 0..archive.len() {
        if let Ok(file) = archive.by_index(i) {
            let name = file.name().to_string();
            if name.starts_with("xl/worksheets/sheet") && name.ends_with(".xml") {
                sheet_names.push(name);
            }
        }
    }
    sheet_names.sort();

    if sheet_names.is_empty() {
        return Ok(vec![String::new()]);
    }

    // Step 3: Parse each worksheet
    let mut pages = Vec::new();
    for sheet_name in &sheet_names {
        let text = parse_xlsx_sheet(archive, sheet_name, &shared_strings)?;
        pages.push(text);
    }

    Ok(pages)
}

/// Parse the shared strings table.
fn parse_shared_strings(
    archive: &mut zip::ZipArchive<Cursor<&[u8]>>,
) -> Result<Vec<String>> {
    let xml = match read_zip_text(archive, "xl/sharedStrings.xml") {
        Ok(xml) => xml,
        Err(_) => return Ok(Vec::new()), // Some XLSX files don't have shared strings
    };

    let doc = roxmltree::Document::parse(&xml)
        .map_err(|e| FormatError::Xml(format!("parsing sharedStrings.xml: {e}")))?;

    let mut strings = Vec::new();
    for node in doc.descendants() {
        if node.tag_name().name() == "si" {
            let mut text = String::new();
            collect_xlsx_si_text(&node, &mut text);
            strings.push(text);
        }
    }

    Ok(strings)
}

/// Collect text from a shared string item (<si>).
fn collect_xlsx_si_text(node: &roxmltree::Node<'_, '_>, result: &mut String) {
    for child in node.descendants() {
        if child.tag_name().name() == "t" {
            if let Some(text) = child.text() {
                result.push_str(text);
            }
        }
    }
}

/// Parse a worksheet and extract cell values as tab-separated text.
fn parse_xlsx_sheet(
    archive: &mut zip::ZipArchive<Cursor<&[u8]>>,
    path: &str,
    shared_strings: &[String],
) -> Result<String> {
    let xml = read_zip_text(archive, path)?;
    let doc = roxmltree::Document::parse(&xml)
        .map_err(|e| FormatError::Xml(format!("parsing worksheet: {e}")))?;

    let mut rows: Vec<Vec<(u32, String)>> = Vec::new();

    for node in doc.descendants() {
        if node.tag_name().name() == "row" {
            let mut cells: Vec<(u32, String)> = Vec::new();

            for cell in node.children() {
                if cell.tag_name().name() != "c" {
                    continue;
                }

                let col_idx = cell
                    .attribute("r")
                    .map(|r| column_index_from_ref(r))
                    .unwrap_or(cells.len() as u32);

                let cell_type = cell.attribute("t").unwrap_or("");

                // Find the value element
                let value = cell
                    .children()
                    .find(|c| c.tag_name().name() == "v")
                    .and_then(|v| v.text())
                    .unwrap_or("");

                let text = match cell_type {
                    "s" => {
                        // Shared string reference
                        if let Ok(idx) = value.parse::<usize>() {
                            shared_strings
                                .get(idx)
                                .cloned()
                                .unwrap_or_default()
                        } else {
                            String::new()
                        }
                    }
                    "inlineStr" => {
                        // Inline string
                        let mut s = String::new();
                        for t_node in cell.descendants() {
                            if t_node.tag_name().name() == "t" {
                                if let Some(t) = t_node.text() {
                                    s.push_str(t);
                                }
                            }
                        }
                        s
                    }
                    _ => value.to_string(),
                };

                cells.push((col_idx, text));
            }

            rows.push(cells);
        }
    }

    // Convert to tab-separated text
    let mut lines = Vec::new();
    for row in &rows {
        if row.is_empty() {
            lines.push(String::new());
            continue;
        }
        let max_col = row.iter().map(|(c, _)| *c).max().unwrap_or(0);
        let mut line_parts: Vec<String> = vec![String::new(); (max_col + 1) as usize];
        for (col, text) in row {
            if (*col as usize) < line_parts.len() {
                line_parts[*col as usize] = text.clone();
            }
        }
        lines.push(line_parts.join("\t"));
    }

    Ok(lines.join("\n"))
}

/// Parse column letter(s) from a cell reference like "B3" -> column index 1.
fn column_index_from_ref(cell_ref: &str) -> u32 {
    let mut col: u32 = 0;
    for ch in cell_ref.chars() {
        if ch.is_ascii_alphabetic() {
            col = col * 26 + (ch.to_ascii_uppercase() as u32 - 'A' as u32 + 1);
        } else {
            break;
        }
    }
    if col > 0 { col - 1 } else { 0 }
}

/// Extract text from a PPTX file.
///
/// PPTX structure: ppt/slides/slide*.xml contains text in <a:t> elements.
fn extract_pptx_text(
    archive: &mut zip::ZipArchive<Cursor<&[u8]>>,
) -> Result<Vec<String>> {
    // Find all slide files
    let mut slide_names: Vec<String> = Vec::new();
    for i in 0..archive.len() {
        if let Ok(file) = archive.by_index(i) {
            let name = file.name().to_string();
            if name.starts_with("ppt/slides/slide") && name.ends_with(".xml") {
                slide_names.push(name);
            }
        }
    }

    // Sort by slide number
    slide_names.sort_by(|a, b| {
        let num_a = extract_slide_number(a);
        let num_b = extract_slide_number(b);
        num_a.cmp(&num_b)
    });

    if slide_names.is_empty() {
        return Ok(vec![String::new()]);
    }

    let mut pages = Vec::new();
    for slide_name in &slide_names {
        let xml = read_zip_text(archive, slide_name)?;
        let doc = roxmltree::Document::parse(&xml)
            .map_err(|e| FormatError::Xml(format!("parsing {slide_name}: {e}")))?;

        let mut text_parts = Vec::new();
        let mut current_paragraph = String::new();

        for node in doc.descendants() {
            match node.tag_name().name() {
                "t" => {
                    // DrawingML text run
                    if let Some(text) = node.text() {
                        current_paragraph.push_str(text);
                    }
                }
                "p" if is_drawingml_namespace(node.tag_name().namespace()) => {
                    // Start of a new paragraph — flush previous
                    if !current_paragraph.is_empty() {
                        text_parts.push(std::mem::take(&mut current_paragraph));
                    }
                }
                _ => {}
            }
        }
        if !current_paragraph.is_empty() {
            text_parts.push(current_paragraph);
        }

        pages.push(text_parts.join("\n"));
    }

    Ok(pages)
}

fn is_drawingml_namespace(ns: Option<&str>) -> bool {
    match ns {
        Some(ns) => ns.contains("drawingml"),
        None => true,
    }
}

/// Extract slide number from filename like "ppt/slides/slide3.xml" -> 3.
fn extract_slide_number(name: &str) -> u32 {
    let stem = name
        .rsplit('/')
        .next()
        .unwrap_or("")
        .strip_prefix("slide")
        .unwrap_or("")
        .strip_suffix(".xml")
        .unwrap_or("");
    stem.parse().unwrap_or(0)
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

impl FormatDocument for OfficeDocument {
    fn metadata(&self) -> FormatMetadata {
        FormatMetadata {
            title: self.title.clone(),
            author: self.author.clone(),
            subject: None,
            creator: Some(format!("justpdf-formats/office-{:?}", self.doc_type)),
            page_count: self.pages.len(),
        }
    }

    fn page_count(&self) -> usize {
        self.pages.len()
    }

    fn page(&self, index: usize) -> Result<FormatPage> {
        if index >= self.pages.len() {
            return Err(FormatError::OutOfRange {
                index,
                count: self.pages.len(),
            });
        }
        // Use US Letter size for all Office documents
        Ok(FormatPage {
            index,
            width_pt: 612.0,
            height_pt: 792.0,
        })
    }

    fn page_text(&self, index: usize) -> Result<String> {
        self.pages
            .get(index)
            .cloned()
            .ok_or(FormatError::OutOfRange {
                index,
                count: self.pages.len(),
            })
    }

    fn render_page(&self, index: usize, dpi: f64) -> Result<RenderedPage> {
        if index >= self.pages.len() {
            return Err(FormatError::OutOfRange {
                index,
                count: self.pages.len(),
            });
        }
        // Render the text content via PDF round-trip
        let text = &self.pages[index];
        let plain = crate::plaintext::PlainTextDocument::from_string(text);
        plain.render_page(0, dpi)
    }

    fn render_page_png(&self, index: usize, dpi: f64) -> Result<Vec<u8>> {
        if index >= self.pages.len() {
            return Err(FormatError::OutOfRange {
                index,
                count: self.pages.len(),
            });
        }
        let text = &self.pages[index];
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

        let page_width = 612.0;
        let page_height = 792.0;
        let margin = 72.0;
        let font_size = 10.0;
        let line_height = 12.0;

        for page_text in &self.pages {
            let mut page = PageBuilder::new(page_width, page_height);
            page.add_font(&font_name, "Courier");
            page.begin_text();
            page.set_font(&font_name, font_size);

            let mut y = page_height - margin;

            for line in page_text.lines() {
                if y < margin {
                    // Would need pagination here for real use
                    break;
                }
                page.move_to(margin, y);
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
    use std::io::Write;

    /// Create a minimal DOCX for testing.
    fn create_test_docx() -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(Cursor::new(&mut buf));
            let opts = zip::write::SimpleFileOptions::default();

            zip.start_file("word/document.xml", opts).unwrap();
            zip.write_all(br#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:r><w:t>Hello World</w:t></w:r>
    </w:p>
    <w:p>
      <w:r><w:t>Second paragraph</w:t></w:r>
    </w:p>
  </w:body>
</w:document>"#).unwrap();

            zip.start_file("docProps/core.xml", opts).unwrap();
            zip.write_all(br#"<?xml version="1.0" encoding="UTF-8"?>
<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties"
    xmlns:dc="http://purl.org/dc/elements/1.1/">
  <dc:title>Test Document</dc:title>
  <dc:creator>Test Author</dc:creator>
</cp:coreProperties>"#).unwrap();

            zip.finish().unwrap();
        }
        buf
    }

    /// Create a minimal XLSX for testing.
    fn create_test_xlsx() -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(Cursor::new(&mut buf));
            let opts = zip::write::SimpleFileOptions::default();

            zip.start_file("xl/sharedStrings.xml", opts).unwrap();
            zip.write_all(br#"<?xml version="1.0" encoding="UTF-8"?>
<sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" count="3">
  <si><t>Name</t></si>
  <si><t>Age</t></si>
  <si><t>Alice</t></si>
</sst>"#).unwrap();

            zip.start_file("xl/worksheets/sheet1.xml", opts).unwrap();
            zip.write_all(br#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData>
    <row r="1">
      <c r="A1" t="s"><v>0</v></c>
      <c r="B1" t="s"><v>1</v></c>
    </row>
    <row r="2">
      <c r="A2" t="s"><v>2</v></c>
      <c r="B2"><v>30</v></c>
    </row>
  </sheetData>
</worksheet>"#).unwrap();

            zip.finish().unwrap();
        }
        buf
    }

    /// Create a minimal PPTX for testing.
    fn create_test_pptx() -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(Cursor::new(&mut buf));
            let opts = zip::write::SimpleFileOptions::default();

            zip.start_file("ppt/slides/slide1.xml", opts).unwrap();
            zip.write_all(br#"<?xml version="1.0" encoding="UTF-8"?>
<p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
       xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
  <p:cSld>
    <p:spTree>
      <p:sp>
        <p:txBody>
          <a:p><a:r><a:t>Slide Title</a:t></a:r></a:p>
          <a:p><a:r><a:t>Bullet point one</a:t></a:r></a:p>
        </p:txBody>
      </p:sp>
    </p:spTree>
  </p:cSld>
</p:sld>"#).unwrap();

            zip.start_file("ppt/slides/slide2.xml", opts).unwrap();
            zip.write_all(br#"<?xml version="1.0" encoding="UTF-8"?>
<p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
       xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
  <p:cSld>
    <p:spTree>
      <p:sp>
        <p:txBody>
          <a:p><a:r><a:t>Second Slide</a:t></a:r></a:p>
        </p:txBody>
      </p:sp>
    </p:spTree>
  </p:cSld>
</p:sld>"#).unwrap();

            zip.finish().unwrap();
        }
        buf
    }

    #[test]
    fn test_docx_text_extraction() {
        let data = create_test_docx();
        let doc = OfficeDocument::from_bytes(&data, OfficeType::Docx).unwrap();
        assert_eq!(doc.page_count(), 1);
        let text = doc.page_text(0).unwrap();
        assert!(text.contains("Hello World"));
        assert!(text.contains("Second paragraph"));
    }

    #[test]
    fn test_docx_metadata() {
        let data = create_test_docx();
        let doc = OfficeDocument::from_bytes(&data, OfficeType::Docx).unwrap();
        let meta = doc.metadata();
        assert_eq!(meta.title.as_deref(), Some("Test Document"));
        assert_eq!(meta.author.as_deref(), Some("Test Author"));
    }

    #[test]
    fn test_xlsx_text_extraction() {
        let data = create_test_xlsx();
        let doc = OfficeDocument::from_bytes(&data, OfficeType::Xlsx).unwrap();
        assert_eq!(doc.page_count(), 1);
        let text = doc.page_text(0).unwrap();
        assert!(text.contains("Name"));
        assert!(text.contains("Age"));
        assert!(text.contains("Alice"));
        assert!(text.contains("30"));
    }

    #[test]
    fn test_pptx_text_extraction() {
        let data = create_test_pptx();
        let doc = OfficeDocument::from_bytes(&data, OfficeType::Pptx).unwrap();
        assert_eq!(doc.page_count(), 2);
        let text1 = doc.page_text(0).unwrap();
        assert!(text1.contains("Slide Title"));
        assert!(text1.contains("Bullet point one"));
        let text2 = doc.page_text(1).unwrap();
        assert!(text2.contains("Second Slide"));
    }

    #[test]
    fn test_office_to_pdf() {
        let data = create_test_docx();
        let doc = OfficeDocument::from_bytes(&data, OfficeType::Docx).unwrap();
        let pdf = doc.to_pdf().unwrap();
        assert!(pdf.starts_with(b"%PDF"));
    }

    #[test]
    fn test_page_out_of_range() {
        let data = create_test_docx();
        let doc = OfficeDocument::from_bytes(&data, OfficeType::Docx).unwrap();
        assert!(doc.page(99).is_err());
        assert!(doc.page_text(99).is_err());
    }

    #[test]
    fn test_column_index_from_ref() {
        assert_eq!(column_index_from_ref("A1"), 0);
        assert_eq!(column_index_from_ref("B3"), 1);
        assert_eq!(column_index_from_ref("Z1"), 25);
        assert_eq!(column_index_from_ref("AA1"), 26);
    }

    #[test]
    fn test_extract_slide_number() {
        assert_eq!(extract_slide_number("ppt/slides/slide1.xml"), 1);
        assert_eq!(extract_slide_number("ppt/slides/slide10.xml"), 10);
    }

    #[test]
    fn test_office_all_text() {
        let data = create_test_docx();
        let doc = OfficeDocument::from_bytes(&data, OfficeType::Docx).unwrap();
        let text = doc.text().unwrap();
        assert!(text.contains("Hello World"));
    }

    #[test]
    fn test_xlsx_tab_separated() {
        let data = create_test_xlsx();
        let doc = OfficeDocument::from_bytes(&data, OfficeType::Xlsx).unwrap();
        let text = doc.page_text(0).unwrap();
        // First row should have Name and Age separated by tab
        let first_line = text.lines().next().unwrap();
        assert!(first_line.contains('\t'));
    }
}
