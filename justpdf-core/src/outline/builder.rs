use crate::error::Result;
use crate::object::{IndirectRef, PdfDict, PdfObject};
use crate::writer::modify::DocumentModifier;

use super::types::*;

/// Add an outline tree to a document.
pub fn set_outlines(modifier: &mut DocumentModifier, items: &[OutlineItem]) -> Result<()> {
    if items.is_empty() {
        return Ok(());
    }

    // Create the outline root dict
    let outlines_obj_num = modifier.writer().alloc_object_num();
    let outlines_ref = IndirectRef {
        obj_num: outlines_obj_num,
        gen_num: 0,
    };

    let (first_ref, last_ref, total_count) =
        build_outline_children(modifier, items, &outlines_ref)?;

    let mut outlines_dict = PdfDict::new();
    outlines_dict.insert(b"Type".to_vec(), PdfObject::Name(b"Outlines".to_vec()));
    outlines_dict.insert(b"First".to_vec(), PdfObject::Reference(first_ref));
    outlines_dict.insert(b"Last".to_vec(), PdfObject::Reference(last_ref));
    outlines_dict.insert(b"Count".to_vec(), PdfObject::Integer(total_count));
    modifier.set_object(outlines_obj_num, PdfObject::Dict(outlines_dict));

    // Update catalog to reference outlines
    let catalog_ref = modifier.catalog_ref().clone();
    if let Some(catalog_obj) = modifier.find_object_pub(catalog_ref.obj_num).cloned() {
        if let PdfObject::Dict(mut catalog_dict) = catalog_obj {
            catalog_dict.insert(
                b"Outlines".to_vec(),
                PdfObject::Reference(outlines_ref),
            );
            modifier.set_object(catalog_ref.obj_num, PdfObject::Dict(catalog_dict));
        }
    }

    Ok(())
}

/// Build a chain of outline item objects as siblings under `parent_ref`.
/// Returns (first_ref, last_ref, visible_count).
fn build_outline_children(
    modifier: &mut DocumentModifier,
    items: &[OutlineItem],
    parent_ref: &IndirectRef,
) -> Result<(IndirectRef, IndirectRef, i64)> {
    // Pre-allocate object numbers for all items
    let item_nums: Vec<u32> = items
        .iter()
        .map(|_| modifier.writer().alloc_object_num())
        .collect();
    let item_refs: Vec<IndirectRef> = item_nums
        .iter()
        .map(|&n| IndirectRef {
            obj_num: n,
            gen_num: 0,
        })
        .collect();

    let mut total_count = items.len() as i64;

    for (i, item) in items.iter().enumerate() {
        let mut dict = PdfDict::new();
        dict.insert(
            b"Title".to_vec(),
            PdfObject::String(item.title.as_bytes().to_vec()),
        );
        dict.insert(
            b"Parent".to_vec(),
            PdfObject::Reference(parent_ref.clone()),
        );

        // Prev/Next links
        if i > 0 {
            dict.insert(
                b"Prev".to_vec(),
                PdfObject::Reference(item_refs[i - 1].clone()),
            );
        }
        if i + 1 < items.len() {
            dict.insert(
                b"Next".to_vec(),
                PdfObject::Reference(item_refs[i + 1].clone()),
            );
        }

        // Destination
        if let Some(ref dest) = item.dest {
            dict.insert(b"Dest".to_vec(), dest.to_pdf_array());
        }

        // Color
        if let Some(ref color) = item.color {
            dict.insert(b"C".to_vec(), PdfObject::Array(color.to_pdf_array()));
        }

        // Style flags
        let flags = item.style.to_flags();
        if flags != 0 {
            dict.insert(b"F".to_vec(), PdfObject::Integer(flags));
        }

        // Children
        if !item.children.is_empty() {
            let (child_first, child_last, child_count) =
                build_outline_children(modifier, &item.children, &item_refs[i])?;
            dict.insert(b"First".to_vec(), PdfObject::Reference(child_first));
            dict.insert(b"Last".to_vec(), PdfObject::Reference(child_last));
            let count_val = if item.is_open {
                child_count
            } else {
                -child_count
            };
            dict.insert(b"Count".to_vec(), PdfObject::Integer(count_val));
            if item.is_open {
                total_count += child_count;
            }
        }

        modifier.set_object(item_nums[i], PdfObject::Dict(dict));
    }

    Ok((
        item_refs.first().unwrap().clone(),
        item_refs.last().unwrap().clone(),
        total_count,
    ))
}

