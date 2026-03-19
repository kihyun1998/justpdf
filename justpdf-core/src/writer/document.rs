use std::collections::HashMap;
use std::path::Path;

use crate::error::{JustPdfError, Result};
use crate::object::{IndirectRef, PdfDict, PdfObject};
use crate::writer::encode::make_stream;
use crate::writer::page::PageBuilder;
use crate::writer::serialize::serialize_pdf;
use crate::writer::PdfWriter;

/// High-level builder for creating complete PDF documents.
pub struct DocumentBuilder {
    writer: PdfWriter,
    /// Page indirect references in order.
    pages: Vec<IndirectRef>,
    /// font_name -> (resource_name, font_ref).
    fonts: HashMap<String, (String, IndirectRef)>,
    font_counter: u32,
    /// Document info dictionary entries.
    info: PdfDict,
    /// Pre-allocated Pages object number (for forward reference).
    pages_obj_num: u32,
    /// XMP metadata stream reference (attached to Catalog).
    xmp_ref: Option<IndirectRef>,
    /// Optional encryption configuration.
    encryption: Option<crate::crypto::EncryptionConfig>,
}

impl DocumentBuilder {
    /// Create a new document builder.
    pub fn new() -> Self {
        let mut writer = PdfWriter::new();
        // Pre-allocate the Pages object number so pages can reference their parent.
        let pages_obj_num = writer.alloc_object_num();

        Self {
            writer,
            pages: Vec::new(),
            fonts: HashMap::new(),
            font_counter: 0,
            info: PdfDict::new(),
            pages_obj_num,
            xmp_ref: None,
            encryption: None,
        }
    }

    /// Add a standard Type1 font (e.g. "Helvetica", "Times-Roman", "Courier").
    ///
    /// Returns the resource name (e.g. "F1") to use when drawing text.
    pub fn add_standard_font(&mut self, base_font: &str) -> String {
        // Check if already added
        if let Some((res_name, _)) = self.fonts.get(base_font) {
            return res_name.clone();
        }

        self.font_counter += 1;
        let resource_name = format!("F{}", self.font_counter);

        // Create font dictionary object
        let mut font_dict = PdfDict::new();
        font_dict.insert(b"Type".to_vec(), PdfObject::Name(b"Font".to_vec()));
        font_dict.insert(b"Subtype".to_vec(), PdfObject::Name(b"Type1".to_vec()));
        font_dict.insert(
            b"BaseFont".to_vec(),
            PdfObject::Name(base_font.as_bytes().to_vec()),
        );

        let font_ref = self.writer.add_object(PdfObject::Dict(font_dict));
        self.fonts
            .insert(base_font.to_string(), (resource_name.clone(), font_ref));

        resource_name
    }

    /// Add a page to the document from a `PageBuilder`.
    pub fn add_page(&mut self, page: PageBuilder) {
        let pages_ref = IndirectRef {
            obj_num: self.pages_obj_num,
            gen_num: 0,
        };
        let page_ref = page.build(&mut self.writer, &pages_ref);
        self.pages.push(page_ref);
    }

    /// Set the document title.
    pub fn set_title(&mut self, title: &str) {
        self.info.insert(
            b"Title".to_vec(),
            PdfObject::String(title.as_bytes().to_vec()),
        );
    }

    /// Set the document author.
    pub fn set_author(&mut self, author: &str) {
        self.info.insert(
            b"Author".to_vec(),
            PdfObject::String(author.as_bytes().to_vec()),
        );
    }

    /// Set the document subject.
    pub fn set_subject(&mut self, subject: &str) {
        self.info.insert(
            b"Subject".to_vec(),
            PdfObject::String(subject.as_bytes().to_vec()),
        );
    }

    /// Set the producer field.
    pub fn set_producer(&mut self, producer: &str) {
        self.info.insert(
            b"Producer".to_vec(),
            PdfObject::String(producer.as_bytes().to_vec()),
        );
    }

    /// Set the creator field.
    pub fn set_creator(&mut self, creator: &str) {
        self.info.insert(
            b"Creator".to_vec(),
            PdfObject::String(creator.as_bytes().to_vec()),
        );
    }

