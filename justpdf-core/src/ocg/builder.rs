use crate::error::{JustPdfError, Result};
use crate::object::{IndirectRef, PdfDict, PdfObject};
use crate::writer::modify::DocumentModifier;

use super::types::OCGState;

/// Add a new OCG to the document and return its reference.
///
/// Creates the OCG dictionary object with /Type /OCG and /Name,
/// then updates the catalog's /OCProperties to include the new OCG
/// in the /OCGs array and in the default configuration's /ON or /OFF list.
pub fn add_ocg(
    modifier: &mut DocumentModifier,
    name: &str,
    initially_visible: bool,
) -> Result<IndirectRef> {
    // Create the OCG dict
    let mut ocg_dict = PdfDict::new();
    ocg_dict.insert(b"Type".to_vec(), PdfObject::Name(b"OCG".to_vec()));
    ocg_dict.insert(
        b"Name".to_vec(),
        PdfObject::String(name.as_bytes().to_vec()),
    );
    let ocg_ref = modifier.add_object(PdfObject::Dict(ocg_dict));

    // Update catalog's /OCProperties
    let catalog_ref = modifier.catalog_ref().clone();
    let catalog_obj = modifier
        .find_object_pub(catalog_ref.obj_num)
        .cloned()
        .unwrap_or(PdfObject::Null);

    let mut catalog_dict = match catalog_obj {
        PdfObject::Dict(d) => d,
        _ => {
            return Err(JustPdfError::InvalidObject {
                offset: 0,
                detail: "catalog is not a dictionary".into(),
            });
        }
    };

    // Get or create OCProperties dict
    let mut oc_props = match catalog_dict.remove(b"OCProperties") {
        Some(PdfObject::Dict(d)) => d,
        _ => PdfDict::new(),
    };

    // Update /OCGs array
    let mut ocgs = match oc_props.remove(b"OCGs") {
        Some(PdfObject::Array(arr)) => arr,
        _ => Vec::new(),
    };
    ocgs.push(PdfObject::Reference(ocg_ref.clone()));
    oc_props.insert(b"OCGs".to_vec(), PdfObject::Array(ocgs));

    // Update /D (default config)
    let mut d_config = match oc_props.remove(b"D") {
        Some(PdfObject::Dict(d)) => d,
        _ => {
            let mut d = PdfDict::new();
            d.insert(b"BaseState".to_vec(), PdfObject::Name(b"ON".to_vec()));
            d
        }
    };

    let base_state = d_config
        .get_name(b"BaseState")
        .unwrap_or(b"ON");

    if initially_visible {
        if base_state == b"OFF" {
            // Need to add to /ON list
            let mut on_list = match d_config.remove(b"ON") {
                Some(PdfObject::Array(arr)) => arr,
                _ => Vec::new(),
            };
            on_list.push(PdfObject::Reference(ocg_ref.clone()));
            d_config.insert(b"ON".to_vec(), PdfObject::Array(on_list));
        }
        // If base_state is ON, no need to add to /ON list (it's on by default)
    } else {
        if base_state == b"ON" || base_state == b"on" {
            // Need to add to /OFF list
            let mut off_list = match d_config.remove(b"OFF") {
                Some(PdfObject::Array(arr)) => arr,
                _ => Vec::new(),
            };
            off_list.push(PdfObject::Reference(ocg_ref.clone()));
            d_config.insert(b"OFF".to_vec(), PdfObject::Array(off_list));
        }
        // If base_state is OFF, no need to add to /OFF list (it's off by default)
    }

    // Also add to /Order for display
    let mut order = match d_config.remove(b"Order") {
        Some(PdfObject::Array(arr)) => arr,
        _ => Vec::new(),
    };
    order.push(PdfObject::Reference(ocg_ref.clone()));
    d_config.insert(b"Order".to_vec(), PdfObject::Array(order));

    oc_props.insert(b"D".to_vec(), PdfObject::Dict(d_config));
    catalog_dict.insert(b"OCProperties".to_vec(), PdfObject::Dict(oc_props));
    modifier.set_object(catalog_ref.obj_num, PdfObject::Dict(catalog_dict));

    Ok(ocg_ref)
}

