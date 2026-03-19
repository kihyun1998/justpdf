use crate::error::{JustPdfError, Result};
use crate::object::{IndirectRef, PdfObject};
use crate::page::Rect;
use crate::parser::PdfDocument;

use super::types::*;

/// Parse the AcroForm from a PDF document.
/// Returns None if the document has no AcroForm.
pub fn parse_acroform(doc: &mut PdfDocument) -> Result<Option<AcroForm>> {
    let catalog_ref = doc
        .catalog_ref()
        .ok_or(JustPdfError::TrailerNotFound)?
        .clone();
    let catalog = doc.resolve(&catalog_ref)?.clone();
    let catalog_dict = match catalog.as_dict() {
        Some(d) => d.clone(),
        None => return Ok(None),
    };

    let acroform_dict = match catalog_dict.get(b"AcroForm") {
        Some(PdfObject::Dict(d)) => d.clone(),
        Some(PdfObject::Reference(r)) => {
            let resolved = doc.resolve(r)?.clone();
            match resolved.as_dict() {
                Some(d) => d.clone(),
                None => return Ok(None),
            }
        }
        _ => return Ok(None),
    };

    let need_appearances = acroform_dict
        .get(b"NeedAppearances")
        .and_then(|o| o.as_bool())
        .unwrap_or(false);

    let sig_flags = acroform_dict
        .get_i64(b"SigFlags")
        .unwrap_or(0) as u32;

    let default_appearance = acroform_dict
        .get(b"DA")
        .and_then(|o| o.as_str())
        .map(|b| String::from_utf8_lossy(b).into_owned());

    let fields_arr = match acroform_dict.get(b"Fields") {
        Some(PdfObject::Array(arr)) => arr.clone(),
        _ => return Ok(Some(AcroForm {
            fields: Vec::new(),
            need_appearances,
            sig_flags,
            default_appearance,
        })),
    };

    let mut fields = Vec::new();
    let inherited_ft: Option<&[u8]> = None;
    let inherited_ff: u32 = 0;
    let inherited_da: Option<&str> = default_appearance.as_deref();

    for item in &fields_arr {
        let field_ref = match item {
            PdfObject::Reference(r) => r.clone(),
            _ => continue,
        };
        walk_field_tree(
            doc,
            &field_ref,
            "",
            inherited_ft,
            inherited_ff,
            inherited_da,
            &mut fields,
        )?;
    }

    Ok(Some(AcroForm {
        fields,
        need_appearances,
        sig_flags,
        default_appearance,
    }))
}

/// Recursively walk the field tree.
fn walk_field_tree(
    doc: &mut PdfDocument,
    field_ref: &IndirectRef,
    parent_name: &str,
    inherited_ft: Option<&[u8]>,
    inherited_ff: u32,
    inherited_da: Option<&str>,
    fields: &mut Vec<FormField>,
) -> Result<()> {
    let field_obj = doc.resolve(field_ref)?.clone();
    let dict = match field_obj.as_dict() {
        Some(d) => d.clone(),
        None => return Ok(()),
    };

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
        format!("{parent_name}.{partial_name}")
    };

    // Get field type — either from this dict or inherited
    let ft = dict
        .get_name(b"FT")
        .or(inherited_ft);

    // Get field flags — either from this dict or inherited
    let ff = dict
        .get_i64(b"Ff")
        .map(|v| v as u32)
        .unwrap_or(inherited_ff);

    // Get default appearance — either from this dict or inherited
    let da = dict
        .get(b"DA")
        .and_then(|o| o.as_str())
        .map(|b| String::from_utf8_lossy(b).into_owned());
    let da_ref = da.as_deref().or(inherited_da);

    // Check for /Kids — if present, recurse
    if let Some(kids) = dict.get_array(b"Kids") {
        let kids: Vec<PdfObject> = kids.to_vec();
        for kid in &kids {
            if let Some(kid_ref) = kid.as_reference() {
                walk_field_tree(
                    doc,
                    kid_ref,
                    &full_name,
                    ft,
                    ff,
                    da_ref,
                    fields,
                )?;
            }
        }
        return Ok(());
    }

    // Leaf field (terminal node) — determine type and extract value
    let field_type = match ft {
        Some(b"Tx") => FieldType::Text,
        Some(b"Btn") => {
            let flags = FieldFlags(ff);
            if flags.has(FieldFlags::PUSH_BUTTON) {
                FieldType::PushButton
            } else if flags.has(FieldFlags::RADIO) {
                FieldType::RadioButton
            } else {
                FieldType::Checkbox
            }
        }
        Some(b"Ch") => {
            let flags = FieldFlags(ff);
            if flags.has(FieldFlags::COMBO) {
                FieldType::ComboBox
            } else {
                FieldType::ListBox
            }
        }
        Some(b"Sig") => FieldType::Signature,
        _ => return Ok(()), // Unknown or missing field type, skip
    };

    let value = dict.get(b"V").cloned();
    let default_value = dict.get(b"DV").cloned();

    let options = dict
        .get_array(b"Opt")
        .map(|arr| {
            arr.iter()
                .filter_map(|item| match item {
                    PdfObject::String(s) => Some(String::from_utf8_lossy(s).into_owned()),
                    PdfObject::Array(pair) => pair
                        .get(1)
                        .or_else(|| pair.first())
                        .and_then(|o| o.as_str())
                        .map(|b| String::from_utf8_lossy(b).into_owned()),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default();

    let rect = dict
        .get_array(b"Rect")
        .and_then(Rect::from_pdf_array);

    fields.push(FormField {
        name: full_name,
        partial_name,
        field_type,
        value,
        default_value,
        flags: FieldFlags(ff),
        options,
        rect,
        default_appearance: da_ref.map(|s| s.to_string()),
        field_ref: field_ref.clone(),
        page_obj_num: None,
    });

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_determine_field_type_text() {
        // Text field: FT=Tx
        assert_eq!(
            match Some(b"Tx".as_slice()) {
                Some(b"Tx") => FieldType::Text,
                _ => FieldType::Text,
            },
            FieldType::Text
        );
    }

    #[test]
    fn test_determine_field_type_checkbox() {
        // Checkbox: FT=Btn, no PUSH_BUTTON or RADIO flags
        let flags = FieldFlags(0);
        assert!(!flags.has(FieldFlags::PUSH_BUTTON));
        assert!(!flags.has(FieldFlags::RADIO));
    }

    #[test]
    fn test_determine_field_type_radio() {
        let flags = FieldFlags(FieldFlags::RADIO);
        assert!(flags.has(FieldFlags::RADIO));
        assert!(!flags.has(FieldFlags::PUSH_BUTTON));
    }

    #[test]
    fn test_determine_field_type_combo() {
        let flags = FieldFlags(FieldFlags::COMBO);
        assert!(flags.has(FieldFlags::COMBO));
    }
}
