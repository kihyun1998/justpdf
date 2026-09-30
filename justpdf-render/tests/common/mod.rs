//! Synthetic test PDFs and pixel reads shared by the render tests.

#![allow(dead_code)]

use justpdf_core::PdfDocument;
use justpdf_render::{RenderOptions, render_page_to_pixmap};

/// One object body of a test PDF: a plain object or a stream.
pub enum Obj<'a> {
    Plain(&'a str),
    Stream(&'a str, &'a str),
}
pub use Obj::{Plain, Stream};

/// Builds a PDF whose object `n` (1-based) is `objects[n - 1]`, with a
/// correct xref table and `/Root 1 0 R`.
pub fn pdf(objects: &[Obj]) -> PdfDocument {
    let mut out = b"%PDF-1.7\n".to_vec();
    let mut offsets = Vec::new();
    for (i, obj) in objects.iter().enumerate() {
        offsets.push(out.len());
        let body = match obj {
            Plain(body) => body.to_string(),
            Stream(dict, content) => format!(
                "<< {dict} /Length {} >>\nstream\n{content}\nendstream",
                content.len()
            ),
        };
        out.extend_from_slice(format!("{} 0 obj\n{body}\nendobj\n", i + 1).as_bytes());
    }
    let xref_at = out.len();
    out.extend_from_slice(
        format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes(),
    );
    for off in offsets {
        out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref_at}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );
    PdfDocument::from_bytes(out).unwrap()
}

/// A 100×100 page: catalog 1, pages 2, page 3 with `resources`, content 4.
/// Further objects start at 5.
pub fn page(resources: &str, content: &str, rest: Vec<Obj>) -> PdfDocument {
    let page_dict = format!(
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] /Resources {resources} /Contents 4 0 R >>"
    );
    let mut objects = vec![
        Plain("<< /Type /Catalog /Pages 2 0 R >>"),
        Plain("<< /Type /Pages /Kids [3 0 R] /Count 1 >>"),
        Plain(&page_dict),
        Stream("", content),
    ];
    objects.extend(rest);
    pdf(&objects)
}

/// RGB of device pixel (x, y) at 72 dpi; y counts down from the top.
pub fn rgb(doc: &PdfDocument, x: u32, y: u32) -> [u8; 3] {
    let px = render_page_to_pixmap(doc, 0, &RenderOptions::default()).unwrap();
    let i = ((y * px.width + x) * 4) as usize;
    [px.data[i], px.data[i + 1], px.data[i + 2]]
}

pub const RED: [u8; 3] = [255, 0, 0];
pub const WHITE: [u8; 3] = [255, 255, 255];
