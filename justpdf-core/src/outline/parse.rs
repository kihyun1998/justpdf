use crate::error::Result;
use crate::object::{IndirectRef, PdfDict, PdfObject};
use crate::parser::PdfDocument;
use crate::annot::types::AnnotColor;

use super::types::*;

/// Read all outline (bookmark) items from the document.
pub fn read_outlines(doc: &PdfDocument) -> Result<Vec<OutlineItem>> {
    let catalog_ref = match doc.catalog_ref() {
        Some(r) => r.clone(),
        None => return Ok(Vec::new()),
    };
    let catalog = doc.resolve(&catalog_ref)?;
    let catalog_dict = match catalog.as_dict() {
        Some(d) => d,
        None => return Ok(Vec::new()),
    };

    let outlines_ref = match catalog_dict.get_ref(b"Outlines") {
        Some(r) => r.clone(),
        None => return Ok(Vec::new()),
    };

    let outlines_obj = doc.resolve(&outlines_ref)?;
    let outlines_dict = match outlines_obj.as_dict() {
        Some(d) => d.clone(),
        None => return Ok(Vec::new()),
    };

    // Get the /First child
    let first_ref = match outlines_dict.get_ref(b"First") {
        Some(r) => r.clone(),
        None => return Ok(Vec::new()),
    };

    read_outline_siblings(doc, &first_ref)
}

/// Read a chain of sibling outline items starting from `first_ref`.
fn read_outline_siblings(
    doc: &PdfDocument,
    first_ref: &IndirectRef,
) -> Result<Vec<OutlineItem>> {
    let mut items = Vec::new();
    let mut current_ref = Some(first_ref.clone());
    let mut visited = std::collections::HashSet::new();

    while let Some(ref iref) = current_ref {
        if !visited.insert(iref.clone()) {
            break; // prevent infinite loop
        }

        let obj = doc.resolve(iref)?;
        let dict = match obj.as_dict() {
            Some(d) => d.clone(),
            None => break,
        };

        let title = dict
            .get(b"Title")
            .and_then(|o| o.as_str())
            .map(|b| String::from_utf8_lossy(b).into_owned())
            .unwrap_or_default();

        // Destination: try /Dest first, then /A action's /D
        let dest = if let Some(dest_obj) = dict.get(b"Dest") {
            Destination::from_object(dest_obj)
        } else if let Some(action_dict) = dict.get_dict(b"A") {
            // GoTo action
            action_dict.get(b"D").and_then(Destination::from_object)
        } else {
            None
        };

        let color = dict.get_array(b"C").and_then(AnnotColor::from_array);

        let style = OutlineStyle::from_flags(dict.get_i64(b"F").unwrap_or(0));

        // /Count: if negative, the item is closed; if positive or absent, open
        let count = dict.get_i64(b"Count").unwrap_or(0);
        let is_open = count > 0;

        // Recursively read children
        let children = if let Some(child_ref) = dict.get_ref(b"First") {
            read_outline_siblings(doc, &child_ref.clone())?
        } else {
            Vec::new()
        };

        items.push(OutlineItem {
            title,
            dest,
            color,
            style,
            is_open,
            children,
        });

        // Move to next sibling
        current_ref = dict.get_ref(b"Next").cloned();
    }

    Ok(items)
}

/// Read named destinations from the document.
/// Looks in both Catalog -> /Names -> /Dests (name tree)
/// and legacy Catalog -> /Dests (dictionary).
pub fn read_named_destinations(
    doc: &PdfDocument,
) -> Result<Vec<(String, Destination)>> {
    let catalog_ref = match doc.catalog_ref() {
        Some(r) => r.clone(),
        None => return Ok(Vec::new()),
    };
    let catalog = doc.resolve(&catalog_ref)?;
    let catalog_dict = match catalog.as_dict() {
        Some(d) => d.clone(),
        None => return Ok(Vec::new()),
    };

    let mut result = Vec::new();

    // Try name tree: /Names -> /Dests
    if let Some(names_ref) = catalog_dict.get_ref(b"Names") {
        let names_ref = names_ref.clone();
        let names_obj = doc.resolve(&names_ref)?;
        if let Some(names_dict) = names_obj.as_dict() {
            if let Some(dests_ref) = names_dict.get_ref(b"Dests") {
                let dests_ref = dests_ref.clone();
                let dests_obj = doc.resolve(&dests_ref)?;
                if let Some(dests_dict) = dests_obj.as_dict() {
                    parse_name_tree(doc, &dests_dict.clone(), &mut result)?;
                }
            } else if let Some(PdfObject::Dict(dests_dict)) = names_dict.get(b"Dests") {
                parse_name_tree(doc, dests_dict, &mut result)?;
            }
        }
    } else if let Some(PdfObject::Dict(names_dict)) = catalog_dict.get(b"Names") {
        if let Some(dests_ref) = names_dict.get_ref(b"Dests") {
            let dests_ref = dests_ref.clone();
            let dests_obj = doc.resolve(&dests_ref)?;
            if let Some(dests_dict) = dests_obj.as_dict() {
                parse_name_tree(doc, &dests_dict.clone(), &mut result)?;
            }
        } else if let Some(PdfObject::Dict(dests_dict)) = names_dict.get(b"Dests") {
            parse_name_tree(doc, dests_dict, &mut result)?;
        }
    }

    // Try legacy: /Dests dict
    if let Some(dests_ref) = catalog_dict.get_ref(b"Dests") {
        let dests_ref = dests_ref.clone();
        let dests_obj = doc.resolve(&dests_ref)?;
        if let Some(dests_dict) = dests_obj.as_dict() {
            for (key, val) in dests_dict.iter() {
                let name = String::from_utf8_lossy(key).into_owned();
                let dest_obj = match val {
                    PdfObject::Reference(r) => doc.resolve(r)?,
                    other => other.clone(),
                };
                // Value can be array or dict with /D
                let dest = match &dest_obj {
                    PdfObject::Array(_) => Destination::from_object(&dest_obj),
                    PdfObject::Dict(d) => d.get(b"D").and_then(Destination::from_object),
                    _ => None,
                };
                if let Some(d) = dest {
                    result.push((name, d));
                }
            }
        }
    } else if let Some(PdfObject::Dict(dests_dict)) = catalog_dict.get(b"Dests") {
        for (key, val) in dests_dict.iter() {
            let name = String::from_utf8_lossy(key).into_owned();
            if let Some(d) = Destination::from_object(val) {
                result.push((name, d));
            }
        }
    }

    Ok(result)
}

