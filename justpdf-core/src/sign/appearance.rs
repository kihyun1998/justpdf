//! Signature appearance stream generation.

use crate::font::encode_winansi;
use crate::object::{Number, PdfDict, PdfObject, string_syntax};
use crate::writer::document::standard_font_dict;
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
    let y = height - margin - font_size;

    let mut content = String::new();

    // Border
    content.push_str(&format!(
        "0.5 0.5 0.5 RG 0.95 0.95 0.95 rg 0 0 {} {} re B\n",
        Number(width),
        Number(height)
    ));

    // Text
    content.push_str("BT\n");
    content.push_str("/F1 10 Tf\n");
    content.push_str("0 0 0 rg\n");

    // "Digitally signed by: ..."
    content.push_str(&format!("{} {} Td\n", Number(margin), Number(y)));
    content.push_str(&format!(
        "{} Tj\n",
        string_syntax(&encode_winansi(&format!(
            "Digitally signed by: {}",
            signer_name
        )))
    ));
    if let Some(reason) = reason {
        content.push_str(&format!("0 {} Td\n", Number(-(font_size + 2.0))));
        content.push_str(&format!(
            "{} Tj\n",
            string_syntax(&encode_winansi(&format!("Reason: {}", reason)))
        ));
    }

    if let Some(date) = date {
        content.push_str(&format!("0 {} Td\n", Number(-(font_size + 2.0))));
        content.push_str(&format!(
            "{} Tj\n",
            string_syntax(&encode_winansi(&format!("Date: {}", date)))
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
    font_dict.insert(
        b"F1".to_vec(),
        PdfObject::Dict(standard_font_dict("Helvetica")),
    );

    let mut resources = PdfDict::new();
    resources.insert(b"Font".to_vec(), PdfObject::Dict(font_dict));
    stream_dict.insert(b"Resources".to_vec(), PdfObject::Dict(resources));

    (stream_dict, stream_data)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Operators in `content` that come from a number written as `NaN` or `inf`.
    fn non_finite_operators(content: &[u8]) -> Vec<Vec<u8>> {
        crate::content::parse_content_stream(content)
            .unwrap()
            .into_iter()
            .map(|op| op.operator)
            .filter(|op| {
                op.starts_with(b"NaN") || op.starts_with(b"inf") || op.starts_with(b"-inf")
            })
            .collect()
    }

    #[test]
    fn test_non_finite_sizes_are_written_as_numbers() {
        let (dict, data) =
            generate_signature_appearance("A", Some("R"), Some("D"), f64::NAN, f64::INFINITY);
        let content = crate::stream::decode_stream(&data, &dict).unwrap();
        assert_eq!(
            non_finite_operators(&content),
            Vec::<Vec<u8>>::new(),
            "{}",
            String::from_utf8_lossy(&content)
        );
    }

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
        let (dict, data) = generate_signature_appearance("Signer", None, None, 100.0, 40.0);
        assert!(!data.is_empty());
        assert!(dict.get(b"Resources").is_some());
    }

    #[test]
    fn test_text_reads_back_unchanged() {
        let (dict, data) =
            generate_signature_appearance("A) B\r", Some("R(\u{e9}"), Some("D\\"), 200.0, 80.0);
        let content = crate::stream::decode_stream(&data, &dict).unwrap();

        let ops = crate::content::parse_content_stream(&content).unwrap();
        let shown: Vec<_> = ops
            .iter()
            .filter(|op| op.operator == b"Tj")
            .map(|op| op.operands.clone())
            .collect();
        let string = |s: &[u8]| vec![crate::content::Operand::String(s.to_vec())];
        assert_eq!(
            shown,
            vec![
                string(b"Digitally signed by: A) B\r"),
                string(b"Reason: R(\xE9"),
                string(b"Date: D\\")
            ]
        );
    }

    #[test]
    fn test_text_is_win_ansi_in_a_declared_win_ansi_font() {
        let (dict, data) = generate_signature_appearance("Müller", None, None, 200.0, 80.0);
        let content = crate::stream::decode_stream(&data, &dict).unwrap();
        let ops = crate::content::parse_content_stream(&content).unwrap();
        let tj = ops.iter().find(|op| op.operator == b"Tj").unwrap();
        assert_eq!(
            tj.operands,
            vec![crate::content::Operand::String(
                b"Digitally signed by: M\xFCller".to_vec()
            )]
        );
        let f1 = dict
            .get_dict(b"Resources")
            .and_then(|r| r.get_dict(b"Font"))
            .and_then(|f| f.get_dict(b"F1"))
            .unwrap();
        assert_eq!(
            f1.get_name(b"Encoding"),
            Some(b"WinAnsiEncoding".as_slice())
        );
    }
}
