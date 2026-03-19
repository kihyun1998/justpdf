use std::fmt::Write;

use crate::error::Result;
use crate::object::PdfObject;
use crate::page::Rect;
use crate::parser::PdfDocument;
use crate::writer::encode::make_stream;
use crate::writer::modify::DocumentModifier;

use super::parse::parse_acroform;

/// Flatten all form fields: bake appearance streams into page content,
/// remove widget annotations and AcroForm.
pub fn flatten_form(
    modifier: &mut DocumentModifier,
    doc: &mut PdfDocument,
) -> Result<()> {
    let acroform = match parse_acroform(doc)? {
        Some(f) => f,
        None => return Ok(()), // No form to flatten
    };

    if acroform.fields.is_empty() {
        return Ok(());
    }

    // Collect widget annotation references and their appearance streams
    // Walk all pages and check for Widget annotations
    let pages = crate::page::collect_pages(doc)?;
    for page_info in &pages {
        let page_obj = doc.resolve(&page_info.page_ref)?.clone();
        let page_dict = match page_obj.as_dict() {
            Some(d) => d.clone(),
            None => continue,
        };

        let annots = match page_dict.get(b"Annots") {
            Some(PdfObject::Array(arr)) => arr.clone(),
            Some(PdfObject::Reference(r)) => {
                let resolved = doc.resolve(r)?.clone();
                match resolved.as_array() {
                    Some(arr) => arr.to_vec(),
                    None => continue,
                }
            }
            _ => continue,
        };

        let mut remaining_annots = Vec::new();
        let mut flatten_content = String::new();

        for item in &annots {
            let (annot_dict, _annot_ref) = match item {
                PdfObject::Reference(r) => {
                    let resolved = doc.resolve(r)?.clone();
                    match resolved {
                        PdfObject::Dict(d) => (d, Some(r.clone())),
                        _ => {
                            remaining_annots.push(item.clone());
                            continue;
                        }
                    }
                }
                PdfObject::Dict(d) => (d.clone(), None),
                _ => {
                    remaining_annots.push(item.clone());
                    continue;
                }
            };

            let subtype = annot_dict.get_name(b"Subtype").unwrap_or(b"");
            if subtype != b"Widget" {
                remaining_annots.push(item.clone());
                continue;
            }

            // Get appearance stream ref
            let ap_ref = annot_dict
                .get_dict(b"AP")
                .and_then(|ap| {
                    // Try /N first (normal appearance)
                    ap.get(b"N")
                })
                .and_then(|n| match n {
                    PdfObject::Reference(r) => Some(r.clone()),
                    _ => None,
                });

            if let Some(ap_ref) = ap_ref {
                // Get widget rect for positioning
                let rect = annot_dict
                    .get_array(b"Rect")
                    .and_then(Rect::from_pdf_array);

                if let Some(rect) = rect {
                    // Add content to draw the appearance stream at the widget's position
                    let w = rect.width();
                    let h = rect.height();
                    if w > 0.0 && h > 0.0 {
                        let _ = write!(
                            flatten_content,
                            "q\n{} 0 0 {} {} {} cm\n/Fm{} Do\nQ\n",
                            w, h, rect.llx, rect.lly, ap_ref.obj_num
                        );
                    }
                }
            }
            // Widget annotation is consumed (not added to remaining_annots)
        }

        if !flatten_content.is_empty() {
            // Create a new content stream with the flattened widget content
            let (stream_dict, stream_data) =
                make_stream(flatten_content.as_bytes(), true);
            let content_ref = modifier.add_object(PdfObject::Stream {
                dict: stream_dict,
                data: stream_data,
            });

            // Update page: append new content stream, update annots
            let mut updated_page = page_dict.clone();

            // Append to existing content streams
            let mut contents = match updated_page.remove(b"Contents") {
                Some(PdfObject::Array(arr)) => arr,
                Some(other) => vec![other],
                None => Vec::new(),
            };
            contents.push(PdfObject::Reference(content_ref));
            updated_page.insert(b"Contents".to_vec(), PdfObject::Array(contents));

            // Update annotations
            if remaining_annots.is_empty() {
                updated_page.remove(b"Annots");
            } else {
                updated_page.insert(
                    b"Annots".to_vec(),
                    PdfObject::Array(remaining_annots),
                );
            }

            modifier.set_object(
                page_info.page_ref.obj_num,
                PdfObject::Dict(updated_page),
            );
        }
    }

    // Remove AcroForm from catalog
    let catalog_ref = modifier.catalog_ref().clone();
    let catalog_obj = modifier
        .find_object_pub(catalog_ref.obj_num)
        .cloned()
        .unwrap_or(PdfObject::Null);

    if let PdfObject::Dict(mut catalog_dict) = catalog_obj {
        catalog_dict.remove(b"AcroForm");
        modifier.set_object(catalog_ref.obj_num, PdfObject::Dict(catalog_dict));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    // Integration tests for flatten are in the integration test file
    // since they require creating a full PDF with form fields.
}
