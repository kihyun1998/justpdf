use crate::error::{JustPdfError, Result};
use crate::object::PdfObject;
use crate::writer::modify::DocumentModifier;

use super::types::*;

/// Get a field's value by name from a parsed AcroForm.
pub fn get_field_value(form: &AcroForm, field_name: &str) -> Option<PdfObject> {
    form.fields
        .iter()
        .find(|f| f.name == field_name)
        .and_then(|f| f.value.clone())
}

/// Set a field's value by name.
pub fn set_field_value(
    modifier: &mut DocumentModifier,
    form: &AcroForm,
    field_name: &str,
    value: PdfObject,
) -> Result<()> {
    let field = form
        .fields
        .iter()
        .find(|f| f.name == field_name)
        .ok_or_else(|| JustPdfError::FormError {
            detail: format!("field not found: {field_name}"),
        })?;

    if field.flags.is_read_only() {
        return Err(JustPdfError::FormError {
            detail: format!("field is read-only: {field_name}"),
        });
    }

    // Update the field object's /V entry
    let field_obj = modifier
        .find_object_pub(field.field_ref.obj_num)
        .cloned()
        .unwrap_or(PdfObject::Null);

    if let PdfObject::Dict(mut field_dict) = field_obj {
        field_dict.insert(b"V".to_vec(), value.clone());

        // For checkboxes, also update /AS (appearance state)
        if (field.field_type == FieldType::Checkbox || field.field_type == FieldType::RadioButton)
            && let Some(name) = value.as_name()
        {
            field_dict.insert(b"AS".to_vec(), PdfObject::Name(name.to_vec()));
        }

        modifier.set_object(field.field_ref.obj_num, PdfObject::Dict(field_dict));
        Ok(())
    } else {
        Err(JustPdfError::FormError {
            detail: format!("invalid field object for: {field_name}"),
        })
    }
}

/// Toggle a checkbox field.
pub fn toggle_checkbox(
    modifier: &mut DocumentModifier,
    form: &AcroForm,
    field_name: &str,
) -> Result<()> {
    let field = form
        .fields
        .iter()
        .find(|f| f.name == field_name)
        .ok_or_else(|| JustPdfError::FormError {
            detail: format!("field not found: {field_name}"),
        })?;

    if field.field_type != FieldType::Checkbox {
        return Err(JustPdfError::FormError {
            detail: format!("field is not a checkbox: {field_name}"),
        });
    }

    let new_value = if field.is_checked() {
        PdfObject::Name(b"Off".to_vec())
    } else {
        PdfObject::Name(b"Yes".to_vec())
    };

    set_field_value(modifier, form, field_name, new_value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::object::IndirectRef;

    fn make_test_form() -> AcroForm {
        AcroForm {
            fields: vec![
                FormField {
                    name: "name".to_string(),
                    partial_name: "name".to_string(),
                    field_type: FieldType::Text,
                    value: Some(PdfObject::String(b"John".to_vec())),
                    default_value: None,
                    flags: FieldFlags::default(),
                    options: Vec::new(),
                    rect: None,
                    default_appearance: None,
                    field_ref: IndirectRef { obj_num: 10, gen_num: 0 },
                    page_obj_num: None,
                },
                FormField {
                    name: "readonly_field".to_string(),
                    partial_name: "readonly_field".to_string(),
                    field_type: FieldType::Text,
                    value: Some(PdfObject::String(b"Locked".to_vec())),
                    default_value: None,
                    flags: FieldFlags(FieldFlags::READ_ONLY),
                    options: Vec::new(),
                    rect: None,
                    default_appearance: None,
                    field_ref: IndirectRef { obj_num: 11, gen_num: 0 },
                    page_obj_num: None,
                },
                FormField {
                    name: "agree".to_string(),
                    partial_name: "agree".to_string(),
                    field_type: FieldType::Checkbox,
                    value: Some(PdfObject::Name(b"Off".to_vec())),
                    default_value: None,
                    flags: FieldFlags::default(),
                    options: Vec::new(),
                    rect: None,
                    default_appearance: None,
                    field_ref: IndirectRef { obj_num: 12, gen_num: 0 },
                    page_obj_num: None,
                },
            ],
            need_appearances: false,
            sig_flags: 0,
            default_appearance: None,
        }
    }

    #[test]
    fn test_get_field_value() {
        let form = make_test_form();
        let val = get_field_value(&form, "name");
        assert_eq!(val, Some(PdfObject::String(b"John".to_vec())));

        let missing = get_field_value(&form, "nonexistent");
        assert!(missing.is_none());
    }

    #[test]
    fn test_get_field_value_no_form() {
        let form = AcroForm {
            fields: Vec::new(),
            need_appearances: false,
            sig_flags: 0,
            default_appearance: None,
        };
        assert!(get_field_value(&form, "anything").is_none());
    }
}
