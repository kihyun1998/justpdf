//! Signature appearance stream generation.

use crate::object::{PdfDict, PdfObject};
use crate::writer::encode::make_stream;

/// Generate a signature appearance Form XObject.
///
/// Returns (appearance_dict, appearance_data) suitable for use as
/// an /AP /N entry on a signature widget annotation.
pub fn generate_signature_appearance(
    signer_name: &str,
    reason: Option<&str>,
    date: Option<&str>,
    width: f64,
    height: f64,
) -> (PdfDict, Vec<u8>) {
    let font_size = 10.0;
    let margin = 4.0;
    let mut y = height - margin - font_size;

    let mut content = String::new();

    // Border
    content.push_str(&format!(
        "0.5 0.5 0.5 RG 0.95 0.95 0.95 rg 0 0 {} {} re B\n",
        width, height
    ));

    // Text
    content.push_str("BT\n");
    content.push_str("/F1 10 Tf\n");
    content.push_str("0 0 0 rg\n");

    // "Digitally signed by: ..."
    content.push_str(&format!("{} {} Td\n", margin, y));
    content.push_str(&format!(
        "({}) Tj\n",
        escape_pdf_string(&format!("Digitally signed by: {}", signer_name))
    ));
    y -= font_size + 2.0;

    if let Some(reason) = reason {
        content.push_str(&format!("{} {} Td\n", 0.0, -(font_size + 2.0)));
        content.push_str(&format!(
            "({}) Tj\n",
            escape_pdf_string(&format!("Reason: {}", reason))
        ));
        y -= font_size + 2.0;
    }

    if let Some(date) = date {
        content.push_str(&format!("{} {} Td\n", 0.0, -(font_size + 2.0)));
        content.push_str(&format!(
            "({}) Tj\n",
            escape_pdf_string(&format!("Date: {}", date))
        ));
    }

    content.push_str("ET\n");

    // Create the Form XObject
    let (mut stream_dict, stream_data) = make_stream(content.as_bytes(), true);
    stream_dict.insert(b"Type".to_vec(), PdfObject::Name(b"XObject".to_vec()));
    stream_dict.insert(b"Subtype".to_vec(), PdfObject::Name(b"Form".to_vec()));
    stream_dict.insert(
        b"BBox".to_vec(),
        PdfObject::Array(vec![
            PdfObject::Integer(0),
            PdfObject::Integer(0),
            PdfObject::Real(width),
            PdfObject::Real(height),
        ]),
    );

    // Resources with Helvetica font
    let mut font_dict = PdfDict::new();
    let mut f1 = PdfDict::new();
    f1.insert(b"Type".to_vec(), PdfObject::Name(b"Font".to_vec()));
    f1.insert(b"Subtype".to_vec(), PdfObject::Name(b"Type1".to_vec()));
    f1.insert(b"BaseFont".to_vec(), PdfObject::Name(b"Helvetica".to_vec()));
    font_dict.insert(b"F1".to_vec(), PdfObject::Dict(f1));

    let mut resources = PdfDict::new();
    resources.insert(b"Font".to_vec(), PdfObject::Dict(font_dict));
    stream_dict.insert(b"Resources".to_vec(), PdfObject::Dict(resources));

    (stream_dict, stream_data)
}

/// Escape special characters in a PDF string.
fn escape_pdf_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('(', "\\(")
        .replace(')', "\\)")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_appearance() {
        let (dict, data) = generate_signature_appearance(
            "John Doe",
            Some("Approved"),
            Some("2026-01-15"),
            200.0,
            80.0,
        );

        assert_eq!(dict.get_name(b"Type"), Some(b"XObject".as_slice()));
        assert_eq!(dict.get_name(b"Subtype"), Some(b"Form".as_slice()));
        assert!(!data.is_empty());
    }

    #[test]
    fn test_generate_appearance_minimal() {
        let (dict, data) = generate_signature_appearance(
            "Signer",
            None,
            None,
            100.0,
            40.0,
        );
        assert!(!data.is_empty());
        assert!(dict.get(b"Resources").is_some());
    }

    #[test]
    fn test_escape_pdf_string() {
        assert_eq!(escape_pdf_string("Hello (World)"), "Hello \\(World\\)");
        assert_eq!(escape_pdf_string("Back\\slash"), "Back\\\\slash");
    }
}