    /// Embed a TrueType font from raw TTF data.
    ///
    /// Returns the resource name (e.g. "F1", "F2") for use in page content.
    /// The font is embedded with WinAnsiEncoding and a ToUnicode CMap.
    pub fn embed_truetype_font(&mut self, font_data: &[u8]) -> Result<String> {
        let face = ttf_parser::Face::parse(font_data, 0).map_err(|e| JustPdfError::StreamDecode {
            filter: "TrueType".into(),
            detail: format!("failed to parse TTF: {}", e),
        })?;

        // Extract font metrics
        let units_per_em = face.units_per_em() as f64;
        let scale = 1000.0 / units_per_em;

        let font_name = face
            .names()
            .into_iter()
            .find(|n| n.name_id == ttf_parser::name_id::POST_SCRIPT_NAME)
            .and_then(|n| n.to_string())
            .unwrap_or_else(|| "UnknownFont".to_string());

        let ascent = (face.ascender() as f64 * scale) as i64;
        let descent = (face.descender() as f64 * scale) as i64;
        let bbox = face.global_bounding_box();
        let bbox_arr = vec![
            PdfObject::Integer((bbox.x_min as f64 * scale) as i64),
            PdfObject::Integer((bbox.y_min as f64 * scale) as i64),
            PdfObject::Integer((bbox.x_max as f64 * scale) as i64),
            PdfObject::Integer((bbox.y_max as f64 * scale) as i64),
        ];
        let cap_height = face.capital_height().map(|h| (h as f64 * scale) as i64).unwrap_or(ascent);

        // Embed font file as FontFile2 stream (FlateDecode compressed)
        let (ff2_dict, ff2_data) = make_stream(font_data, true);
        let mut ff2_stream_dict = ff2_dict;
        ff2_stream_dict.insert(
            b"Length1".to_vec(),
            PdfObject::Integer(font_data.len() as i64),
        );
        let ff2_ref = self.writer.add_object(PdfObject::Stream {
            dict: ff2_stream_dict,
            data: ff2_data,
        });

        // Create FontDescriptor
        let mut fd = PdfDict::new();
        fd.insert(b"Type".to_vec(), PdfObject::Name(b"FontDescriptor".to_vec()));
        fd.insert(
            b"FontName".to_vec(),
            PdfObject::Name(font_name.as_bytes().to_vec()),
        );
        fd.insert(b"Flags".to_vec(), PdfObject::Integer(32)); // Nonsymbolic
        fd.insert(b"FontBBox".to_vec(), PdfObject::Array(bbox_arr));
        fd.insert(b"ItalicAngle".to_vec(), PdfObject::Integer(0));
        fd.insert(b"Ascent".to_vec(), PdfObject::Integer(ascent));
        fd.insert(b"Descent".to_vec(), PdfObject::Integer(descent));
        fd.insert(b"CapHeight".to_vec(), PdfObject::Integer(cap_height));
        fd.insert(b"StemV".to_vec(), PdfObject::Integer(80));
        fd.insert(b"FontFile2".to_vec(), PdfObject::Reference(ff2_ref));
        let fd_ref = self.writer.add_object(PdfObject::Dict(fd));

        // Build Widths array for chars 32-255
        let mut widths = Vec::with_capacity(224);
        let mut bfchar_entries: Vec<(u8, u16)> = Vec::new();
        for code in 32u16..=255u16 {
            let ch = code as u8 as char;
            let unicode_val = ch as u16;
            if let Some(glyph_id) = face.glyph_index(ch) {
                let w = face
                    .glyph_hor_advance(glyph_id)
                    .map(|a| (a as f64 * scale) as i64)
                    .unwrap_or(0);
                widths.push(PdfObject::Integer(w));
                bfchar_entries.push((code as u8, unicode_val));
            } else {
                widths.push(PdfObject::Integer(0));
            }
        }

        // Generate ToUnicode CMap
        let tounicode_cmap = generate_tounicode_cmap(&bfchar_entries);
        let (cmap_dict, cmap_data) = make_stream(tounicode_cmap.as_bytes(), true);
        let cmap_ref = self.writer.add_object(PdfObject::Stream {
            dict: cmap_dict,
            data: cmap_data,
        });

        // Create Font dictionary
        self.font_counter += 1;
        let resource_name = format!("F{}", self.font_counter);

        let mut font_dict = PdfDict::new();
        font_dict.insert(b"Type".to_vec(), PdfObject::Name(b"Font".to_vec()));
        font_dict.insert(b"Subtype".to_vec(), PdfObject::Name(b"TrueType".to_vec()));
        font_dict.insert(
            b"BaseFont".to_vec(),
            PdfObject::Name(font_name.as_bytes().to_vec()),
        );
        font_dict.insert(b"FirstChar".to_vec(), PdfObject::Integer(32));
        font_dict.insert(b"LastChar".to_vec(), PdfObject::Integer(255));
        font_dict.insert(b"Widths".to_vec(), PdfObject::Array(widths));
        font_dict.insert(b"FontDescriptor".to_vec(), PdfObject::Reference(fd_ref));
        font_dict.insert(
            b"Encoding".to_vec(),
            PdfObject::Name(b"WinAnsiEncoding".to_vec()),
        );
        font_dict.insert(b"ToUnicode".to_vec(), PdfObject::Reference(cmap_ref));

        let font_ref = self.writer.add_object(PdfObject::Dict(font_dict));
        self.fonts
            .insert(font_name.clone(), (resource_name.clone(), font_ref));

        Ok(resource_name)
    }

