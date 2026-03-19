use crate::object::{IndirectRef, PdfDict, PdfObject};
use crate::writer::encode::make_stream;
use crate::writer::PdfWriter;

/// Builder for constructing a single PDF page with content streams.
pub struct PageBuilder {
    width: f64,
    height: f64,
    content: Vec<u8>,
    /// Registered fonts: (resource_name like "F1", font_name like "Helvetica").
    font_names: Vec<(String, String)>,
    /// Registered images: (resource_name like "Im1", image_ref).
    image_names: Vec<(String, IndirectRef)>,
    /// Registered font references: (resource_name like "F1", font_ref).
    /// Used for embedded fonts (TrueType etc.) that are indirect objects.
    font_refs: Vec<(String, IndirectRef)>,
}

impl PageBuilder {
    /// Create a new page builder with the given dimensions.
    /// Default US Letter size is 612 x 792 points.
    pub fn new(width: f64, height: f64) -> Self {
        Self {
            width,
            height,
            content: Vec::new(),
            font_names: Vec::new(),
            image_names: Vec::new(),
            font_refs: Vec::new(),
        }
    }

    /// Set the current font and size. Emits `BT /{name} {size} Tf`.
    pub fn set_font(&mut self, resource_name: &str, size: f64) {
        use std::io::Write;
        write!(self.content, "/{} {} Tf\n", resource_name, size).unwrap();
    }

    /// Begin a text object: `BT`.
    pub fn begin_text(&mut self) {
        self.content.extend_from_slice(b"BT\n");
    }

    /// End a text object: `ET`.
    pub fn end_text(&mut self) {
        self.content.extend_from_slice(b"ET\n");
    }

    /// Move to position (x, y): `x y Td`.
    pub fn move_to(&mut self, x: f64, y: f64) {
        use std::io::Write;
        write!(self.content, "{} {} Td\n", x, y).unwrap();
    }

    /// Show text string with PDF string escaping: `(text) Tj`.
    pub fn show_text(&mut self, text: &str) {
        self.content.push(b'(');
        for &b in text.as_bytes() {
            match b {
                b'\\' => self.content.extend_from_slice(b"\\\\"),
                b'(' => self.content.extend_from_slice(b"\\("),
                b')' => self.content.extend_from_slice(b"\\)"),
                _ => self.content.push(b),
            }
        }
        self.content.extend_from_slice(b") Tj\n");
    }

    /// Set fill color in RGB: `r g b rg`.
    pub fn set_fill_rgb(&mut self, r: f64, g: f64, b: f64) {
        use std::io::Write;
        write!(self.content, "{} {} {} rg\n", r, g, b).unwrap();
    }

    /// Set stroke color in RGB: `r g b RG`.
    pub fn set_stroke_rgb(&mut self, r: f64, g: f64, b: f64) {
        use std::io::Write;
        write!(self.content, "{} {} {} RG\n", r, g, b).unwrap();
    }

    /// Draw a line from (x1,y1) to (x2,y2) and stroke: `x1 y1 m x2 y2 l S`.
    pub fn draw_line(&mut self, x1: f64, y1: f64, x2: f64, y2: f64) {
        use std::io::Write;
        write!(self.content, "{} {} m {} {} l S\n", x1, y1, x2, y2).unwrap();
    }

    /// Draw a stroked rectangle: `x y w h re S`.
    pub fn draw_rect(&mut self, x: f64, y: f64, w: f64, h: f64) {
        use std::io::Write;
        write!(self.content, "{} {} {} {} re S\n", x, y, w, h).unwrap();
    }

    /// Draw a filled rectangle: `x y w h re f`.
    pub fn fill_rect(&mut self, x: f64, y: f64, w: f64, h: f64) {
        use std::io::Write;
        write!(self.content, "{} {} {} {} re f\n", x, y, w, h).unwrap();
    }

    /// Draw an image with transformation: `q w 0 0 h x y cm /name Do Q`.
    pub fn draw_image(&mut self, name: &str, x: f64, y: f64, w: f64, h: f64) {
        use std::io::Write;
        write!(
            self.content,
            "q {} 0 0 {} {} {} cm /{} Do Q\n",
            w, h, x, y, name
        )
        .unwrap();
    }

