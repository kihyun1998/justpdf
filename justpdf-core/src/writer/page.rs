use std::sync::Arc;

use crate::font::{Font, FontEntry};
use crate::object::{ByteSink, IndirectRef, Number, PdfDict, PdfObject, name_syntax, write_string};
use crate::writer::PdfWriter;
use crate::writer::encode::make_stream;

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
    /// Font handles this page draws with, declared in its resources.
    fonts: Vec<Arc<FontEntry>>,
    /// The font `set_font` selected and its size; `None` after `set_font_name`.
    current_font: Option<(Font, f64)>,
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
            fonts: Vec::new(),
            current_font: None,
        }
    }

    /// Select `font` at `size` for `show_text`, `/{name} {size} Tf`, and
    /// declare it in this page's resources.
    pub fn set_font(&mut self, font: &Font, size: f64) {
        let primary = font.entries()[0].clone();
        self.write_tf(&primary, size);
        self.current_font = Some((font.clone(), size));
    }

    /// Set the current font by resource name and size: `/{name} {size} Tf`,
    /// the name escaped. `show_text` then writes WinAnsi codes.
    #[deprecated(
        note = "register a `Font` with `add_font` or `add_standard_font` and use `set_font`"
    )]
    pub fn set_font_name(&mut self, resource_name: &str, size: f64) {
        self.write_tf_name(resource_name, size);
        self.current_font = None;
    }

    fn write_tf(&mut self, entry: &Arc<FontEntry>, size: f64) {
        if !self.fonts.iter().any(|e| Arc::ptr_eq(e, entry)) {
            self.fonts.push(entry.clone());
        }
        self.write_tf_name(entry.resource_name(), size);
    }

    fn write_tf_name(&mut self, resource_name: &str, size: f64) {
        use std::io::Write;
        writeln!(
            self.content,
            "{} {} Tf",
            name_syntax(resource_name.as_bytes()),
            Number(size)
        )
        .unwrap();
    }

    /// The font handles this page draws with.
    pub(crate) fn font_entries(&self) -> &[Arc<FontEntry>] {
        &self.fonts
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
        writeln!(self.content, "{} {} Td", Number(x), Number(y)).unwrap();
    }

    /// Show a text string in the font `set_font` selected: WinAnsi codes for a
    /// standard font (`?` for a character with none), 2-byte CIDs for an
    /// embedded one. Each character goes to the first font of the chain that
    /// has a glyph for it, runs in different fonts separated by `Tf` at the
    /// same size and the selected font restored after; a character no font
    /// has is drawn as the primary font's `.notdef` (`?`). Each run is written
    /// as `(codes) Tj`, or `<hex> Tj` when the bytes are not all printable
    /// ASCII.
    ///
    /// After `set_font_name`, or with no font selected, the text is written as
    /// WinAnsi codes.
    pub fn show_text(&mut self, text: &str) {
        let Some((font, size)) = self.current_font.clone() else {
            self.write_tj(&crate::font::encode_winansi(text));
            return;
        };
        let primary = &font.entries()[0];
        let mut active = primary.clone();
        for (entry, codes) in font.encode(text) {
            if !Arc::ptr_eq(&entry, &active) {
                self.write_tf(&entry, size);
                active = entry;
            }
            self.write_tj(&codes);
        }
        if !Arc::ptr_eq(&active, primary) {
            self.write_tf(primary, size);
        }
    }

    fn write_tj(&mut self, codes: &[u8]) {
        let _ = write_string(&mut ByteSink(&mut self.content), codes);
        self.content.extend_from_slice(b" Tj\n");
    }

    /// Set fill color in RGB: `r g b rg`.
    pub fn set_fill_rgb(&mut self, r: f64, g: f64, b: f64) {
        use std::io::Write;
        writeln!(self.content, "{} {} {} rg", Number(r), Number(g), Number(b)).unwrap();
    }

    /// Set stroke color in RGB: `r g b RG`.
    pub fn set_stroke_rgb(&mut self, r: f64, g: f64, b: f64) {
        use std::io::Write;
        writeln!(self.content, "{} {} {} RG", Number(r), Number(g), Number(b)).unwrap();
    }

    /// Draw a line from (x1,y1) to (x2,y2) and stroke: `x1 y1 m x2 y2 l S`.
    pub fn draw_line(&mut self, x1: f64, y1: f64, x2: f64, y2: f64) {
        use std::io::Write;
        writeln!(
            self.content,
            "{} {} m {} {} l S",
            Number(x1),
            Number(y1),
            Number(x2),
            Number(y2)
        )
        .unwrap();
    }

    /// Draw a stroked rectangle: `x y w h re S`.
    pub fn draw_rect(&mut self, x: f64, y: f64, w: f64, h: f64) {
        use std::io::Write;
        writeln!(
            self.content,
            "{} {} {} {} re S",
            Number(x),
            Number(y),
            Number(w),
            Number(h)
        )
        .unwrap();
    }

    /// Draw a filled rectangle: `x y w h re f`.
    pub fn fill_rect(&mut self, x: f64, y: f64, w: f64, h: f64) {
        use std::io::Write;
        writeln!(
            self.content,
            "{} {} {} {} re f",
            Number(x),
            Number(y),
            Number(w),
            Number(h)
        )
        .unwrap();
    }

    /// Draw an image with transformation: `q w 0 0 h x y cm /name Do Q`.
    pub fn draw_image(&mut self, name: &str, x: f64, y: f64, w: f64, h: f64) {
        use std::io::Write;
        writeln!(
            self.content,
            "q {} 0 0 {} {} {} cm {} Do Q",
            Number(w),
            Number(h),
            Number(x),
            Number(y),
            name_syntax(name.as_bytes())
        )
        .unwrap();
    }

    /// Draw an inline image directly in the content stream.
    ///
    /// Writes `BI /W {width} /H {height} /BPC {bpc} /CS /{cs} ID {data} EI`,
    /// the color space name escaped, mapped to the unit square. For small
    /// images whose data holds no whitespace + `EI` + whitespace; a raster
    /// goes through [`embed_rgb`](crate::writer::embed_rgb) and `draw_image`.
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
            "BI /W {} /H {} /BPC {} /CS {} ID ",
            width,
            height,
            bpc,
            name_syntax(color_space.as_bytes())
        )
        .unwrap();
        self.content.extend_from_slice(data);
        self.content.extend_from_slice(b" EI\n");
    }

    /// Register a standard Type1 font resource for this page, declared
    /// `/WinAnsiEncoding` (except `Symbol` and `ZapfDingbats`).
    #[deprecated(note = "register a `Font` with `add_standard_font`; `set_font` declares it")]
    pub fn add_font(&mut self, resource_name: &str, font_name: &str) {
        self.font_names
            .push((resource_name.to_string(), font_name.to_string()));
    }

    /// Register an embedded font resource by indirect reference.
    /// Used for TrueType and other embedded fonts.
    pub fn add_font_ref(&mut self, resource_name: &str, font_ref: IndirectRef) {
        self.font_refs.push((resource_name.to_string(), font_ref));
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
        if !self.font_names.is_empty() || !self.font_refs.is_empty() || !self.fonts.is_empty() {
            let mut font_dict = PdfDict::new();
            for (res_name, base_font) in &self.font_names {
                // For standard fonts, we create inline font dicts.
                let f = crate::writer::document::standard_font_dict(base_font);
                font_dict.insert(res_name.as_bytes().to_vec(), PdfObject::Dict(f));
            }
            // Add embedded font references
            for (res_name, font_ref) in &self.font_refs {
                font_dict.insert(
                    res_name.as_bytes().to_vec(),
                    PdfObject::Reference(font_ref.clone()),
                );
            }
            for entry in &self.fonts {
                font_dict.insert(
                    entry.resource_name().as_bytes().to_vec(),
                    PdfObject::Reference(entry.font_ref().clone()),
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
        page_dict.insert(b"Parent".to_vec(), PdfObject::Reference(pages_ref.clone()));
        page_dict.insert(
            b"MediaBox".to_vec(),
            PdfObject::Array(vec![
                PdfObject::Real(0.0),
                PdfObject::Real(0.0),
                PdfObject::Real(self.width),
                PdfObject::Real(self.height),
            ]),
        );
        page_dict.insert(b"Contents".to_vec(), PdfObject::Reference(content_ref));
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
    use crate::content::{Operand, parse_content_stream};

    /// The `/Encoding` of the inline font dictionary `add_font(_, base_font)` emits.
    #[allow(deprecated)]
    fn inline_font_encoding(base_font: &str) -> Option<Vec<u8>> {
        let mut writer = PdfWriter::new();
        let pages_ref = writer.alloc_object_num();
        let mut page = PageBuilder::new(100.0, 100.0);
        page.add_font("F1", base_font);
        let page_ref = page.build(
            &mut writer,
            &IndirectRef {
                obj_num: pages_ref,
                gen_num: 0,
            },
        );
        let page_obj = writer
            .objects
            .iter()
            .find(|(n, _)| *n == page_ref.obj_num)
            .map(|(_, o)| o);
        let Some(PdfObject::Dict(page_dict)) = page_obj else {
            panic!("expected a page dictionary")
        };
        let font = page_dict
            .get_dict(b"Resources")
            .and_then(|r| r.get_dict(b"Font"))
            .and_then(|f| f.get_dict(b"F1"))
            .unwrap();
        font.get_name(b"Encoding").map(<[u8]>::to_vec)
    }

    #[test]
    fn inline_standard_fonts_are_declared_win_ansi_except_symbolic_ones() {
        assert_eq!(
            inline_font_encoding("Helvetica").as_deref(),
            Some(b"WinAnsiEncoding".as_slice())
        );
        assert_eq!(inline_font_encoding("Symbol"), None);
        assert_eq!(inline_font_encoding("ZapfDingbats"), None);
    }

    #[test]
    fn show_text_writes_win_ansi_bytes() {
        let mut page = PageBuilder::new(100.0, 100.0);
        page.begin_text();
        page.show_text("café 한");
        page.end_text();
        let ops = parse_content_stream(&page.content).unwrap();
        let tj = ops.iter().find(|op| op.operator == b"Tj").unwrap();
        assert_eq!(
            tj.operands,
            vec![Operand::String(vec![0x63, 0x61, 0x66, 0xE9, b' ', b'?'])]
        );
    }

    #[test]
    #[allow(deprecated)]
    fn test_numbers_read_back_as_integers_or_finite_reals() {
        let mut page = PageBuilder::new(612.0, 792.0);
        page.set_fill_rgb(f64::NAN, 0.0, 1.0);
        page.set_stroke_rgb(0.25, f64::INFINITY, 1.0);
        page.begin_text();
        page.set_font_name("F1", 1e20);
        page.move_to(f64::NAN, f64::NEG_INFINITY);
        page.end_text();
        page.draw_line(-0.0, 612.0, 0.5, 2147483648.0);
        page.draw_rect(f64::NAN, 1.0, 2.0, 3.0);
        page.fill_rect(1.0, f64::INFINITY, 2.0, 3.0);
        page.draw_image("Im1", f64::NAN, 0.0, 10.0, 10.0);

        let max = f64::from(f32::MAX);
        let ops = parse_content_stream(&page.content).unwrap();
        let operands = |operator: &[u8]| {
            ops.iter()
                .find(|op| op.operator == operator)
                .unwrap()
                .operands
                .clone()
        };
        use Operand::{Integer, Real};
        assert_eq!(operands(b"rg"), vec![Integer(0), Integer(0), Integer(1)]);
        assert_eq!(operands(b"RG"), vec![Real(0.25), Real(max), Integer(1)]);
        assert_eq!(
            operands(b"Tf"),
            vec![Operand::Name(b"F1".to_vec()), Real(1e20)]
        );
        assert_eq!(operands(b"Td"), vec![Integer(0), Real(-max)]);
        assert_eq!(operands(b"m"), vec![Integer(0), Integer(612)]);
        assert_eq!(operands(b"l"), vec![Real(0.5), Real(2147483648.0)]);
        let rects: Vec<_> = ops
            .iter()
            .filter(|op| op.operator == b"re")
            .map(|op| op.operands.clone())
            .collect();
        assert_eq!(
            rects,
            vec![
                vec![Integer(0), Integer(1), Integer(2), Integer(3)],
                vec![Integer(1), Real(max), Integer(2), Integer(3)],
            ]
        );
        assert_eq!(
            operands(b"cm"),
            vec![
                Integer(10),
                Integer(0),
                Integer(0),
                Integer(10),
                Integer(0),
                Integer(0)
            ]
        );
    }

    #[test]
    #[allow(deprecated)]
    fn test_written_names_and_text_read_back_unchanged() {
        let mut page = PageBuilder::new(612.0, 792.0);
        page.begin_text();
        page.set_font_name("F 1#", 12.0);
        page.show_text("1) a\\b\r\n(c");
        page.end_text();
        page.draw_image("Im/1", 0.0, 0.0, 10.0, 10.0);
        page.draw_inline_image(1, 1, 8, "Device Gray", &[0x80]);

        let ops = parse_content_stream(&page.content).unwrap();
        let operands = |operator: &[u8]| {
            ops.iter()
                .find(|op| op.operator == operator)
                .unwrap()
                .operands
                .clone()
        };
        assert_eq!(operands(b"Tf")[0], Operand::Name(b"F 1#".to_vec()));
        assert_eq!(
            operands(b"Tj"),
            vec![Operand::String(b"1) a\\b\r\n(c".to_vec())]
        );
        assert_eq!(operands(b"Do"), vec![Operand::Name(b"Im/1".to_vec())]);
        let [Operand::InlineImage { dict, data }] = &operands(b"BI")[..] else {
            panic!("expected an inline image")
        };
        assert!(dict.contains(&(b"CS".to_vec(), Operand::Name(b"Device Gray".to_vec()))));
        assert_eq!(data, &[0x80]);
    }

    #[test]
    #[allow(deprecated)]
    fn test_page_builder_content() {
        let mut page = PageBuilder::new(612.0, 792.0);
        page.begin_text();
        page.set_font_name("F1", 12.0);
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
    #[allow(deprecated)]
    fn test_page_builder_build() {
        let mut writer = PdfWriter::new();
        let pages_ref = IndirectRef {
            obj_num: 99,
            gen_num: 0,
        };

        let mut page = PageBuilder::new(612.0, 792.0);
        page.add_font("F1", "Helvetica");
        page.begin_text();
        page.set_font_name("F1", 12.0);
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
    #[allow(deprecated)]
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
        page.set_font_name("F1", 12.0);
        page.show_text("Hello");
        page.end_text();

        let page_ref = page.build(&mut writer, &pages_ref);
        assert!(page_ref.obj_num > 0);
    }
}