/// Set visibility of an OCG in the default configuration.
///
/// Updates the /D config's /ON and /OFF arrays to reflect the desired state.
pub fn set_ocg_visibility(
    modifier: &mut DocumentModifier,
    ocg_ref: &IndirectRef,
    visible: bool,
) -> Result<()> {
    let catalog_ref = modifier.catalog_ref().clone();
    let catalog_obj = modifier
        .find_object_pub(catalog_ref.obj_num)
        .cloned()
        .unwrap_or(PdfObject::Null);

    let mut catalog_dict = match catalog_obj {
        PdfObject::Dict(d) => d,
        _ => {
            return Err(JustPdfError::InvalidObject {
                offset: 0,
                detail: "catalog is not a dictionary".into(),
            });
        }
    };

    let mut oc_props = match catalog_dict.remove(b"OCProperties") {
        Some(PdfObject::Dict(d)) => d,
        _ => {
            return Err(JustPdfError::InvalidObject {
                offset: 0,
                detail: "no OCProperties in catalog".into(),
            });
        }
    };

    let mut d_config = match oc_props.remove(b"D") {
        Some(PdfObject::Dict(d)) => d,
        _ => {
            return Err(JustPdfError::InvalidObject {
                offset: 0,
                detail: "no default configuration in OCProperties".into(),
            });
        }
    };

    let base_state = d_config
        .get_name(b"BaseState")
        .unwrap_or(b"ON");
    let base_is_on = base_state != b"OFF";

    // Remove OCG from both /ON and /OFF lists first
    let on_list = remove_ref_from_array(d_config.remove(b"ON"), ocg_ref);
    let off_list = remove_ref_from_array(d_config.remove(b"OFF"), ocg_ref);

    if visible {
        if !base_is_on {
            // Base is OFF, so add to /ON
            let mut on_list = on_list;
            on_list.push(PdfObject::Reference(ocg_ref.clone()));
            d_config.insert(b"ON".to_vec(), PdfObject::Array(on_list));
        } else if !on_list.is_empty() {
            d_config.insert(b"ON".to_vec(), PdfObject::Array(on_list));
        }
        if !off_list.is_empty() {
            d_config.insert(b"OFF".to_vec(), PdfObject::Array(off_list));
        }
    } else {
        if base_is_on {
            // Base is ON, so add to /OFF
            let mut off_list = off_list;
            off_list.push(PdfObject::Reference(ocg_ref.clone()));
            d_config.insert(b"OFF".to_vec(), PdfObject::Array(off_list));
        } else if !off_list.is_empty() {
            d_config.insert(b"OFF".to_vec(), PdfObject::Array(off_list));
        }
        if !on_list.is_empty() {
            d_config.insert(b"ON".to_vec(), PdfObject::Array(on_list));
        }
    }

    oc_props.insert(b"D".to_vec(), PdfObject::Dict(d_config));
    catalog_dict.insert(b"OCProperties".to_vec(), PdfObject::Dict(oc_props));
    modifier.set_object(catalog_ref.obj_num, PdfObject::Dict(catalog_dict));

    Ok(())
}

/// Remove an OCG from the document.
///
/// Removes the OCG reference from:
/// - /OCProperties /OCGs array
/// - /D config /ON, /OFF, and /Order arrays
pub fn remove_ocg(modifier: &mut DocumentModifier, ocg_ref: &IndirectRef) -> Result<()> {
    let catalog_ref = modifier.catalog_ref().clone();
    let catalog_obj = modifier
        .find_object_pub(catalog_ref.obj_num)
        .cloned()
        .unwrap_or(PdfObject::Null);

    let mut catalog_dict = match catalog_obj {
        PdfObject::Dict(d) => d,
        _ => {
            return Err(JustPdfError::InvalidObject {
                offset: 0,
                detail: "catalog is not a dictionary".into(),
            });
        }
    };

    let mut oc_props = match catalog_dict.remove(b"OCProperties") {
        Some(PdfObject::Dict(d)) => d,
        _ => return Ok(()), // No OCProperties, nothing to remove
    };

    // Remove from /OCGs array
    let ocgs = remove_ref_from_array(oc_props.remove(b"OCGs"), ocg_ref);
    if !ocgs.is_empty() {
        oc_props.insert(b"OCGs".to_vec(), PdfObject::Array(ocgs));
    }

    // Update /D config
    if let Some(PdfObject::Dict(mut d_config)) = oc_props.remove(b"D") {
        let on_list = remove_ref_from_array(d_config.remove(b"ON"), ocg_ref);
        if !on_list.is_empty() {
            d_config.insert(b"ON".to_vec(), PdfObject::Array(on_list));
        }

        let off_list = remove_ref_from_array(d_config.remove(b"OFF"), ocg_ref);
        if !off_list.is_empty() {
            d_config.insert(b"OFF".to_vec(), PdfObject::Array(off_list));
        }

        let order = remove_ref_from_order(d_config.remove(b"Order"), ocg_ref);
        if !order.is_empty() {
            d_config.insert(b"Order".to_vec(), PdfObject::Array(order));
        }

        oc_props.insert(b"D".to_vec(), PdfObject::Dict(d_config));
    }

    catalog_dict.insert(b"OCProperties".to_vec(), PdfObject::Dict(oc_props));
    modifier.set_object(catalog_ref.obj_num, PdfObject::Dict(catalog_dict));

    Ok(())
}

