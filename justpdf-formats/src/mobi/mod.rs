//! MOBI eBook format support.
//!
//! Parses PDB (Palm Database) containers with MOBI headers, decompresses
//! PalmDOC text (LZ77-style compression), strips HTML tags, and provides
//! PDF conversion.

use crate::common::{FormatDocument, FormatMetadata, FormatPage, RenderedPage};
use crate::error::FormatError;
use crate::Result;

use justpdf_core::writer::{DocumentBuilder, PageBuilder};

/// A parsed MOBI document.
pub struct MobiDocument {
    /// Book title from MOBI header or PDB name.
    title: String,
    /// Text pages (paginated for output).
    pages: Vec<Vec<String>>,
}

/// PDB record info.
#[derive(Debug, Clone)]
struct PdbRecord {
    offset: u32,
}

impl MobiDocument {
    /// Parse a MOBI document from raw bytes.
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        if data.len() < 78 {
            return Err(FormatError::Format {
                detail: "file too small for PDB header".into(),
            });
        }

        // PDB header: name (32 bytes), then various fields
        let name = parse_pdb_name(&data[0..32]);

        // Number of records at offset 76..78
        let num_records = u16::from_be_bytes([data[76], data[77]]) as usize;
        if num_records == 0 {
            return Err(FormatError::Format {
                detail: "no PDB records".into(),
            });
        }

        // Record list starts at offset 78, each entry is 8 bytes (4 offset + 4 attr/id)
        let record_list_start = 78;
        let record_list_size = num_records * 8;
        if data.len() < record_list_start + record_list_size {
            return Err(FormatError::Format {
                detail: "truncated PDB record list".into(),
            });
        }

        let mut records = Vec::with_capacity(num_records);
        for i in 0..num_records {
            let base = record_list_start + i * 8;
            let offset =
                u32::from_be_bytes([data[base], data[base + 1], data[base + 2], data[base + 3]]);
            records.push(PdbRecord { offset });
        }

        // Record 0 is the MOBI/PalmDOC header
        let rec0_offset = records[0].offset as usize;
        if data.len() < rec0_offset + 16 {
            return Err(FormatError::Format {
                detail: "truncated PalmDOC header".into(),
            });
        }

        let compression = u16::from_be_bytes([data[rec0_offset], data[rec0_offset + 1]]);
        let text_length =
            u32::from_be_bytes([data[rec0_offset + 4], data[rec0_offset + 5], data[rec0_offset + 6], data[rec0_offset + 7]])
                as usize;
        let text_record_count =
            u16::from_be_bytes([data[rec0_offset + 8], data[rec0_offset + 9]]) as usize;

        // Try to get the full title from MOBI header
        let title = Self::extract_mobi_title(data, rec0_offset).unwrap_or(name);

        // Decompress text records (records 1..=text_record_count)
        let mut full_text = Vec::with_capacity(text_length);
        let max_record = text_record_count.min(records.len() - 1);

        for i in 1..=max_record {
            let start = records[i].offset as usize;
            let end = if i + 1 < records.len() {
                records[i + 1].offset as usize
            } else {
                data.len()
            };

            if start >= data.len() || end > data.len() || start >= end {
                continue;
            }

            let record_data = &data[start..end];

            match compression {
                1 => {
                    // No compression
                    full_text.extend_from_slice(record_data);
                }
                2 => {
                    // PalmDOC compression
                    let decompressed = decompress_palmdoc(record_data)?;
                    full_text.extend_from_slice(&decompressed);
                }
                _ => {
                    return Err(FormatError::Format {
                        detail: format!("unsupported MOBI compression type: {compression}"),
                    });
                }
            }

            if full_text.len() >= text_length {
                full_text.truncate(text_length);
                break;
            }
        }

        // Convert to string and strip HTML tags
        let raw_text = String::from_utf8_lossy(&full_text).to_string();
        let clean_text = strip_html_tags(&raw_text);

        // Paginate
        let pages = paginate(&clean_text);

