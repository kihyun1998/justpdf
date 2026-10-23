use std::fmt::Write;

use crate::annot::placement::{appearance_matrix, is_hidden, normal_appearance};
use crate::error::Result;
use crate::object::{IndirectRef, Number, PdfDict, PdfObject};
use crate::page::Rect;
use crate::parser::PdfDocument;
use crate::writer::encode::make_stream;
use crate::writer::modify::DocumentModifier;

use super::parse::parse_acroform;

/// Flatten all form fields: draw each widget's normal appearance into its
/// page's content (ISO 32000-2 §12.5.5), remove every widget annotation, and
/// remove the AcroForm. Objects are read from `modifier`, falling back to
/// `doc`, so values filled in `modifier` are the ones drawn.
pub fn flatten_form(modifier: &mut DocumentModifier, doc: &PdfDocument) -> Result<()> {
    if parse_acroform(doc)?.is_none() {
        return Ok(());
    }

    let pages = crate::page::collect_pages(doc)?;
    for page_info in &pages {
        let Some(PdfObject::Dict(mut page_dict)) = object_at(modifier, doc, &page_info.page_ref)
        else {
            continue;
        };
        let Some(PdfObject::Array(annots)) = page_dict
            .get(b"Annots")
            .and_then(|a| direct(modifier, doc, a))
        else {
            continue;
        };

        let mut remaining = Vec::new();
        let mut drawn = String::new();
        let mut xobjects: Vec<(Vec<u8>, IndirectRef)> = Vec::new();
        let mut removed_widget = false;
        let resources = effective_resources(modifier, doc, &page_dict, page_info);
        let mut taken = xobject_names(modifier, doc, &resources);

        for item in &annots {
            let Some(PdfObject::Dict(annot)) = direct(modifier, doc, item) else {
                remaining.push(item.clone());
                continue;
            };
            if annot.get_name(b"Subtype") != Some(b"Widget") {
                remaining.push(item.clone());
                continue;
            }
            removed_widget = true;
            if is_hidden(&annot) {
                continue;
            }
            let Some((Some(ap_ref), PdfObject::Stream { dict: form, .. })) =
                normal_appearance(&annot, |r| object_at(modifier, doc, r))
            else {
                continue;
            };
            let Some(rect) = annot.get_array(b"Rect").and_then(Rect::from_pdf_array) else {
                continue;
            };
            let Some(m) = appearance_matrix(&form, &rect) else {
                continue;
            };
            let name = free_name(&mut taken);
            let _ = write!(
                drawn,
                "q\n{} {} {} {} {} {} cm\n/{} Do\nQ\n",
                Number(m[0]),
                Number(m[1]),
                Number(m[2]),
                Number(m[3]),
                Number(m[4]),
                Number(m[5]),
                String::from_utf8_lossy(&name)
            );
            xobjects.push((name, ap_ref));
        }

        if !removed_widget {
            continue;
        }
        if remaining.is_empty() {
            page_dict.remove(b"Annots");
        } else {
            page_dict.insert(b"Annots".to_vec(), PdfObject::Array(remaining));
        }
        if !drawn.is_empty() {
            add_drawn_content(modifier, &mut page_dict, &drawn);
            page_dict.insert(
                b"Resources".to_vec(),
                PdfObject::Dict(with_xobjects(modifier, doc, resources, xobjects)),
            );
        }
        modifier.set_object(page_info.page_ref.obj_num, PdfObject::Dict(page_dict));
    }

    // Remove AcroForm from catalog
    let catalog_ref = modifier.catalog_ref();
    if let Some(PdfObject::Dict(mut catalog_dict)) = object_at(modifier, doc, &catalog_ref) {
        catalog_dict.remove(b"AcroForm");
        modifier.set_object(catalog_ref.obj_num, PdfObject::Dict(catalog_dict));
    }

    Ok(())
}

/// The object `r` names: `modifier`'s, else `doc`'s.
fn object_at(modifier: &DocumentModifier, doc: &PdfDocument, r: &IndirectRef) -> Option<PdfObject> {
    modifier
        .find_object_pub(r.obj_num)
        .cloned()
        .or_else(|| doc.resolve(r).ok())
}

/// `obj`, resolved through [`object_at`] when it is a reference.
fn direct(modifier: &DocumentModifier, doc: &PdfDocument, obj: &PdfObject) -> Option<PdfObject> {
    match obj {
        PdfObject::Reference(r) => object_at(modifier, doc, r),
        other => Some(other.clone()),
    }
}

/// The resources that apply to the page: its own `/Resources`, else the
/// inherited ones, as a dictionary (a copy when it is a shared indirect
/// object).
fn effective_resources(
    modifier: &DocumentModifier,
    doc: &PdfDocument,
    page: &PdfDict,
    page_info: &crate::page::PageInfo,
) -> PdfDict {
    page.get(b"Resources")
        .or(page_info.resources_ref.as_ref())
        .and_then(|r| direct(modifier, doc, r))
        .and_then(|r| r.as_dict().cloned())
        .unwrap_or_default()
}

/// The XObject names `resources` already uses.
fn xobject_names(
    modifier: &DocumentModifier,
    doc: &PdfDocument,
    resources: &PdfDict,
) -> Vec<Vec<u8>> {
    resources
        .get(b"XObject")
        .and_then(|x| direct(modifier, doc, x))
        .and_then(|x| {
            x.as_dict()
                .map(|d| d.iter().map(|(k, _)| k.clone()).collect())
        })
        .unwrap_or_default()
}

/// The first `FlatN` not in `taken`, added to it.
fn free_name(taken: &mut Vec<Vec<u8>>) -> Vec<u8> {
    let name = (0..)
        .map(|n| format!("Flat{n}").into_bytes())
        .find(|n| !taken.contains(n))
        .unwrap_or_default();
    taken.push(name.clone());
    name
}

/// `resources` with an `/XObject` dictionary holding its existing entries and
/// `added`.
fn with_xobjects(
    modifier: &DocumentModifier,
    doc: &PdfDocument,
    mut resources: PdfDict,
    added: Vec<(Vec<u8>, IndirectRef)>,
) -> PdfDict {
    let mut xobjects = resources
        .get(b"XObject")
        .and_then(|x| direct(modifier, doc, x))
        .and_then(|x| x.as_dict().cloned())
        .unwrap_or_default();
    for (name, r) in added {
        xobjects.insert(name, PdfObject::Reference(r));
    }
    resources.insert(b"XObject".to_vec(), PdfObject::Dict(xobjects));
    resources
}

/// Wrap `page`'s existing content in `q … Q` and append `drawn` after it.
fn add_drawn_content(modifier: &mut DocumentModifier, page: &mut PdfDict, drawn: &str) {
    let mut stream = |data: &[u8]| {
        let (dict, data) = make_stream(data, true);
        PdfObject::Reference(modifier.add_object(PdfObject::Stream { dict, data }))
    };
    let existing = match page.remove(b"Contents") {
        Some(PdfObject::Array(arr)) => arr,
        Some(other) => vec![other],
        None => Vec::new(),
    };
    let mut contents = vec![stream(b"q\n")];
    contents.extend(existing);
    contents.push(stream(format!("Q\n{drawn}").as_bytes()));
    page.insert(b"Contents".to_vec(), PdfObject::Array(contents));
}