/// Remove all outlines from the document.
pub fn remove_outlines(modifier: &mut DocumentModifier) -> Result<()> {
    let catalog_ref = modifier.catalog_ref().clone();
    if let Some(catalog_obj) = modifier.find_object_pub(catalog_ref.obj_num).cloned() {
        if let PdfObject::Dict(mut catalog_dict) = catalog_obj {
            catalog_dict.remove(b"Outlines");
            modifier.set_object(catalog_ref.obj_num, PdfObject::Dict(catalog_dict));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::types::*;
    use crate::annot::types::AnnotColor;
    use crate::object::IndirectRef;

    fn page_ref(n: u32) -> IndirectRef {
        IndirectRef {
            obj_num: n,
            gen_num: 0,
        }
    }

    #[test]
    fn test_build_outline_items_structure() {
        // Verify that OutlineItem can represent a complex tree
        let items = vec![
            OutlineItem {
                title: "Chapter 1".to_string(),
                dest: Some(Destination::Fit {
                    page_ref: page_ref(1),
                }),
                color: None,
                style: OutlineStyle::default(),
                is_open: true,
                children: vec![
                    OutlineItem {
                        title: "Section 1.1".to_string(),
                        dest: Some(Destination::XYZ {
                            page_ref: page_ref(2),
                            left: Some(0.0),
                            top: Some(792.0),
                            zoom: None,
                        }),
                        color: None,
                        style: OutlineStyle::default(),
                        is_open: false,
                        children: Vec::new(),
                    },
                    OutlineItem {
                        title: "Section 1.2".to_string(),
                        dest: Some(Destination::FitH {
                            page_ref: page_ref(3),
                            top: Some(500.0),
                        }),
                        color: Some(AnnotColor::Rgb(0.0, 0.0, 1.0)),
                        style: OutlineStyle {
                            italic: true,
                            bold: false,
                        },
                        is_open: false,
                        children: Vec::new(),
                    },
                ],
            },
            OutlineItem {
                title: "Chapter 2".to_string(),
                dest: Some(Destination::Fit {
                    page_ref: page_ref(4),
                }),
                color: None,
                style: OutlineStyle {
                    italic: false,
                    bold: true,
                },
                is_open: false,
                children: Vec::new(),
            },
        ];

        assert_eq!(items.len(), 2);
        assert_eq!(items[0].children.len(), 2);
        assert_eq!(items[0].children[0].title, "Section 1.1");
        assert_eq!(items[0].children[1].title, "Section 1.2");
        assert!(items[0].is_open);
        assert!(!items[1].is_open);
        assert!(items[1].style.bold);
        assert!(items[0].children[1].style.italic);
    }

    #[test]
    fn test_outline_item_to_pdf_dest() {
        // Test that each outline item's dest can be serialized
        let item = OutlineItem {
            title: "Test".to_string(),
            dest: Some(Destination::XYZ {
                page_ref: page_ref(5),
                left: Some(72.0),
                top: Some(720.0),
                zoom: Some(1.0),
            }),
            color: None,
            style: OutlineStyle::default(),
            is_open: false,
            children: Vec::new(),
        };

        let pdf_obj = item.dest.as_ref().unwrap().to_pdf_array();
        match &pdf_obj {
            crate::object::PdfObject::Array(arr) => {
                assert_eq!(arr.len(), 5);
                assert!(arr[0].is_reference());
                assert_eq!(arr[1].as_name(), Some(b"XYZ".as_slice()));
            }
            _ => panic!("expected array"),
        }
    }

    #[test]
    fn test_empty_outline_items() {
        let items: Vec<OutlineItem> = Vec::new();
        assert!(items.is_empty());
    }

    #[test]
    fn test_outline_with_named_dest() {
        let item = OutlineItem {
            title: "Named Dest Bookmark".to_string(),
            dest: Some(Destination::Named("chapter3".to_string())),
            color: None,
            style: OutlineStyle::default(),
            is_open: false,
            children: Vec::new(),
        };

        let pdf_obj = item.dest.as_ref().unwrap().to_pdf_array();
        match &pdf_obj {
            crate::object::PdfObject::String(s) => {
                assert_eq!(s, b"chapter3");
            }
            _ => panic!("expected string for named dest"),
        }
    }

    #[test]
    fn test_outline_color_serialization() {
        let color = AnnotColor::Rgb(1.0, 0.0, 0.0);
        let arr = color.to_pdf_array();
        assert_eq!(arr.len(), 3);

        let restored = AnnotColor::from_array(&arr).unwrap();
        assert_eq!(restored, AnnotColor::Rgb(1.0, 0.0, 0.0));
    }

    #[test]
    fn test_deeply_nested_outlines() {
        // Test 3 levels of nesting
        let leaf = OutlineItem {
            title: "Leaf".to_string(),
            dest: Some(Destination::Fit {
                page_ref: page_ref(10),
            }),
            color: None,
            style: OutlineStyle::default(),
            is_open: false,
            children: Vec::new(),
        };
        let mid = OutlineItem {
            title: "Middle".to_string(),
            dest: Some(Destination::Fit {
                page_ref: page_ref(5),
            }),
            color: None,
            style: OutlineStyle::default(),
            is_open: true,
            children: vec![leaf],
        };
        let root = OutlineItem {
            title: "Root".to_string(),
            dest: Some(Destination::Fit {
                page_ref: page_ref(1),
            }),
            color: None,
            style: OutlineStyle::default(),
            is_open: true,
            children: vec![mid],
        };

        assert_eq!(root.children[0].children[0].title, "Leaf");
        assert_eq!(root.children[0].children[0].children.len(), 0);
    }
}
