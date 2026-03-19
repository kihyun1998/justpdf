//! Signature field detection in PDF documents.

use crate::error::{JustPdfError, Result};
use crate::object::{IndirectRef, PdfObject};
use crate::parser::PdfDocument;

use super::cert;
use super::types::SignatureInfo;

/// Detect all digital signatures in a PDF document.
///
/// Walks the AcroForm field tree looking for /FT /Sig fields with a /V value,
/// and extracts the signature information.
pub fn detect_signatures(doc: &PdfDocument) -> Result<Vec<SignatureInfo>> {
    let catalog_ref = doc
        .catalog_ref()
        .ok_or(JustPdfError::TrailerNotFound)?
        .clone();
    let catalog = doc.resolve(&catalog_ref)?;
    let catalog_dict = match catalog.as_dict() {
        Some(d) => d.clone(),
        None => return Ok(Vec::new()),
    };

    let acroform_dict = match catalog_dict.get(b"AcroForm") {
        Some(PdfObject::Dict(d)) => d.clone(),
        Some(PdfObject::Reference(r)) => {
            let resolved = doc.resolve(r)?;
            match resolved.as_dict() {
                Some(d) => d.clone(),
                None => return Ok(Vec::new()),
            }
        }
        _ => return Ok(Vec::new()),
    };

    let fields_arr = match acroform_dict.get(b"Fields") {
        Some(PdfObject::Array(arr)) => arr.clone(),
        _ => return Ok(Vec::new()),
    };

    let mut signatures = Vec::new();

    for field_obj in &fields_arr {
        let field_ref = match field_obj {
            PdfObject::Reference(r) => r.clone(),
            _ => continue,
        };
        collect_sig_fields(doc, &field_ref, "", &mut signatures)?;
    }

    Ok(signatures)
}

