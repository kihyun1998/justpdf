//! Optional content visibility for the content stream interpreters: the
//! `/OC` operand of `BDC` and the `/OC` entry of an XObject or annotation,
//! evaluated by `ocg::is_oc_visible` under the default configuration.

use justpdf_core::PdfDocument;
use justpdf_core::content::Operand;
use justpdf_core::object::{PdfDict, PdfObject};
use justpdf_core::ocg::{self, OCConfig};

use crate::resources::ResourceScopes;

/// The document's default optional content configuration, if any.
pub(crate) fn default_config(doc: &PdfDocument) -> Option<OCConfig> {
    ocg::read_oc_properties(doc)
        .ok()
        .flatten()
        .and_then(|props| props.default_config)
}

/// Whether the marked-content section `BDC` opens with `operands` is visible:
/// a section tagged other than `/OC` always is. Its properties operand is an
/// inline dictionary, or a name looked up in `resources`' `/Properties`; a
/// name that does not resolve is visible.
pub(crate) fn section_visible(
    operands: &[Operand],
    resources: &ResourceScopes,
    doc: &PdfDocument,
    config: &OCConfig,
) -> bool {
    if operands.first().and_then(|o| o.as_name()) != Some(b"OC") {
        return true;
    }
    let oc = match operands.get(1) {
        Some(Operand::Name(name)) => match resources.lookup(doc, b"Properties", name) {
            Ok(Some(found)) => match found.obj_ref {
                Some(r) => PdfObject::Reference(r),
                None => found.object,
            },
            _ => return true,
        },
        Some(op @ Operand::Dict(_)) => operand_to_pdf_object(op),
        _ => return true,
    };
    ocg::is_oc_visible(&oc, doc, config)
}

/// Whether an object with the `/OC` entry of `dict` is visible: always when
/// there is no entry or no configuration.
pub(crate) fn entry_visible(dict: &PdfDict, doc: &PdfDocument, config: Option<&OCConfig>) -> bool {
    match (dict.get(b"OC"), config) {
        (Some(oc), Some(config)) => ocg::is_oc_visible(oc, doc, config),
        _ => true,
    }
}

fn operand_to_pdf_object(op: &Operand) -> PdfObject {
    match op {
        Operand::Integer(v) => PdfObject::Integer(*v),
        Operand::Real(v) => PdfObject::Real(*v),
        Operand::Bool(v) => PdfObject::Bool(*v),
        Operand::Null => PdfObject::Null,
        Operand::Name(n) => PdfObject::Name(n.clone()),
        Operand::String(s) => PdfObject::String(s.clone()),
        Operand::Array(arr) => PdfObject::Array(arr.iter().map(operand_to_pdf_object).collect()),
        Operand::Dict(entries) => {
            let mut dict = PdfDict::new();
            for (k, v) in entries {
                dict.insert(k.clone(), operand_to_pdf_object(v));
            }
            PdfObject::Dict(dict)
        }
        Operand::InlineImage { .. } => PdfObject::Null,
    }
}