    /// Set encryption for the document.
    pub fn set_encryption(&mut self, config: crate::crypto::EncryptionConfig) {
        self.encryption = Some(config);
    }

    /// Set XMP metadata on the document catalog.
    ///
    /// Generates an XMP XML metadata stream with the given fields and attaches
    /// it to the Catalog as `/Metadata`.
    pub fn set_xmp_metadata(&mut self, title: &str, author: &str, subject: &str, creator: &str) {
        let xmp = format!(
            "<?xpacket begin=\"\u{FEFF}\" id=\"W5M0MpCehiHzreSzNTczkc9d\"?>\n\
<x:xmpmeta xmlns:x=\"adobe:ns:meta/\">\n\
<rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\n\
<rdf:Description rdf:about=\"\"\n\
  xmlns:dc=\"http://purl.org/dc/elements/1.1/\"\n\
  xmlns:xmp=\"http://ns.adobe.com/xap/1.0/\"\n\
  xmlns:pdf=\"http://ns.adobe.com/pdf/1.3/\">\n\
<dc:title><rdf:Alt><rdf:li xml:lang=\"x-default\">{title}</rdf:li></rdf:Alt></dc:title>\n\
<dc:creator><rdf:Seq><rdf:li>{author}</rdf:li></rdf:Seq></dc:creator>\n\
<dc:subject><rdf:Bag><rdf:li>{subject}</rdf:li></rdf:Bag></dc:subject>\n\
<xmp:CreatorTool>{creator}</xmp:CreatorTool>\n\
<pdf:Producer>justpdf</pdf:Producer>\n\
</rdf:Description>\n\
</rdf:RDF>\n\
</x:xmpmeta>\n\
<?xpacket end=\"w\"?>"
        );

        let mut meta_dict = PdfDict::new();
        meta_dict.insert(b"Type".to_vec(), PdfObject::Name(b"Metadata".to_vec()));
        meta_dict.insert(b"Subtype".to_vec(), PdfObject::Name(b"XML".to_vec()));
        meta_dict.insert(
            b"Length".to_vec(),
            PdfObject::Integer(xmp.len() as i64),
        );

        // Store as uncompressed stream so XMP can be found by text search
        let meta_ref = self.writer.add_object(PdfObject::Stream {
            dict: meta_dict,
            data: xmp.into_bytes(),
        });
        self.xmp_ref = Some(meta_ref);
    }

