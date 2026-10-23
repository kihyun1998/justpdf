use crate::error::{JustPdfError, Result};
use crate::object::{IndirectRef, PdfDict, PdfObject};
use crate::page::Rect;
use crate::writer::modify::DocumentModifier;

use super::appearance::generate_field_appearance;
use super::types::*;

/// Get a field's value by name from a parsed AcroForm.
pub fn get_field_value(form: &AcroForm, field_name: &str) -> Option<PdfObject> {
    form.fields
        .iter()
        .find(|f| f.name == field_name)
        .and_then(|f| f.value.clone())
}

/// Set a field's value by name: `/V` on the field, and for a checkbox or
/// radio button `/AS` on each widget — the value where the widget's `/AP /N`
/// has that state (or the widget has no `/AP`), `/Off` otherwise. For a text
/// field, combo box or list box, each widget's `/AP` becomes `<< /N … >>`
/// drawn from the new value in that widget's `/Rect`; a widget
/// [`generate_field_appearance`] cannot draw keeps its `/AP`. Sets the
/// AcroForm's `/NeedAppearances` so a viewer draws the new value.
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

    let Some(mut field_dict) = dict_at(modifier, &field.field_ref) else {
        return Err(JustPdfError::FormError {
            detail: format!("invalid field object for: {field_name}"),
        });
    };
    field_dict.insert(b"V".to_vec(), value.clone());
    modifier.set_object(field.field_ref.obj_num, PdfObject::Dict(field_dict));

    if matches!(
        field.field_type,
        FieldType::Checkbox | FieldType::RadioButton
    ) && let Some(state) = value.as_name()
    {
        for widget in &field.widgets {
            let Some(mut widget_dict) = dict_at(modifier, widget) else {
                continue;
            };
            let shown = match appearance_states(&widget_dict, modifier) {
                Some(states) if !states.iter().any(|s| s == state) => b"Off".to_vec(),
                _ => state.to_vec(),
            };
            widget_dict.insert(b"AS".to_vec(), PdfObject::Name(shown));
            modifier.set_object(widget.obj_num, PdfObject::Dict(widget_dict));
        }
    }

    if matches!(
        field.field_type,
        FieldType::Text | FieldType::ComboBox | FieldType::ListBox
    ) {
        redraw_widgets(modifier, field, &value);
    }

    set_need_appearances(modifier);
    Ok(())
}

/// Set the `/AP` of each widget of `field` to `<< /N … >>`, drawn from
/// `value` in the widget's own `/Rect`, with the widget's own `/DA` where it
/// has one and the field's otherwise. A widget the generator cannot draw
/// keeps its `/AP`.
fn redraw_widgets(modifier: &mut DocumentModifier, field: &FormField, value: &PdfObject) {
    let mut drawn = field.clone();
    drawn.value = Some(value.clone());
    for widget in &field.widgets {
        let Some(mut widget_dict) = dict_at(modifier, widget) else {
            continue;
        };
        drawn.default_appearance = widget_dict
            .get(b"DA")
            .and_then(|da| direct(modifier, da))
            .and_then(|da| da.as_str().map(|b| String::from_utf8_lossy(b).into_owned()))
            .or_else(|| field.default_appearance.clone());
        drawn.rect = widget_dict
            .get(b"Rect")
            .and_then(|r| direct(modifier, r))
            .and_then(|r| Rect::from_pdf_array(r.as_array()?));
        let Ok(normal) = generate_field_appearance(&drawn, modifier) else {
            continue;
        };
        let mut ap = PdfDict::new();
        ap.insert(b"N".to_vec(), PdfObject::Reference(normal));
        widget_dict.insert(b"AP".to_vec(), PdfObject::Dict(ap));
        modifier.set_object(widget.obj_num, PdfObject::Dict(widget_dict));
    }
}

/// The dictionary object `r` holds in `modifier`.
fn dict_at(modifier: &DocumentModifier, r: &IndirectRef) -> Option<PdfDict> {
    match modifier.find_object_pub(r.obj_num)? {
        PdfObject::Dict(d) => Some(d.clone()),
        _ => None,
    }
}

/// `obj`, resolved in `modifier` when it is a reference.
fn direct(modifier: &DocumentModifier, obj: &PdfObject) -> Option<PdfObject> {
    match obj {
        PdfObject::Reference(r) => modifier.find_object_pub(r.obj_num).cloned(),
        other => Some(other.clone()),
    }
}

/// The appearance state names of a widget: the keys of its `/AP /N`, else of
/// `/AP /D`. `None` when it has neither.
fn appearance_states(widget: &PdfDict, modifier: &DocumentModifier) -> Option<Vec<Vec<u8>>> {
    let ap = direct(modifier, widget.get(b"AP")?)?;
    let ap = ap.as_dict()?;
    [b"N".as_slice(), b"D"].iter().find_map(|key| {
        let states = direct(modifier, ap.get(key)?)?;
        let states = states.as_dict()?;
        Some(states.iter().map(|(name, _)| name.clone()).collect())
    })
}

/// A checkbox's on-state: the first state of its widgets' appearances that is
/// not `/Off`, `/Yes` when they have none.
fn checkbox_on_state(field: &FormField, modifier: &DocumentModifier) -> Vec<u8> {
    field
        .widgets
        .iter()
        .filter_map(|w| appearance_states(&dict_at(modifier, w)?, modifier))
        .flatten()
        .find(|state| state != b"Off")
        .unwrap_or_else(|| b"Yes".to_vec())
}

/// Set `/NeedAppearances true` in the catalog's AcroForm, a direct dictionary
/// or the object a reference points to.
fn set_need_appearances(modifier: &mut DocumentModifier) {
    let catalog = modifier.catalog_ref();
    let Some(mut catalog_dict) = dict_at(modifier, &catalog) else {
        return;
    };
    match catalog_dict.get(b"AcroForm").cloned() {
        Some(PdfObject::Reference(r)) => {
            if let Some(mut acroform) = dict_at(modifier, &r) {
                acroform.insert(b"NeedAppearances".to_vec(), PdfObject::Bool(true));
                modifier.set_object(r.obj_num, PdfObject::Dict(acroform));
            }
        }
        Some(PdfObject::Dict(mut acroform)) => {
            acroform.insert(b"NeedAppearances".to_vec(), PdfObject::Bool(true));
            catalog_dict.insert(b"AcroForm".to_vec(), PdfObject::Dict(acroform));
            modifier.set_object(catalog.obj_num, PdfObject::Dict(catalog_dict));
        }
        _ => {}
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
        PdfObject::Name(checkbox_on_state(field, modifier))
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
                    field_ref: IndirectRef {
                        obj_num: 10,
                        gen_num: 0,
                    },
                    widgets: Vec::new(),
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
                    field_ref: IndirectRef {
                        obj_num: 11,
                        gen_num: 0,
                    },
                    widgets: Vec::new(),
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
                    field_ref: IndirectRef {
                        obj_num: 12,
                        gen_num: 0,
                    },
                    widgets: Vec::new(),
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