        Ok(Self { title, pages })
    }

    /// Open a MOBI file from disk.
    pub fn open(path: &std::path::Path) -> Result<Self> {
        let data = std::fs::read(path)?;
        Self::from_bytes(&data)
    }

    /// Try to extract the full title from the MOBI header.
    fn extract_mobi_title(data: &[u8], rec0_offset: usize) -> Option<String> {
        // MOBI header starts at rec0_offset + 16
        let mobi_start = rec0_offset + 16;
        if data.len() < mobi_start + 92 {
            return None;
        }

        // Full name offset at mobi_start + 84 (relative to rec0_offset)
        let full_name_offset = u32::from_be_bytes([
            data[mobi_start + 84 - 16],
            data[mobi_start + 85 - 16],
            data[mobi_start + 86 - 16],
            data[mobi_start + 87 - 16],
        ]) as usize;

        // Full name length at mobi_start + 88
        let full_name_length = u32::from_be_bytes([
            data[mobi_start + 88 - 16],
            data[mobi_start + 89 - 16],
            data[mobi_start + 90 - 16],
            data[mobi_start + 91 - 16],
        ]) as usize;

        let abs_offset = rec0_offset + full_name_offset;
        if abs_offset + full_name_length <= data.len() && full_name_length > 0 {
            let name_bytes = &data[abs_offset..abs_offset + full_name_length];
            let name = String::from_utf8_lossy(name_bytes).trim().to_string();
            if !name.is_empty() {
                return Some(name);
            }
        }

        None
    }

    /// Build PDF bytes for a single page.
    fn build_page_pdf(&self, page_lines: &[String]) -> Result<Vec<u8>> {
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
        let mut y = page_height - margin - font_size;

        for line in page_lines {
            page.move_to(margin, y);
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

/// Parse the PDB name field (32 bytes, null-terminated).
fn parse_pdb_name(data: &[u8]) -> String {
    let end = data.iter().position(|&b| b == 0).unwrap_or(data.len());
    String::from_utf8_lossy(&data[..end]).trim().to_string()
}

/// Decompress PalmDOC (LZ77-style) compressed data.
fn decompress_palmdoc(input: &[u8]) -> Result<Vec<u8>> {
    let mut output = Vec::with_capacity(input.len() * 2);
    let mut i = 0;

    while i < input.len() {
        let byte = input[i];
        i += 1;

        match byte {
            // 0x00: literal null byte
            0x00 => {
                output.push(0);
            }
            // 0x01..=0x08: copy next N bytes literally
            0x01..=0x08 => {
                let count = byte as usize;
                let end = (i + count).min(input.len());
                output.extend_from_slice(&input[i..end]);
                i = end;
            }
            // 0x09..=0x7F: direct byte (literal)
            0x09..=0x7F => {
                output.push(byte);
            }
            // 0x80..=0xBF: length-distance pair (back reference)
            0x80..=0xBF => {
                if i >= input.len() {
                    break;
                }
                let next = input[i];
                i += 1;

                // Distance is top 5 bits of (byte - 0x80) << 3 | top 3 bits of next
                let combined = ((byte as u16 & 0x3F) << 8) | next as u16;
                let distance = (combined >> 3) as usize;
                let length = (combined & 0x07) as usize + 3;

                if distance == 0 || distance > output.len() {
                    // Invalid back reference, skip
                    continue;
                }

                let start = output.len() - distance;
                for j in 0..length {
                    let idx = start + (j % distance);
                    if idx < output.len() {
                        output.push(output[idx]);
                    }
                }
            }
            // 0xC0..=0xFF: space + character
            0xC0..=0xFF => {
                output.push(b' ');
                output.push(byte ^ 0x80);
            }
        }
    }

    Ok(output)
}

/// Strip HTML tags from text, replacing block elements with newlines.
fn strip_html_tags(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let mut in_tag = false;
    let mut tag_name = String::new();
    let mut collecting_tag_name = false;

    for ch in html.chars() {
        if ch == '<' {
            in_tag = true;
            tag_name.clear();
            collecting_tag_name = true;
            continue;
        }
        if ch == '>' {
            in_tag = false;
            collecting_tag_name = false;
            // Add newline for block-level tags
            let tag_lower = tag_name.to_lowercase();
            if tag_lower == "p"
                || tag_lower == "/p"
                || tag_lower == "br"
                || tag_lower == "br/"
                || tag_lower == "div"
                || tag_lower == "/div"
                || tag_lower == "h1"
                || tag_lower == "/h1"
                || tag_lower == "h2"
                || tag_lower == "/h2"
                || tag_lower == "h3"
                || tag_lower == "/h3"
            {
                result.push('\n');
            }
            continue;
        }
        if in_tag {
            if collecting_tag_name && !ch.is_whitespace() {
                tag_name.push(ch);
            } else {
                collecting_tag_name = false;
            }
        } else {
            result.push(ch);
        }
    }

    // Decode common HTML entities
    let result = result
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&nbsp;", " ")
        .replace("&#39;", "'");

    // Collapse multiple newlines
    let mut collapsed = String::with_capacity(result.len());
    let mut prev_newline = false;
    for ch in result.chars() {
        if ch == '\n' || ch == '\r' {
            if !prev_newline {
                collapsed.push('\n');
            }
            prev_newline = true;
        } else {
            prev_newline = false;
            collapsed.push(ch);
        }
    }

    collapsed.trim().to_string()
}

/// Paginate text into pages of lines.
fn paginate(text: &str) -> Vec<Vec<String>> {
    let page_width: f64 = 612.0;
    let page_height: f64 = 792.0;
    let margin: f64 = 72.0;
    let font_size: f64 = 10.0;
    let line_spacing: f64 = 1.2;

    let usable_width = page_width - 2.0 * margin;
    let usable_height = page_height - 2.0 * margin;
    let line_height = font_size * line_spacing;
    let char_width = font_size * 0.6;
    let chars_per_line = (usable_width / char_width).floor() as usize;
    let lines_per_page = (usable_height / line_height).floor() as usize;

    let mut all_lines = Vec::new();
    for line in text.lines() {
        if line.is_empty() {
            all_lines.push(String::new());
        } else {
            let mut remaining = line;
            while !remaining.is_empty() {
                if remaining.len() <= chars_per_line {
                    all_lines.push(remaining.to_string());
                    break;
                }
                let break_at = remaining[..chars_per_line]
                    .rfind(' ')
                    .unwrap_or(chars_per_line);
                all_lines.push(remaining[..break_at].to_string());
                remaining = remaining[break_at..].trim_start();
            }
        }
    }

    let pages: Vec<Vec<String>> = all_lines
        .chunks(lines_per_page.max(1))
        .map(|chunk| chunk.to_vec())
        .collect();

    if pages.is_empty() {
        vec![vec![]]
    } else {
        pages
    }
}

impl FormatDocument for MobiDocument {
    fn metadata(&self) -> FormatMetadata {
        FormatMetadata {
            title: Some(self.title.clone()),
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
            return Err(FormatError::OutOfRange {
                index,
                count: self.pages.len(),
            });
        }
        Ok(FormatPage {
            index,
            width_pt: 612.0,
            height_pt: 792.0,
        })
    }

    fn page_text(&self, index: usize) -> Result<String> {
        if index >= self.pages.len() {
            return Err(FormatError::OutOfRange {
                index,
                count: self.pages.len(),
            });
        }
        Ok(self.pages[index].join("\n"))
    }

    fn render_page(&self, index: usize, dpi: f64) -> Result<RenderedPage> {
        if index >= self.pages.len() {
            return Err(FormatError::OutOfRange {
                index,
                count: self.pages.len(),
            });
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
            return Err(FormatError::OutOfRange {
                index,
                count: self.pages.len(),
            });
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
        let page_width = 612.0;
        let page_height = 792.0;
        let margin = 72.0;
        let font_size = 10.0;
        let line_spacing = 1.2;

        let mut builder = DocumentBuilder::new();
        let font_name = builder.add_standard_font("Courier");

        for page_lines in &self.pages {
            let mut page = PageBuilder::new(page_width, page_height);
            page.add_font(&font_name, "Courier");
            page.begin_text();
            page.set_font(&font_name, font_size);

            let line_height = font_size * line_spacing;
            let mut y = page_height - margin - font_size;

            for line in page_lines {
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

    /// Build a minimal valid PDB/MOBI file for testing.
    fn build_test_mobi(text: &str, compress: bool) -> Vec<u8> {
        let mut data = Vec::new();

        // PDB header (78 bytes)
        // Name (32 bytes)
        let name = b"Test Book\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0";
        data.extend_from_slice(name);

        // Attributes (2), version (2), created (4), modified (4), backup (4)
        data.extend_from_slice(&[0u8; 16]);

        // Modification number (4), app info offset (4), sort info offset (4)
        data.extend_from_slice(&[0u8; 12]);

        // Type "BOOK" (4)
        data.extend_from_slice(b"BOOK");
        // Creator "MOBI" (4)
        data.extend_from_slice(b"MOBI");

        // Unique ID seed (4), next record list (4)
        data.extend_from_slice(&[0u8; 8]);

        // Number of records: 2 (header record + 1 text record)
        data.extend_from_slice(&2u16.to_be_bytes());

        // Record list (2 records, each 8 bytes)
        // Record 0 (header) - offset will be at 78 + 16 = 94
        let rec0_offset: u32 = 94;
        data.extend_from_slice(&rec0_offset.to_be_bytes());
        data.extend_from_slice(&[0u8; 4]); // attributes + id

        // Prepare text content
        let text_bytes = if compress {
            compress_palmdoc_simple(text.as_bytes())
        } else {
            text.as_bytes().to_vec()
        };

        // Record 1 (text) - offset after header record
        // Header record is 16 bytes minimum
        let rec1_offset: u32 = rec0_offset + 16;
        data.extend_from_slice(&rec1_offset.to_be_bytes());
        data.extend_from_slice(&[0u8; 4]);

        // Record 0: PalmDOC header (16 bytes)
        let compression: u16 = if compress { 2 } else { 1 };
        data.extend_from_slice(&compression.to_be_bytes()); // compression
        data.extend_from_slice(&[0u8; 2]); // unused
        data.extend_from_slice(&(text.len() as u32).to_be_bytes()); // text length
        data.extend_from_slice(&1u16.to_be_bytes()); // record count
        data.extend_from_slice(&4096u16.to_be_bytes()); // record size
        data.extend_from_slice(&[0u8; 4]); // current position

        // Record 1: text data
        data.extend_from_slice(&text_bytes);

        data
    }

    /// Simple PalmDOC "compression" for testing - just wraps literal bytes.
    fn compress_palmdoc_simple(input: &[u8]) -> Vec<u8> {
        let mut output = Vec::new();
        let mut i = 0;
        while i < input.len() {
            let byte = input[i];
            if byte == 0 {
                output.push(0);
            } else if byte >= 0x09 && byte <= 0x7F {
                output.push(byte);
            } else {
                // Use literal copy
                output.push(1); // copy 1 byte
                output.push(byte);
            }
            i += 1;
        }
        output
    }

    #[test]
    fn test_parse_mobi_uncompressed() {
        let mobi_data = build_test_mobi("Hello, MOBI World!", false);
        let doc = MobiDocument::from_bytes(&mobi_data).unwrap();
        assert_eq!(doc.title, "Test Book");
        let text = doc.page_text(0).unwrap();
        assert!(text.contains("Hello, MOBI World!"));
    }

    #[test]
    fn test_parse_mobi_compressed() {
        let mobi_data = build_test_mobi("Compressed text here.", true);
        let doc = MobiDocument::from_bytes(&mobi_data).unwrap();
        let text = doc.page_text(0).unwrap();
        assert!(text.contains("Compressed text here."));
    }

    #[test]
    fn test_mobi_to_pdf() {
        let mobi_data = build_test_mobi("PDF output test.", false);
        let doc = MobiDocument::from_bytes(&mobi_data).unwrap();
        let pdf = doc.to_pdf().unwrap();
        assert!(pdf.starts_with(b"%PDF"));
    }

    #[test]
    fn test_mobi_too_small() {
        let data = vec![0u8; 10];
        let result = MobiDocument::from_bytes(&data);
        assert!(result.is_err());
    }

    #[test]
    fn test_strip_html() {
        let html = "<html><body><p>Hello</p><p>World</p></body></html>";
        let text = strip_html_tags(html);
        assert!(text.contains("Hello"));
        assert!(text.contains("World"));
        assert!(!text.contains("<p>"));
    }

    #[test]
    fn test_decompress_palmdoc_literal() {
        // Test literal bytes (0x09..0x7F)
        let input = vec![0x48, 0x65, 0x6C, 0x6C, 0x6F]; // "Hello"
        let output = decompress_palmdoc(&input).unwrap();
        assert_eq!(output, b"Hello");
    }

    #[test]
    fn test_decompress_palmdoc_space_char() {
        // 0xC0..0xFF: space + (byte ^ 0x80)
        // 0xE1 => space + 'a' (0x61)
        let input = vec![0xE1];
        let output = decompress_palmdoc(&input).unwrap();
        assert_eq!(output, b" a");
    }

    #[test]
    fn test_mobi_page_out_of_range() {
        let mobi_data = build_test_mobi("Test", false);
        let doc = MobiDocument::from_bytes(&mobi_data).unwrap();
        assert!(doc.page(999).is_err());
        assert!(doc.page_text(999).is_err());
    }
}