    /// Build the document and return the PDF bytes.
    pub fn build(mut self) -> Result<Vec<u8>> {
        // Create Pages dictionary
        let kids: Vec<PdfObject> = self
            .pages
            .iter()
            .map(|r| PdfObject::Reference(r.clone()))
            .collect();
        let page_count = kids.len() as i64;

        let mut pages_dict = PdfDict::new();
        pages_dict.insert(b"Type".to_vec(), PdfObject::Name(b"Pages".to_vec()));
        pages_dict.insert(b"Kids".to_vec(), PdfObject::Array(kids));
        pages_dict.insert(b"Count".to_vec(), PdfObject::Integer(page_count));

        self.writer
            .set_object(self.pages_obj_num, PdfObject::Dict(pages_dict));

        // Create Catalog dictionary
        let mut catalog_dict = PdfDict::new();
        catalog_dict.insert(b"Type".to_vec(), PdfObject::Name(b"Catalog".to_vec()));
        catalog_dict.insert(
            b"Pages".to_vec(),
            PdfObject::Reference(IndirectRef {
                obj_num: self.pages_obj_num,
                gen_num: 0,
            }),
        );
        if let Some(ref xmp_ref) = self.xmp_ref {
            catalog_dict.insert(
                b"Metadata".to_vec(),
                PdfObject::Reference(xmp_ref.clone()),
            );
        }
        let catalog_ref = self.writer.add_object(PdfObject::Dict(catalog_dict));

        // Create Info dictionary if non-empty
        let info_ref = if !self.info.is_empty() {
            Some(self.writer.add_object(PdfObject::Dict(self.info)))
        } else {
            None
        };

        // Handle encryption
        if let Some(config) = self.encryption {
            let file_id = crate::crypto::generate_file_id(b"justpdf", 0);
            let (state, encrypt_dict, id_array) = config.build(&file_id)?;

            let encrypt_ref = self.writer.add_object(PdfObject::Dict(encrypt_dict));

            // Update the state with the encrypt obj num so it won't be encrypted
            let mut state = state;
            state.encrypt_obj_num = Some(encrypt_ref.obj_num);

            crate::writer::serialize_pdf_encrypted(
                &self.writer.objects,
                self.writer.version,
                &catalog_ref,
                info_ref.as_ref(),
                &encrypt_ref,
                &state,
                &id_array,
            )
        } else {
            serialize_pdf(
                &self.writer.objects,
                self.writer.version,
                &catalog_ref,
                info_ref.as_ref(),
            )
        }
    }

    /// Build the document and save to a file.
    pub fn save(self, path: &Path) -> Result<()> {
        let bytes = self.build()?;
        std::fs::write(path, bytes)?;
        Ok(())
    }
}

impl Default for DocumentBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Embed a JPEG image into the document.
///
/// Parses the JPEG header to determine width, height, and number of color
/// components, then creates an Image XObject stream.
///
/// Returns `(resource_name, indirect_ref)` for the image.
pub fn embed_jpeg(doc: &mut DocumentBuilder, jpeg_data: &[u8]) -> Result<(String, IndirectRef)> {
    let (width, height, components) = parse_jpeg_header(jpeg_data)?;

    let color_space = match components {
        1 => b"DeviceGray".to_vec(),
        3 => b"DeviceRGB".to_vec(),
        4 => b"DeviceCMYK".to_vec(),
        _ => b"DeviceRGB".to_vec(),
    };

    let mut dict = PdfDict::new();
    dict.insert(b"Type".to_vec(), PdfObject::Name(b"XObject".to_vec()));
    dict.insert(b"Subtype".to_vec(), PdfObject::Name(b"Image".to_vec()));
    dict.insert(b"Width".to_vec(), PdfObject::Integer(width as i64));
    dict.insert(b"Height".to_vec(), PdfObject::Integer(height as i64));
    dict.insert(
        b"ColorSpace".to_vec(),
        PdfObject::Name(color_space),
    );
    dict.insert(b"BitsPerComponent".to_vec(), PdfObject::Integer(8));
    dict.insert(
        b"Filter".to_vec(),
        PdfObject::Name(b"DCTDecode".to_vec()),
    );
    dict.insert(
        b"Length".to_vec(),
        PdfObject::Integer(jpeg_data.len() as i64),
    );

    let image_obj = PdfObject::Stream {
        dict,
        data: jpeg_data.to_vec(),
    };
    let image_ref = doc.writer.add_object(image_obj);

    // Assign an image resource name
    let res_name = format!("Im{}", image_ref.obj_num);

    Ok((res_name, image_ref))
}

/// Parse a JPEG header to extract width, height, and number of components.
///
/// Looks for a SOF0 (0xFF 0xC0) or SOF2 (0xFF 0xC2) marker and reads the
/// frame header fields.
fn parse_jpeg_header(data: &[u8]) -> Result<(u32, u32, u8)> {
    if data.len() < 2 || data[0] != 0xFF || data[1] != 0xD8 {
        return Err(JustPdfError::StreamDecode {
            filter: "DCTDecode".into(),
            detail: "not a valid JPEG (missing SOI marker)".into(),
        });
    }

    let mut pos = 2;
    while pos + 1 < data.len() {
        if data[pos] != 0xFF {
            pos += 1;
            continue;
        }

        let marker = data[pos + 1];
        pos += 2;

        // SOF0 or SOF2 (baseline or progressive)
        if marker == 0xC0 || marker == 0xC2 {
            if pos + 7 > data.len() {
                break;
            }
            // Skip frame length (2 bytes) and precision (1 byte)
            let height = ((data[pos + 3] as u32) << 8) | (data[pos + 4] as u32);
            let width = ((data[pos + 5] as u32) << 8) | (data[pos + 6] as u32);
            let components = data[pos + 7];
            return Ok((width, height, components));
        }

        // Skip segment: read segment length
        if pos + 1 >= data.len() {
            break;
        }
        // Markers without payload
        if marker == 0xD8 || marker == 0xD9 || (0xD0..=0xD7).contains(&marker) {
            continue;
        }
        let seg_len = ((data[pos] as usize) << 8) | (data[pos + 1] as usize);
        if seg_len < 2 {
            break;
        }
        pos += seg_len;
    }

    Err(JustPdfError::StreamDecode {
        filter: "DCTDecode".into(),
        detail: "could not find SOF marker in JPEG data".into(),
    })
}