/// Remove all references matching `ocg_ref` from an array.
fn remove_ref_from_array(arr_opt: Option<PdfObject>, ocg_ref: &IndirectRef) -> Vec<PdfObject> {
    match arr_opt {
        Some(PdfObject::Array(arr)) => arr
            .into_iter()
            .filter(|item| {
                if let PdfObject::Reference(r) = item {
                    r != ocg_ref
                } else {
                    true
                }
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// Remove references matching `ocg_ref` from an /Order array,
/// which may contain nested arrays.
fn remove_ref_from_order(arr_opt: Option<PdfObject>, ocg_ref: &IndirectRef) -> Vec<PdfObject> {
    match arr_opt {
        Some(PdfObject::Array(arr)) => arr
            .into_iter()
            .filter_map(|item| match item {
                PdfObject::Reference(ref r) if r == ocg_ref => None,
                PdfObject::Array(sub) => {
                    let filtered =
                        remove_ref_from_order(Some(PdfObject::Array(sub)), ocg_ref);
                    if filtered.is_empty() {
                        None
                    } else {
                        Some(PdfObject::Array(filtered))
                    }
                }
                other => Some(other),
            })
            .collect(),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::page::collect_pages;
    use crate::parser::PdfDocument;
    use crate::writer::document::DocumentBuilder;
    use crate::writer::page::PageBuilder;

    fn create_test_pdf() -> Vec<u8> {
        let mut doc = DocumentBuilder::new();
        let font = doc.add_standard_font("Helvetica");
        let mut page = PageBuilder::new(612.0, 792.0);
        page.add_font(&font, "Helvetica");
        page.begin_text();
        page.set_font(&font, 12.0);
        page.move_to(72.0, 720.0);
        page.show_text("Test page");
        page.end_text();
        doc.add_page(page);
        doc.build().unwrap()
    }

    #[test]
    fn test_add_ocg_visible() {
        let bytes = create_test_pdf();
        let mut doc = PdfDocument::from_bytes(bytes).unwrap();
        let mut modifier = DocumentModifier::from_document(&doc).unwrap();

        let ocg_ref = add_ocg(&mut modifier, "Visible Layer", true).unwrap();
        assert!(ocg_ref.obj_num > 0);

        // Verify the OCG object exists
        let ocg_obj = modifier.find_object_pub(ocg_ref.obj_num).unwrap();
        let ocg_dict = ocg_obj.as_dict().unwrap();
        assert_eq!(ocg_dict.get_name(b"Type"), Some(b"OCG".as_slice()));
        assert_eq!(
            ocg_dict.get_string(b"Name"),
            Some(b"Visible Layer".as_slice())
        );

        // Verify catalog has OCProperties
        let catalog_ref = modifier.catalog_ref().clone();
        let catalog = modifier
            .find_object_pub(catalog_ref.obj_num)
            .unwrap()
            .as_dict()
            .unwrap();
        let oc_props = catalog.get_dict(b"OCProperties").unwrap();
        let ocgs = oc_props.get_array(b"OCGs").unwrap();
        assert_eq!(ocgs.len(), 1);

        // Should NOT be in /OFF list since initially_visible=true and base_state=ON
        let d_config = oc_props.get_dict(b"D").unwrap();
        assert!(d_config.get_array(b"OFF").is_none());
    }

    #[test]
    fn test_add_ocg_hidden() {
        let bytes = create_test_pdf();
        let mut doc = PdfDocument::from_bytes(bytes).unwrap();
        let mut modifier = DocumentModifier::from_document(&doc).unwrap();

        let ocg_ref = add_ocg(&mut modifier, "Hidden Layer", false).unwrap();

        // Verify catalog has OCProperties with the OCG in /OFF
        let catalog_ref = modifier.catalog_ref().clone();
        let catalog = modifier
            .find_object_pub(catalog_ref.obj_num)
            .unwrap()
            .as_dict()
            .unwrap();
        let oc_props = catalog.get_dict(b"OCProperties").unwrap();
        let d_config = oc_props.get_dict(b"D").unwrap();
        let off_list = d_config.get_array(b"OFF").unwrap();
        assert_eq!(off_list.len(), 1);
        assert_eq!(off_list[0].as_reference(), Some(&ocg_ref));
    }

    #[test]
    fn test_add_multiple_ocgs() {
        let bytes = create_test_pdf();
        let mut doc = PdfDocument::from_bytes(bytes).unwrap();
        let mut modifier = DocumentModifier::from_document(&doc).unwrap();

        let _ = add_ocg(&mut modifier, "Layer 1", true).unwrap();
        let ref2 = add_ocg(&mut modifier, "Layer 2", false).unwrap();
        let _ = add_ocg(&mut modifier, "Layer 3", true).unwrap();

        let catalog_ref = modifier.catalog_ref().clone();
        let catalog = modifier
            .find_object_pub(catalog_ref.obj_num)
            .unwrap()
            .as_dict()
            .unwrap();
        let oc_props = catalog.get_dict(b"OCProperties").unwrap();
        let ocgs = oc_props.get_array(b"OCGs").unwrap();
        assert_eq!(ocgs.len(), 3);

        let d_config = oc_props.get_dict(b"D").unwrap();

        // Only Layer 2 should be in /OFF
        let off_list = d_config.get_array(b"OFF").unwrap();
        assert_eq!(off_list.len(), 1);
        assert_eq!(off_list[0].as_reference(), Some(&ref2));

        // /Order should have all 3
        let order = d_config.get_array(b"Order").unwrap();
        assert_eq!(order.len(), 3);
    }

    #[test]
    fn test_set_ocg_visibility_off() {
        let bytes = create_test_pdf();
        let mut doc = PdfDocument::from_bytes(bytes).unwrap();
        let mut modifier = DocumentModifier::from_document(&doc).unwrap();

        let ocg_ref = add_ocg(&mut modifier, "Toggle Layer", true).unwrap();

        // Set to not visible
        set_ocg_visibility(&mut modifier, &ocg_ref, false).unwrap();

        let catalog_ref = modifier.catalog_ref().clone();
        let catalog = modifier
            .find_object_pub(catalog_ref.obj_num)
            .unwrap()
            .as_dict()
            .unwrap();
        let oc_props = catalog.get_dict(b"OCProperties").unwrap();
        let d_config = oc_props.get_dict(b"D").unwrap();
        let off_list = d_config.get_array(b"OFF").unwrap();
        assert!(off_list.iter().any(|item| item.as_reference() == Some(&ocg_ref)));
    }

    #[test]
    fn test_set_ocg_visibility_on() {
        let bytes = create_test_pdf();
        let mut doc = PdfDocument::from_bytes(bytes).unwrap();
        let mut modifier = DocumentModifier::from_document(&doc).unwrap();

        let ocg_ref = add_ocg(&mut modifier, "Toggle Layer", false).unwrap();

        // Set to visible (was initially hidden)
        set_ocg_visibility(&mut modifier, &ocg_ref, true).unwrap();

        let catalog_ref = modifier.catalog_ref().clone();
        let catalog = modifier
            .find_object_pub(catalog_ref.obj_num)
            .unwrap()
            .as_dict()
            .unwrap();
        let oc_props = catalog.get_dict(b"OCProperties").unwrap();
        let d_config = oc_props.get_dict(b"D").unwrap();

        // Should not be in /OFF
        let off = d_config.get_array(b"OFF");
        if let Some(off_list) = off {
            assert!(!off_list.iter().any(|item| item.as_reference() == Some(&ocg_ref)));
        }
    }

    #[test]
    fn test_set_visibility_no_oc_properties_error() {
        let bytes = create_test_pdf();
        let mut doc = PdfDocument::from_bytes(bytes).unwrap();
        let mut modifier = DocumentModifier::from_document(&doc).unwrap();

        let dummy_ref = IndirectRef {
            obj_num: 999,
            gen_num: 0,
        };
        let result = set_ocg_visibility(&mut modifier, &dummy_ref, true);
        assert!(result.is_err());
    }

    #[test]
    fn test_remove_ocg() {
        let bytes = create_test_pdf();
        let mut doc = PdfDocument::from_bytes(bytes).unwrap();
        let mut modifier = DocumentModifier::from_document(&doc).unwrap();

        let ref1 = add_ocg(&mut modifier, "Layer 1", true).unwrap();
        let ref2 = add_ocg(&mut modifier, "Layer 2", true).unwrap();

        // Remove Layer 1
        remove_ocg(&mut modifier, &ref1).unwrap();

        let catalog_ref = modifier.catalog_ref().clone();
        let catalog = modifier
            .find_object_pub(catalog_ref.obj_num)
            .unwrap()
            .as_dict()
            .unwrap();
        let oc_props = catalog.get_dict(b"OCProperties").unwrap();
        let ocgs = oc_props.get_array(b"OCGs").unwrap();
        assert_eq!(ocgs.len(), 1);
        assert_eq!(ocgs[0].as_reference(), Some(&ref2));

        let d_config = oc_props.get_dict(b"D").unwrap();
        let order = d_config.get_array(b"Order").unwrap();
        assert_eq!(order.len(), 1);
        assert_eq!(order[0].as_reference(), Some(&ref2));
    }

    #[test]
    fn test_remove_ocg_no_oc_properties() {
        let bytes = create_test_pdf();
        let mut doc = PdfDocument::from_bytes(bytes).unwrap();
        let mut modifier = DocumentModifier::from_document(&doc).unwrap();

        let dummy_ref = IndirectRef {
            obj_num: 999,
            gen_num: 0,
        };
        // Should not error if there are no OCProperties
        let result = remove_ocg(&mut modifier, &dummy_ref);
        assert!(result.is_ok());
    }

    #[test]
    fn test_add_ocg_roundtrip() {
        let bytes = create_test_pdf();
        let mut doc = PdfDocument::from_bytes(bytes).unwrap();
        let mut modifier = DocumentModifier::from_document(&doc).unwrap();

        add_ocg(&mut modifier, "Roundtrip Layer", true).unwrap();

        // Build and re-parse
        let new_bytes = modifier.build().unwrap();
        let mut reparsed = PdfDocument::from_bytes(new_bytes).unwrap();

        // Should be able to read OC properties
        let props = crate::ocg::read_oc_properties(&reparsed).unwrap();
        assert!(props.is_some());

        let props = props.unwrap();
        assert_eq!(props.groups.len(), 1);
        assert_eq!(props.groups[0].name, "Roundtrip Layer");

        let config = props.default_config.unwrap();
        assert_eq!(config.base_state, OCGState::On);
        assert!(config.off_groups.is_empty());
        assert_eq!(config.order.len(), 1);
    }

    #[test]
    fn test_add_hidden_ocg_roundtrip() {
        let bytes = create_test_pdf();
        let mut doc = PdfDocument::from_bytes(bytes).unwrap();
        let mut modifier = DocumentModifier::from_document(&doc).unwrap();

        let ocg_ref = add_ocg(&mut modifier, "Hidden Layer", false).unwrap();

        let new_bytes = modifier.build().unwrap();
        let mut reparsed = PdfDocument::from_bytes(new_bytes).unwrap();

        let props = crate::ocg::read_oc_properties(&reparsed).unwrap().unwrap();
        let config = props.default_config.unwrap();
        assert_eq!(config.off_groups.len(), 1);

        // Verify visibility
        assert!(!crate::ocg::is_ocg_visible(&ocg_ref, &config));
    }

    #[test]
    fn test_remove_ref_from_array_helper() {
        let target = IndirectRef {
            obj_num: 5,
            gen_num: 0,
        };
        let arr = PdfObject::Array(vec![
            PdfObject::Reference(IndirectRef {
                obj_num: 3,
                gen_num: 0,
            }),
            PdfObject::Reference(target.clone()),
            PdfObject::Reference(IndirectRef {
                obj_num: 7,
                gen_num: 0,
            }),
        ]);
        let result = remove_ref_from_array(Some(arr), &target);
        assert_eq!(result.len(), 2);
        assert!(result.iter().all(|item| item.as_reference() != Some(&target)));
    }

    #[test]
    fn test_remove_ref_from_array_none() {
        let target = IndirectRef {
            obj_num: 5,
            gen_num: 0,
        };
        let result = remove_ref_from_array(None, &target);
        assert!(result.is_empty());
    }

    #[test]
    fn test_remove_ref_from_order_nested() {
        let target = IndirectRef {
            obj_num: 5,
            gen_num: 0,
        };
        let arr = PdfObject::Array(vec![
            PdfObject::Reference(IndirectRef {
                obj_num: 3,
                gen_num: 0,
            }),
            PdfObject::Array(vec![
                PdfObject::Reference(target.clone()),
                PdfObject::Reference(IndirectRef {
                    obj_num: 7,
                    gen_num: 0,
                }),
            ]),
        ]);
        let result = remove_ref_from_order(Some(arr), &target);
        assert_eq!(result.len(), 2);
        // The nested array should now have 1 item
        if let PdfObject::Array(sub) = &result[1] {
            assert_eq!(sub.len(), 1);
        } else {
            panic!("expected nested array");
        }
    }
}