    /// Draw an inline image directly in the content stream.
    ///
    /// Writes `BI /W {width} /H {height} /BPC {bpc} /CS /{cs} ID {data} EI`.
    pub fn draw_inline_image(
        &mut self,
        width: u32,
        height: u32,
        bpc: u8,
        color_space: &str,
        data: &[u8],
    ) {
        use std::io::Write;
        write!(
            self.content,
            "BI /W {} /H {} /BPC {} /CS /{} ID ",
            width, height, bpc, color_space
        )
        .unwrap();
        self.content.extend_from_slice(data);
        self.content.extend_from_slice(b" EI\n");
    }

    /// Register a font resource for this page.
    pub fn add_font(&mut self, resource_name: &str, font_name: &str) {
        self.font_names
            .push((resource_name.to_string(), font_name.to_string()));
    }

    /// Register an embedded font resource by indirect reference.
    /// Used for TrueType and other embedded fonts.
    pub fn add_font_ref(&mut self, resource_name: &str, font_ref: IndirectRef) {
        self.font_refs
            .push((resource_name.to_string(), font_ref));
    }

    /// Register an image resource for this page.
    pub fn add_image(&mut self, resource_name: &str, image_ref: IndirectRef) {
        self.image_names
            .push((resource_name.to_string(), image_ref));
    }

    /// Build the page object and add it (and its content stream) to the writer.
    ///
    /// Returns the indirect reference to the Page dictionary.
    pub fn build(self, writer: &mut PdfWriter, pages_ref: &IndirectRef) -> IndirectRef {
        // Create content stream
        let (stream_dict, stream_data) = make_stream(&self.content, true);
        let content_stream = PdfObject::Stream {
            dict: stream_dict,
            data: stream_data,
        };
        let content_ref = writer.add_object(content_stream);

        // Build Resources dictionary
        let mut resources = PdfDict::new();

        // Font resources
        if !self.font_names.is_empty() || !self.font_refs.is_empty() {
            let mut font_dict = PdfDict::new();
            for (res_name, _font_name) in &self.font_names {
                // For standard fonts, we create inline font dicts.
                let mut f = PdfDict::new();
                f.insert(b"Type".to_vec(), PdfObject::Name(b"Font".to_vec()));
                f.insert(b"Subtype".to_vec(), PdfObject::Name(b"Type1".to_vec()));
                let base_font = self
                    .font_names
                    .iter()
                    .find(|(n, _)| n == res_name)
                    .map(|(_, bf)| bf.clone())
                    .unwrap_or_default();
                f.insert(
                    b"BaseFont".to_vec(),
                    PdfObject::Name(base_font.into_bytes()),
                );
                font_dict.insert(
                    res_name.as_bytes().to_vec(),
                    PdfObject::Dict(f),
                );
            }
            // Add embedded font references
            for (res_name, font_ref) in &self.font_refs {
                font_dict.insert(
                    res_name.as_bytes().to_vec(),
                    PdfObject::Reference(font_ref.clone()),
                );
            }
            resources.insert(b"Font".to_vec(), PdfObject::Dict(font_dict));
        }

        // Image / XObject resources
        if !self.image_names.is_empty() {
            let mut xobject_dict = PdfDict::new();
            for (res_name, img_ref) in &self.image_names {
                xobject_dict.insert(
                    res_name.as_bytes().to_vec(),
                    PdfObject::Reference(img_ref.clone()),
                );
            }
            resources.insert(b"XObject".to_vec(), PdfObject::Dict(xobject_dict));
        }

        // Build Page dictionary
        let mut page_dict = PdfDict::new();
        page_dict.insert(b"Type".to_vec(), PdfObject::Name(b"Page".to_vec()));
        page_dict.insert(
            b"Parent".to_vec(),
            PdfObject::Reference(pages_ref.clone()),
        );
        page_dict.insert(
            b"MediaBox".to_vec(),
            PdfObject::Array(vec![
                PdfObject::Real(0.0),
                PdfObject::Real(0.0),
                PdfObject::Real(self.width),
                PdfObject::Real(self.height),
            ]),
        );
        page_dict.insert(
            b"Contents".to_vec(),
            PdfObject::Reference(content_ref),
        );
        page_dict.insert(b"Resources".to_vec(), PdfObject::Dict(resources));

        writer.add_object(PdfObject::Dict(page_dict))
    }
}