/// Generate a ToUnicode CMap string mapping byte codes to Unicode values.
fn generate_tounicode_cmap(entries: &[(u8, u16)]) -> String {
    let mut cmap = String::new();
    cmap.push_str("/CIDInit /ProcSet findresource begin\n");
    cmap.push_str("12 dict begin\n");
    cmap.push_str("begincmap\n");
    cmap.push_str("/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n");
    cmap.push_str("/CMapName /Adobe-Identity-UCS def\n");
    cmap.push_str("/CMapType 2 def\n");
    cmap.push_str("1 begincodespacerange\n");
    cmap.push_str("<00> <FF>\n");
    cmap.push_str("endcodespacerange\n");

    // Write bfchar entries in chunks of 100 (PDF spec limit)
    let mut i = 0;
    while i < entries.len() {
        let chunk_size = (entries.len() - i).min(100);
        cmap.push_str(&format!("{} beginbfchar\n", chunk_size));
        for &(code, unicode) in &entries[i..i + chunk_size] {
            cmap.push_str(&format!("<{:02X}> <{:04X}>\n", code, unicode));
        }
        cmap.push_str("endbfchar\n");
        i += chunk_size;
    }

    cmap.push_str("endcmap\n");
    cmap.push_str("CMapName currentdict /CMap defineresource pop\n");
    cmap.push_str("end\n");
    cmap.push_str("end\n");
    cmap
}