/// Recursively walk the field tree looking for signature fields.
fn collect_sig_fields(
    doc: &PdfDocument,
    field_ref: &IndirectRef,
    parent_name: &str,
    sigs: &mut Vec<SignatureInfo>,
) -> Result<()> {
    let obj = doc.resolve(field_ref)?;
    let dict = match obj.as_dict() {
        Some(d) => d.clone(),
        None => return Ok(()),
    };

    // Build field name
    let partial_name = dict
        .get(b"T")
        .and_then(|o| o.as_str())
        .map(|b| String::from_utf8_lossy(b).into_owned())
        .unwrap_or_default();

    let full_name = if parent_name.is_empty() {
        partial_name.clone()
    } else if partial_name.is_empty() {
        parent_name.to_string()
    } else {
        format!("{}.{}", parent_name, partial_name)
    };

    // Check for /Kids — if present, recurse
    if let Some(PdfObject::Array(kids)) = dict.get(b"Kids") {
        let kids = kids.clone();
        for kid in &kids {
            if let PdfObject::Reference(r) = kid {
                collect_sig_fields(doc, r, &full_name, sigs)?;
            }
        }
        return Ok(());
    }

    // Check if this is a signature field
    let ft = dict.get_name(b"FT");
    if ft != Some(b"Sig") {
        return Ok(());
    }

    // Get the signature value dictionary (/V)
    let sig_dict = match dict.get(b"V") {
        Some(PdfObject::Dict(d)) => d.clone(),
        Some(PdfObject::Reference(r)) => {
            let resolved = doc.resolve(r)?;
            match resolved.as_dict() {
                Some(d) => d.clone(),
                None => return Ok(()),
            }
        }
        _ => return Ok(()), // No value = unsigned field
    };

    // Extract signature properties
    let filter = sig_dict
        .get_name(b"Filter")
        .unwrap_or(b"")
        .to_vec();
    let sub_filter = sig_dict
        .get_name(b"SubFilter")
        .unwrap_or(b"")
        .to_vec();

    let byte_range = sig_dict
        .get_array(b"ByteRange")
        .map(|arr| {
            arr.iter()
                .filter_map(|o| o.as_i64())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let contents_raw = sig_dict
        .get_string(b"Contents")
        .unwrap_or(&[])
        .to_vec();

    let signer_name = sig_dict
        .get(b"Name")
        .and_then(|o| o.as_str())
        .map(|b| String::from_utf8_lossy(b).into_owned());

    let signing_time = sig_dict
        .get(b"M")
        .and_then(|o| o.as_str())
        .map(|b| String::from_utf8_lossy(b).into_owned());

    let reason = sig_dict
        .get(b"Reason")
        .and_then(|o| o.as_str())
        .map(|b| String::from_utf8_lossy(b).into_owned());

    let location = sig_dict
        .get(b"Location")
        .and_then(|o| o.as_str())
        .map(|b| String::from_utf8_lossy(b).into_owned());

    let contact_info = sig_dict
        .get(b"ContactInfo")
        .and_then(|o| o.as_str())
        .map(|b| String::from_utf8_lossy(b).into_owned());

    // Parse certificate chain from the CMS data
    let cert_chain = if !contents_raw.is_empty() {
        cert::extract_certificates(&contents_raw).unwrap_or_default()
    } else {
        Vec::new()
    };

    sigs.push(SignatureInfo {
        field_name: full_name,
        field_ref: field_ref.clone(),
        signer_name,
        signing_time,
        reason,
        location,
        contact_info,
        filter,
        sub_filter,
        byte_range,
        contents_raw,
        cert_chain,
    });

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::object::PdfDict;
    use crate::writer::PdfWriter;
    use crate::writer::serialize::serialize_pdf;

    /// Create a PDF with a signature field (unsigned — no /V).
    fn create_pdf_with_sig_field(signed: bool) -> Vec<u8> {
        let mut w = PdfWriter::new();
        let pages_num = w.alloc_object_num();
        let pages_ref = IndirectRef { obj_num: pages_num, gen_num: 0 };

        // Signature field
        let mut sig_field = PdfDict::new();
        sig_field.insert(b"Type".to_vec(), PdfObject::Name(b"Annot".to_vec()));
        sig_field.insert(b"Subtype".to_vec(), PdfObject::Name(b"Widget".to_vec()));
        sig_field.insert(b"FT".to_vec(), PdfObject::Name(b"Sig".to_vec()));
        sig_field.insert(b"T".to_vec(), PdfObject::String(b"Signature1".to_vec()));
        sig_field.insert(
            b"Rect".to_vec(),
            PdfObject::Array(vec![
                PdfObject::Integer(0), PdfObject::Integer(0),
                PdfObject::Integer(0), PdfObject::Integer(0),
            ]),
        );

        if signed {
            // Add a fake /V signature dict
            let mut sig_val = PdfDict::new();
            sig_val.insert(b"Filter".to_vec(), PdfObject::Name(b"Adobe.PPKLite".to_vec()));
            sig_val.insert(b"SubFilter".to_vec(), PdfObject::Name(b"adbe.pkcs7.detached".to_vec()));
            sig_val.insert(b"ByteRange".to_vec(), PdfObject::Array(vec![
                PdfObject::Integer(0), PdfObject::Integer(100),
                PdfObject::Integer(200), PdfObject::Integer(300),
            ]));
            sig_val.insert(b"Contents".to_vec(), PdfObject::String(vec![0u8; 32]));
            sig_val.insert(b"Name".to_vec(), PdfObject::String(b"Test Signer".to_vec()));
            sig_val.insert(b"Reason".to_vec(), PdfObject::String(b"Testing".to_vec()));
            sig_field.insert(b"V".to_vec(), PdfObject::Dict(sig_val));
        }

        let sig_ref = w.add_object(PdfObject::Dict(sig_field));

        // Page
        let mut page = PdfDict::new();
        page.insert(b"Type".to_vec(), PdfObject::Name(b"Page".to_vec()));
        page.insert(b"Parent".to_vec(), PdfObject::Reference(pages_ref.clone()));
        page.insert(b"MediaBox".to_vec(), PdfObject::Array(vec![
            PdfObject::Integer(0), PdfObject::Integer(0),
            PdfObject::Integer(612), PdfObject::Integer(792),
        ]));
        page.insert(b"Annots".to_vec(), PdfObject::Array(vec![
            PdfObject::Reference(sig_ref.clone()),
        ]));
        let page_ref = w.add_object(PdfObject::Dict(page));

        // Pages
        let mut pages = PdfDict::new();
        pages.insert(b"Type".to_vec(), PdfObject::Name(b"Pages".to_vec()));
        pages.insert(b"Kids".to_vec(), PdfObject::Array(vec![PdfObject::Reference(page_ref)]));
        pages.insert(b"Count".to_vec(), PdfObject::Integer(1));
        w.set_object(pages_num, PdfObject::Dict(pages));

        // AcroForm
        let mut acroform = PdfDict::new();
        acroform.insert(b"Fields".to_vec(), PdfObject::Array(vec![
            PdfObject::Reference(sig_ref),
        ]));
        acroform.insert(b"SigFlags".to_vec(), PdfObject::Integer(3));

        // Catalog
        let mut catalog = PdfDict::new();
        catalog.insert(b"Type".to_vec(), PdfObject::Name(b"Catalog".to_vec()));
        catalog.insert(b"Pages".to_vec(), PdfObject::Reference(pages_ref));
        catalog.insert(b"AcroForm".to_vec(), PdfObject::Dict(acroform));
        let catalog_ref = w.add_object(PdfObject::Dict(catalog));

        serialize_pdf(&w.objects, (1, 7), &catalog_ref, None).unwrap()
    }

    #[test]
    fn test_detect_unsigned_sig_field() {
        let bytes = create_pdf_with_sig_field(false);
        let mut doc = PdfDocument::from_bytes(bytes).unwrap();
        let sigs = detect_signatures(&doc).unwrap();
        // Unsigned field has no /V, so it should not be detected as a signature
        assert_eq!(sigs.len(), 0);
    }

    #[test]
    fn test_detect_signed_sig_field() {
        let bytes = create_pdf_with_sig_field(true);
        let mut doc = PdfDocument::from_bytes(bytes).unwrap();
        let sigs = detect_signatures(&doc).unwrap();
        assert_eq!(sigs.len(), 1);
        assert_eq!(sigs[0].field_name, "Signature1");
        assert_eq!(sigs[0].signer_name, Some("Test Signer".to_string()));
        assert_eq!(sigs[0].reason, Some("Testing".to_string()));
        assert_eq!(sigs[0].filter, b"Adobe.PPKLite");
        assert_eq!(sigs[0].sub_filter, b"adbe.pkcs7.detached");
        assert_eq!(sigs[0].byte_range, vec![0, 100, 200, 300]);
    }

    #[test]
    fn test_detect_no_acroform() {
        // A simple PDF without AcroForm
        let bytes = {
            let mut w = PdfWriter::new();
            let mut catalog = PdfDict::new();
            catalog.insert(b"Type".to_vec(), PdfObject::Name(b"Catalog".to_vec()));
            let catalog_ref = w.add_object(PdfObject::Dict(catalog));
            serialize_pdf(&w.objects, (1, 7), &catalog_ref, None).unwrap()
        };
        let mut doc = PdfDocument::from_bytes(bytes).unwrap();
        let sigs = detect_signatures(&doc).unwrap();
        assert!(sigs.is_empty());
    }
}