impl Default for PageBuilder {
    fn default() -> Self {
        Self::new(612.0, 792.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_page_builder_content() {
        let mut page = PageBuilder::new(612.0, 792.0);
        page.begin_text();
        page.set_font("F1", 12.0);
        page.move_to(72.0, 720.0);
        page.show_text("Hello");
        page.end_text();

        let content = String::from_utf8(page.content.clone()).unwrap();
        assert!(content.contains("BT\n"));
        assert!(content.contains("/F1 12 Tf\n"));
        assert!(content.contains("72 720 Td\n"));
        assert!(content.contains("(Hello) Tj\n"));
        assert!(content.contains("ET\n"));
    }

    #[test]
    fn test_page_builder_text_escaping() {
        let mut page = PageBuilder::new(612.0, 792.0);
        page.show_text("Hello (world) \\ end");

        let content = String::from_utf8(page.content.clone()).unwrap();
        assert!(content.contains("(Hello \\(world\\) \\\\ end) Tj"));
    }

    #[test]
    fn test_page_builder_graphics() {
        let mut page = PageBuilder::new(612.0, 792.0);
        page.set_fill_rgb(1.0, 0.0, 0.0);
        page.fill_rect(10.0, 10.0, 100.0, 50.0);
        page.set_stroke_rgb(0.0, 0.0, 1.0);
        page.draw_rect(10.0, 10.0, 100.0, 50.0);
        page.draw_line(0.0, 0.0, 100.0, 100.0);

        let content = String::from_utf8(page.content.clone()).unwrap();
        assert!(content.contains("1 0 0 rg\n"));
        assert!(content.contains("10 10 100 50 re f\n"));
        assert!(content.contains("0 0 1 RG\n"));
        assert!(content.contains("10 10 100 50 re S\n"));
        assert!(content.contains("0 0 m 100 100 l S\n"));
    }

    #[test]
    fn test_page_builder_image() {
        let mut page = PageBuilder::new(612.0, 792.0);
        page.draw_image("Im1", 0.0, 0.0, 200.0, 150.0);

        let content = String::from_utf8(page.content.clone()).unwrap();
        assert!(content.contains("q 200 0 0 150 0 0 cm /Im1 Do Q\n"));
    }

    #[test]
    fn test_page_builder_build() {
        let mut writer = PdfWriter::new();
        let pages_ref = IndirectRef {
            obj_num: 99,
            gen_num: 0,
        };

        let mut page = PageBuilder::new(612.0, 792.0);
        page.add_font("F1", "Helvetica");
        page.begin_text();
        page.set_font("F1", 12.0);
        page.move_to(72.0, 720.0);
        page.show_text("Test");
        page.end_text();

        let page_ref = page.build(&mut writer, &pages_ref);

        // Should have added 2 objects: content stream + page dict
        assert_eq!(writer.objects.len(), 2);
        assert_eq!(page_ref.gen_num, 0);
    }

    #[test]
    fn test_inline_image() {
        let mut page = PageBuilder::new(612.0, 792.0);
        let data = vec![0xFF, 0x00, 0xFF, 0x00, 0xFF, 0x00]; // 2x1 RGB
        page.draw_inline_image(2, 1, 8, "DeviceRGB", &data);

        let content = String::from_utf8_lossy(&page.content);
        assert!(content.contains("BI /W 2 /H 1 /BPC 8 /CS /DeviceRGB ID "));
        assert!(content.contains(" EI\n"));
    }

    #[test]
    fn test_page_builder_font_ref() {
        let mut writer = PdfWriter::new();
        let pages_ref = IndirectRef {
            obj_num: 99,
            gen_num: 0,
        };

        let font_ref = IndirectRef {
            obj_num: 50,
            gen_num: 0,
        };

        let mut page = PageBuilder::new(612.0, 792.0);
        page.add_font_ref("F1", font_ref);
        page.begin_text();
        page.set_font("F1", 12.0);
        page.show_text("Hello");
        page.end_text();

        let page_ref = page.build(&mut writer, &pages_ref);
        assert!(page_ref.obj_num > 0);
    }
}