/// Embed a PNG image into the document.
///
/// Decodes the PNG, extracts RGB data (with optional alpha as SMask),
/// and creates an Image XObject.
///
/// Returns `(resource_name, indirect_ref)` for the image.
pub fn embed_png(doc: &mut DocumentBuilder, png_data: &[u8]) -> Result<(String, IndirectRef)> {
    use crate::writer::encode::encode_flate;

    let decoder = png::Decoder::new(png_data);
    let mut reader = decoder.read_info().map_err(|e| JustPdfError::StreamDecode {
        filter: "PNG".into(),
        detail: format!("failed to decode PNG: {}", e),
    })?;

    let info = reader.info().clone();
    let width = info.width;
    let height = info.height;
    let color_type = info.color_type;

    // Read all pixel data
    let mut img_data = vec![0u8; reader.output_buffer_size()];
    let output_info = reader.next_frame(&mut img_data).map_err(|e| JustPdfError::StreamDecode {
        filter: "PNG".into(),
        detail: format!("failed to read PNG frame: {}", e),
    })?;
    img_data.truncate(output_info.buffer_size());

    let (rgb_data, alpha_data) = match color_type {
        png::ColorType::Rgb => (img_data, None),
        png::ColorType::Rgba => {
            // Split into RGB and Alpha channels
            let pixel_count = (width * height) as usize;
            let mut rgb = Vec::with_capacity(pixel_count * 3);
            let mut alpha = Vec::with_capacity(pixel_count);
            for chunk in img_data.chunks(4) {
                if chunk.len() == 4 {
                    rgb.extend_from_slice(&chunk[..3]);
                    alpha.push(chunk[3]);
                }
            }
            (rgb, Some(alpha))
        }
        png::ColorType::Grayscale => {
            // Convert grayscale to RGB
            let mut rgb = Vec::with_capacity(img_data.len() * 3);
            for &g in &img_data {
                rgb.push(g);
                rgb.push(g);
                rgb.push(g);
            }
            (rgb, None)
        }
        png::ColorType::GrayscaleAlpha => {
            let pixel_count = (width * height) as usize;
            let mut rgb = Vec::with_capacity(pixel_count * 3);
            let mut alpha = Vec::with_capacity(pixel_count);
            for chunk in img_data.chunks(2) {
                if chunk.len() == 2 {
                    rgb.push(chunk[0]);
                    rgb.push(chunk[0]);
                    rgb.push(chunk[0]);
                    alpha.push(chunk[1]);
                }
            }
            (rgb, Some(alpha))
        }
        _ => {
            return Err(JustPdfError::StreamDecode {
                filter: "PNG".into(),
                detail: format!("unsupported PNG color type: {:?}", color_type),
            });
        }
    };

    // Compress RGB data
    let compressed_rgb = encode_flate(&rgb_data)?;

    // Create SMask if alpha channel exists
    let smask_ref = if let Some(alpha) = alpha_data {
        let compressed_alpha = encode_flate(&alpha)?;
        let mut smask_dict = PdfDict::new();
        smask_dict.insert(b"Type".to_vec(), PdfObject::Name(b"XObject".to_vec()));
        smask_dict.insert(b"Subtype".to_vec(), PdfObject::Name(b"Image".to_vec()));
        smask_dict.insert(b"Width".to_vec(), PdfObject::Integer(width as i64));
        smask_dict.insert(b"Height".to_vec(), PdfObject::Integer(height as i64));
        smask_dict.insert(
            b"ColorSpace".to_vec(),
            PdfObject::Name(b"DeviceGray".to_vec()),
        );
        smask_dict.insert(b"BitsPerComponent".to_vec(), PdfObject::Integer(8));
        smask_dict.insert(
            b"Filter".to_vec(),
            PdfObject::Name(b"FlateDecode".to_vec()),
        );

        let r = doc.writer.add_object(PdfObject::Stream {
            dict: smask_dict,
            data: compressed_alpha,
        });
        Some(r)
    } else {
        None
    };

    // Create Image XObject
    let mut img_dict = PdfDict::new();
    img_dict.insert(b"Type".to_vec(), PdfObject::Name(b"XObject".to_vec()));
    img_dict.insert(b"Subtype".to_vec(), PdfObject::Name(b"Image".to_vec()));
    img_dict.insert(b"Width".to_vec(), PdfObject::Integer(width as i64));
    img_dict.insert(b"Height".to_vec(), PdfObject::Integer(height as i64));
    img_dict.insert(
        b"ColorSpace".to_vec(),
        PdfObject::Name(b"DeviceRGB".to_vec()),
    );
    img_dict.insert(b"BitsPerComponent".to_vec(), PdfObject::Integer(8));
    img_dict.insert(
        b"Filter".to_vec(),
        PdfObject::Name(b"FlateDecode".to_vec()),
    );
    if let Some(ref smask) = smask_ref {
        img_dict.insert(b"SMask".to_vec(), PdfObject::Reference(smask.clone()));
    }

    let image_ref = doc.writer.add_object(PdfObject::Stream {
        dict: img_dict,
        data: compressed_rgb,
    });

    let res_name = format!("Im{}", image_ref.obj_num);
    Ok((res_name, image_ref))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::PdfDocument;

    #[test]
    fn test_create_and_parse_pdf() {
        let mut doc = DocumentBuilder::new();
        let font_name = doc.add_standard_font("Helvetica");

        let mut page = PageBuilder::new(612.0, 792.0);
        page.add_font(&font_name, "Helvetica");
        page.begin_text();
        page.set_font(&font_name, 24.0);
        page.move_to(72.0, 720.0);
        page.show_text("Hello, World!");
        page.end_text();

        doc.add_page(page);
        doc.set_title("Test PDF");

        let bytes = doc.build().unwrap();

        // Verify it starts with PDF header
        assert!(bytes.starts_with(b"%PDF-1.7"));

        // Parse it back
        let mut parsed = PdfDocument::from_bytes(bytes).unwrap();
        let pages = crate::page::collect_pages(&parsed).unwrap();
        assert_eq!(pages.len(), 1);
    }

    #[test]
    fn test_document_with_info() {
        let mut doc = DocumentBuilder::new();
        doc.set_title("My Title");
        doc.set_author("Test Author");
        doc.set_producer("justpdf");

        // Add a minimal page
        let page = PageBuilder::new(612.0, 792.0);
        doc.add_page(page);

        let bytes = doc.build().unwrap();
        let text = String::from_utf8_lossy(&bytes);

        assert!(text.contains("My Title"));
        assert!(text.contains("Test Author"));
        assert!(text.contains("justpdf"));
    }

    #[test]
    fn test_document_multiple_pages() {
        let mut doc = DocumentBuilder::new();

        let page1 = PageBuilder::new(612.0, 792.0);
        doc.add_page(page1);

        let page2 = PageBuilder::new(612.0, 792.0);
        doc.add_page(page2);

        let bytes = doc.build().unwrap();

        let mut parsed = PdfDocument::from_bytes(bytes).unwrap();
        let pages = crate::page::collect_pages(&parsed).unwrap();
        assert_eq!(pages.len(), 2);
    }

    #[test]
    fn test_add_standard_font_idempotent() {
        let mut doc = DocumentBuilder::new();
        let name1 = doc.add_standard_font("Helvetica");
        let name2 = doc.add_standard_font("Helvetica");
        assert_eq!(name1, name2);

        let name3 = doc.add_standard_font("Courier");
        assert_ne!(name1, name3);
    }

    #[test]
    fn test_embed_truetype_invalid_data() {
        let mut doc = DocumentBuilder::new();
        let result = doc.embed_truetype_font(b"not a font");
        assert!(result.is_err());
    }

    #[test]
    fn test_embed_png_minimal() {
        // Create a minimal 2x2 RGB PNG in memory using the png encoder
        let mut png_bytes = Vec::new();
        {
            let mut encoder = png::Encoder::new(std::io::Cursor::new(&mut png_bytes), 2, 2);
            encoder.set_color(png::ColorType::Rgb);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().unwrap();
            // 2x2 RGB = 12 bytes (R,G,B per pixel)
            let data: [u8; 12] = [
                255, 0, 0, // red
                0, 255, 0, // green
                0, 0, 255, // blue
                255, 255, 0, // yellow
            ];
            writer.write_image_data(&data).unwrap();
        }

        let mut doc = DocumentBuilder::new();
        let (name, img_ref) = embed_png(&mut doc, &png_bytes).unwrap();
        assert!(name.starts_with("Im"));
        assert!(img_ref.obj_num > 0);

        // Build a doc with the image
        let mut page = PageBuilder::new(612.0, 792.0);
        page.add_image(&name, img_ref);
        page.draw_image(&name, 0.0, 0.0, 100.0, 100.0);
        doc.add_page(page);
        let bytes = doc.build().unwrap();
        assert!(bytes.starts_with(b"%PDF-1.7"));
    }

    #[test]
    fn test_embed_png_with_alpha() {
        // Create a 2x2 RGBA PNG
        let mut png_bytes = Vec::new();
        {
            let mut encoder = png::Encoder::new(std::io::Cursor::new(&mut png_bytes), 2, 2);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().unwrap();
            let data: [u8; 16] = [
                255, 0, 0, 128, // red, semi-transparent
                0, 255, 0, 255, // green, opaque
                0, 0, 255, 0,   // blue, fully transparent
                255, 255, 0, 64, // yellow, mostly transparent
            ];
            writer.write_image_data(&data).unwrap();
        }

        let mut doc = DocumentBuilder::new();
        let (name, img_ref) = embed_png(&mut doc, &png_bytes).unwrap();
        assert!(name.starts_with("Im"));

        // The writer should have at least 2 objects: SMask + Image
        // (SMask for alpha channel)
        assert!(doc.writer.objects.len() >= 2);

        let mut page = PageBuilder::new(612.0, 792.0);
        page.add_image(&name, img_ref);
        page.draw_image(&name, 0.0, 0.0, 100.0, 100.0);
        doc.add_page(page);
        let bytes = doc.build().unwrap();
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.contains("SMask"));
    }

    #[test]
    fn test_xmp_metadata() {
        let mut doc = DocumentBuilder::new();
        doc.set_xmp_metadata("Test Title", "Test Author", "Test Subject", "TestCreator");

        let page = PageBuilder::new(612.0, 792.0);
        doc.add_page(page);

        let bytes = doc.build().unwrap();
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.contains("Test Title"));
        assert!(text.contains("Test Author"));
        assert!(text.contains("Test Subject"));
        assert!(text.contains("TestCreator"));
        assert!(text.contains("xmpmeta"));
        assert!(text.contains("/Metadata"));
    }

    #[test]
    fn test_tounicode_cmap_generation() {
        let entries = vec![(0x41u8, 0x0041u16), (0x42, 0x0042)];
        let cmap = generate_tounicode_cmap(&entries);
        assert!(cmap.contains("beginbfchar"));
        assert!(cmap.contains("<41> <0041>"));
        assert!(cmap.contains("<42> <0042>"));
        assert!(cmap.contains("endbfchar"));
    }

    #[test]
    fn test_parse_jpeg_header() {
        // Minimal JPEG with SOI, SOF0 marker
        // SOI: FF D8
        // APP0: FF E0 00 02
        // SOF0: FF C0 00 0B 08 00 64 00 C8 03 ...
        //   precision=8, height=100, width=200, components=3
        let jpeg_fixed = vec![
            0xFF, 0xD8, // SOI
            0xFF, 0xE0, 0x00, 0x02, // APP0 segment, length=2 (just the length bytes)
            0xFF, 0xC0, // SOF0 marker
            0x00, 0x0B, // frame header length = 11
            0x08, // precision = 8 bits
            0x00, 0x64, // height = 100
            0x00, 0xC8, // width = 200
            0x03, // components = 3
            // component specs would follow but we don't need them
        ];

        // After SOI: pos=2
        // FF E0: marker, pos=4. seg_len=2, pos=4+2=6
        // FF C0: pos=8 (after consuming marker bytes at [6],[7])
        // data[8+3]=data[11]=0x00, data[8+4]=data[12]=0x64 => height=100
        // data[8+5]=data[13]=0x00, data[8+6]=data[14]=0xC8 => width=200
        // data[8+7]=data[15]=0x03 => components=3

        let (w, h, c) = parse_jpeg_header(&jpeg_fixed).unwrap();
        assert_eq!(w, 200);
        assert_eq!(h, 100);
        assert_eq!(c, 3);
    }

    // --- Negative Tests ---

    #[test]
    fn test_negative_page_size() {
        // Negative or zero page size should still produce a valid PDF
        // (the writer doesn't validate dimensions — that's the renderer's job)
        // But we verify it doesn't panic
        let mut doc = DocumentBuilder::new();
        let page = PageBuilder::new(-100.0, 0.0);
        doc.add_page(page);
        let result = doc.build();
        assert!(result.is_ok());
    }

    #[test]
    fn test_empty_font_name() {
        // Empty font name should still work (no panic)
        let mut doc = DocumentBuilder::new();
        let name = doc.add_standard_font("");
        assert!(!name.is_empty()); // returns "F1" regardless
    }

    #[test]
    fn test_embed_truetype_invalid_data_returns_error() {
        let mut doc = DocumentBuilder::new();
        let result = doc.embed_truetype_font(b"not a font file");
        assert!(result.is_err());
    }

    #[test]
    fn test_embed_jpeg_invalid_data_returns_error() {
        let mut doc = DocumentBuilder::new();
        let result = embed_jpeg(&mut doc, b"not a jpeg");
        assert!(result.is_err());
    }

    #[test]
    fn test_embed_png_invalid_data_returns_error() {
        let mut doc = DocumentBuilder::new();
        let result = embed_png(&mut doc, b"not a png");
        assert!(result.is_err());
    }

    #[test]
    fn test_delete_page_out_of_range() {
        use crate::writer::modify::DocumentModifier;

        let mut doc = DocumentBuilder::new();
        let page = PageBuilder::new(612.0, 792.0);
        doc.add_page(page);
        let bytes = doc.build().unwrap();

        let mut parsed = PdfDocument::from_bytes(bytes).unwrap();
        let mut modifier = DocumentModifier::from_document(&mut parsed).unwrap();

        // Deleting page 999 on a 1-page doc should not panic
        let result = modifier.delete_page(999);
        assert!(result.is_ok());

        // Original page should still be there
        let new_bytes = modifier.build().unwrap();
        let mut reparsed = PdfDocument::from_bytes(new_bytes).unwrap();
        let pages = crate::page::collect_pages(&reparsed).unwrap();
        assert_eq!(pages.len(), 1);
    }

    #[test]
    fn test_save_io_error() {
        // Writing to an invalid path should return an I/O error
        let mut doc = DocumentBuilder::new();
        let page = PageBuilder::new(612.0, 792.0);
        doc.add_page(page);

        let result = doc.save(std::path::Path::new("/nonexistent/path/to/file.pdf"));
        assert!(result.is_err());
    }
}