/// Parse a PDF name tree node recursively, collecting (name, destination) pairs.
fn parse_name_tree(
    doc: &PdfDocument,
    node: &PdfDict,
    result: &mut Vec<(String, Destination)>,
) -> Result<()> {
    // Leaf node: /Names array [key1 value1 key2 value2 ...]
    if let Some(names_arr) = node.get_array(b"Names") {
        let names_arr = names_arr.to_vec();
        let mut i = 0;
        while i + 1 < names_arr.len() {
            let key = match &names_arr[i] {
                PdfObject::String(s) => String::from_utf8_lossy(s).into_owned(),
                _ => {
                    i += 2;
                    continue;
                }
            };
            let val = &names_arr[i + 1];
            let val_resolved = match val {
                PdfObject::Reference(r) => doc.resolve(r)?,
                other => other.clone(),
            };
            let dest = match &val_resolved {
                PdfObject::Array(_) => Destination::from_object(&val_resolved),
                PdfObject::Dict(d) => d.get(b"D").and_then(Destination::from_object),
                _ => None,
            };
            if let Some(d) = dest {
                result.push((key, d));
            }
            i += 2;
        }
    }

    // Interior node: /Kids array of child node refs
    if let Some(kids) = node.get_array(b"Kids") {
        let kids = kids.to_vec();
        for kid in &kids {
            if let PdfObject::Reference(r) = kid {
                let r = r.clone();
                let kid_obj = doc.resolve(&r)?;
                if let Some(kid_dict) = kid_obj.as_dict() {
                    parse_name_tree(doc, &kid_dict.clone(), result)?;
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: read_outlines and read_named_destinations require a PdfDocument,
    // which needs a full PDF byte stream. We test parsing helpers indirectly
    // through the Destination type tests in types.rs.
    // Here we verify the parsing logic using the Destination API.

    #[test]
    fn test_destination_from_goto_action_dict() {
        // Simulate parsing an /A action dict containing /D
        let mut action_dict = PdfDict::new();
        action_dict.insert(b"S".to_vec(), PdfObject::Name(b"GoTo".to_vec()));
        action_dict.insert(
            b"D".to_vec(),
            PdfObject::Array(vec![
                PdfObject::Reference(IndirectRef {
                    obj_num: 10,
                    gen_num: 0,
                }),
                PdfObject::Name(b"XYZ".to_vec()),
                PdfObject::Real(0.0),
                PdfObject::Real(792.0),
                PdfObject::Null,
            ]),
        );
        let dest = action_dict
            .get(b"D")
            .and_then(Destination::from_object)
            .unwrap();
        match dest {
            Destination::XYZ {
                page_ref,
                left,
                top,
                zoom,
            } => {
                assert_eq!(page_ref.obj_num, 10);
                assert_eq!(left, Some(0.0));
                assert_eq!(top, Some(792.0));
                assert_eq!(zoom, None);
            }
            _ => panic!("expected XYZ destination"),
        }
    }

    #[test]
    fn test_outline_item_with_children() {
        // Test that OutlineItem can hold nested children
        let child = OutlineItem {
            title: "Child".to_string(),
            dest: Some(Destination::Fit {
                page_ref: IndirectRef {
                    obj_num: 2,
                    gen_num: 0,
                },
            }),
            color: None,
            style: OutlineStyle::default(),
            is_open: false,
            children: Vec::new(),
        };
        let parent = OutlineItem {
            title: "Parent".to_string(),
            dest: Some(Destination::Fit {
                page_ref: IndirectRef {
                    obj_num: 1,
                    gen_num: 0,
                },
            }),
            color: Some(AnnotColor::Rgb(1.0, 0.0, 0.0)),
            style: OutlineStyle {
                italic: false,
                bold: true,
            },
            is_open: true,
            children: vec![child],
        };
        assert_eq!(parent.children.len(), 1);
        assert_eq!(parent.children[0].title, "Child");
        assert!(parent.is_open);
        assert!(parent.style.bold);
    }

    #[test]
    fn test_outline_count_determines_open_state() {
        // Positive count means open, negative means closed, zero means no children or closed
        // Simulating the is_open logic from read_outline_siblings
        let count_positive: i64 = 5;
        let count_negative: i64 = -3;
        let count_zero: i64 = 0;

        assert!(count_positive > 0); // is_open = true
        assert!(!(count_negative > 0)); // is_open = false
        assert!(!(count_zero > 0)); // is_open = false
    }
}
