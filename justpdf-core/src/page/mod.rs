use crate::error::{JustPdfError, Result};
use crate::object::{IndirectRef, PdfDict, PdfObject};
use crate::parser::PdfDocument;

/// A rectangle defined by [llx, lly, urx, ury].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub llx: f64,
    pub lly: f64,
    pub urx: f64,
    pub ury: f64,
}

impl Rect {
    pub fn width(&self) -> f64 {
        (self.urx - self.llx).abs()
    }

    pub fn height(&self) -> f64 {
        (self.ury - self.lly).abs()
    }

    /// Parse a Rect from a PDF array [llx, lly, urx, ury].
    pub fn from_pdf_array(arr: &[PdfObject]) -> Option<Self> {
        if arr.len() < 4 {
            return None;
        }
        Some(Self {
            llx: arr[0].as_f64()?,
            lly: arr[1].as_f64()?,
            urx: arr[2].as_f64()?,
            ury: arr[3].as_f64()?,
        })
    }
}

impl std::fmt::Display for Rect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{} {} {} {}]", self.llx, self.lly, self.urx, self.ury)
    }
}

/// Information about a single PDF page.
#[derive(Debug, Clone)]
pub struct PageInfo {
    /// 0-based page index.
    pub index: usize,
    /// The indirect reference to this page object.
    pub page_ref: IndirectRef,
    /// MediaBox (required, possibly inherited).
    pub media_box: Rect,
    /// CropBox (optional, defaults to MediaBox).
    pub crop_box: Option<Rect>,
    /// BleedBox (optional).
    pub bleed_box: Option<Rect>,
    /// TrimBox (optional).
    pub trim_box: Option<Rect>,
    /// ArtBox (optional).
    pub art_box: Option<Rect>,
    /// Page rotation in degrees (0, 90, 180, 270).
    pub rotate: i64,
    /// Reference to the Contents (stream or array of streams).
    pub contents_ref: Option<PdfObject>,
    /// Reference to the Resources dict.
    pub resources_ref: Option<PdfObject>,
}

/// Walk the page tree and collect all pages in order.
pub fn collect_pages(doc: &mut PdfDocument) -> Result<Vec<PageInfo>> {
    let catalog_ref = doc
        .catalog_ref()
        .ok_or(JustPdfError::TrailerNotFound)?
        .clone();

    let catalog = doc.resolve(&catalog_ref)?.clone();
    let catalog_dict = catalog.as_dict().ok_or(JustPdfError::InvalidObject {
        offset: 0,
        detail: "catalog is not a dict".into(),
    })?;

    let pages_ref = catalog_dict
        .get_ref(b"Pages")
        .ok_or(JustPdfError::InvalidObject {
            offset: 0,
            detail: "catalog has no /Pages".into(),
        })?
        .clone();

    let mut pages = Vec::new();
    let inherited = InheritedAttrs::default();
    walk_page_tree(doc, &pages_ref, &inherited, &mut pages)?;
    Ok(pages)
}

/// Get the total page count from the Pages dict /Count.
pub fn page_count(doc: &mut PdfDocument) -> Result<usize> {
    let catalog_ref = doc
        .catalog_ref()
        .ok_or(JustPdfError::TrailerNotFound)?
        .clone();

    let catalog = doc.resolve(&catalog_ref)?.clone();
    let catalog_dict = catalog.as_dict().ok_or(JustPdfError::InvalidObject {
        offset: 0,
        detail: "catalog is not a dict".into(),
    })?;

    let pages_ref = catalog_dict
        .get_ref(b"Pages")
        .ok_or(JustPdfError::InvalidObject {
            offset: 0,
            detail: "catalog has no /Pages".into(),
        })?
        .clone();

    let pages_obj = doc.resolve(&pages_ref)?.clone();
    let pages_dict = pages_obj.as_dict().ok_or(JustPdfError::InvalidObject {
        offset: 0,
        detail: "Pages is not a dict".into(),
    })?;

    Ok(pages_dict.get_i64(b"Count").unwrap_or(0) as usize)
}

/// Get a single page by 0-based index without collecting all pages.
///
/// This walks the page tree, counting pages as it goes, and returns the
/// `PageInfo` for the requested page as soon as it is found.  For documents
/// with many pages this avoids allocating and resolving every page object when
/// only a single page is needed.
pub fn get_page(doc: &mut PdfDocument, index: usize) -> Result<PageInfo> {
    let catalog_ref = doc
        .catalog_ref()
        .ok_or(JustPdfError::TrailerNotFound)?
        .clone();

    let catalog = doc.resolve(&catalog_ref)?.clone();
    let catalog_dict = catalog.as_dict().ok_or(JustPdfError::InvalidObject {
        offset: 0,
        detail: "catalog is not a dict".into(),
    })?;

    let pages_ref = catalog_dict
        .get_ref(b"Pages")
        .ok_or(JustPdfError::InvalidObject {
            offset: 0,
            detail: "catalog has no /Pages".into(),
        })?
        .clone();

    // Optional: fast-path bounds check via /Count.
    let pages_obj = doc.resolve(&pages_ref)?.clone();
    let pages_dict = pages_obj.as_dict().ok_or(JustPdfError::InvalidObject {
        offset: 0,
        detail: "Pages is not a dict".into(),
    })?;
    let count = pages_dict.get_i64(b"Count").unwrap_or(0) as usize;
    if index >= count {
        return Err(JustPdfError::InvalidObject {
            offset: 0,
            detail: format!(
                "page index {index} out of range (document has {count} pages)"
            ),
        });
    }

    let inherited = InheritedAttrs::default();
    let mut counter: usize = 0;
    walk_page_tree_find(doc, &pages_ref, &inherited, index, &mut counter)
        .and_then(|opt| {
            opt.ok_or(JustPdfError::InvalidObject {
                offset: 0,
                detail: format!("page index {index} not found in page tree"),
            })
        })
}

/// Recursively walk the page tree looking for the page at `target` index.
/// `counter` tracks how many leaf pages have been seen so far.
/// Returns `Ok(Some(page))` as soon as the target page is found, or
/// `Ok(None)` after exhausting the subtree without finding it.
fn walk_page_tree_find(
    doc: &mut PdfDocument,
    node_ref: &IndirectRef,
    inherited: &InheritedAttrs,
    target: usize,
    counter: &mut usize,
) -> Result<Option<PageInfo>> {
    let node_obj = doc.resolve(node_ref)?.clone();
    let dict = node_obj.as_dict().ok_or(JustPdfError::InvalidObject {
        offset: 0,
        detail: "page tree node is not a dict".into(),
    })?;

    let node_type = dict.get_name(b"Type").unwrap_or(b"");

    match node_type {
        b"Pages" => {
            // Pruning: if this subtree's /Count means the target lies beyond
            // it, skip the entire subtree.
            let subtree_count = dict.get_i64(b"Count").unwrap_or(0) as usize;
            if *counter + subtree_count <= target {
                *counter += subtree_count;
                return Ok(None);
            }

            let updated = inherited.with_overrides(dict);
            if let Some(kids) = dict.get_array(b"Kids") {
                let kid_refs: Vec<IndirectRef> = kids
                    .iter()
                    .filter_map(|obj| obj.as_reference().cloned())
                    .collect();

                for kid_ref in kid_refs {
                    if let Some(page) =
                        walk_page_tree_find(doc, &kid_ref, &updated, target, counter)?
                    {
                        return Ok(Some(page));
                    }
                }
            }
            Ok(None)
        }
        _ if node_type == b"Page"
            || dict.contains_key(b"MediaBox")
            || inherited.media_box.is_some() =>
        {
            let current_index = *counter;
            *counter += 1;

            if current_index != target {
                return Ok(None);
            }

            let updated = inherited.with_overrides(dict);

            let media_box = updated.media_box.unwrap_or(Rect {
                llx: 0.0,
                lly: 0.0,
                urx: 612.0,
                ury: 792.0,
            });

            Ok(Some(PageInfo {
                index: current_index,
                page_ref: node_ref.clone(),
                media_box,
                crop_box: updated
                    .crop_box
                    .or_else(|| dict.get_array(b"CropBox").and_then(Rect::from_pdf_array)),
                bleed_box: dict.get_array(b"BleedBox").and_then(Rect::from_pdf_array),
                trim_box: dict.get_array(b"TrimBox").and_then(Rect::from_pdf_array),
                art_box: dict.get_array(b"ArtBox").and_then(Rect::from_pdf_array),
                rotate: updated.rotate.unwrap_or(0),
                contents_ref: dict.get(b"Contents").cloned(),
                resources_ref: updated
                    .resources
                    .or_else(|| dict.get(b"Resources").cloned()),
            }))
        }
        _ => Ok(None),
    }
}

/// Attributes that can be inherited from parent Pages nodes.
#[derive(Debug, Clone, Default)]
struct InheritedAttrs {
    media_box: Option<Rect>,
    crop_box: Option<Rect>,
    rotate: Option<i64>,
    resources: Option<PdfObject>,
}

impl InheritedAttrs {
    /// Create a child copy with overrides from a Pages/Page dict.
    fn with_overrides(&self, dict: &PdfDict) -> Self {
        let mut child = self.clone();

        if let Some(arr) = dict.get_array(b"MediaBox")
            && let Some(rect) = Rect::from_pdf_array(arr)
        {
            child.media_box = Some(rect);
        }
        if let Some(arr) = dict.get_array(b"CropBox")
            && let Some(rect) = Rect::from_pdf_array(arr)
        {
            child.crop_box = Some(rect);
        }
        if let Some(r) = dict.get_i64(b"Rotate") {
            child.rotate = Some(r);
        }
        if dict.get(b"Resources").is_some() {
            child.resources = dict.get(b"Resources").cloned();
        }

        child
    }
}

/// Recursively walk the page tree.
fn walk_page_tree(
    doc: &mut PdfDocument,
    node_ref: &IndirectRef,
    inherited: &InheritedAttrs,
    pages: &mut Vec<PageInfo>,
) -> Result<()> {
    let node_obj = doc.resolve(node_ref)?.clone();
    let dict = node_obj.as_dict().ok_or(JustPdfError::InvalidObject {
        offset: 0,
        detail: "page tree node is not a dict".into(),
    })?;

    let node_type = dict.get_name(b"Type").unwrap_or(b"");

    match node_type {
        b"Pages" => {
            let updated = inherited.with_overrides(dict);
            if let Some(kids) = dict.get_array(b"Kids") {
                let kid_refs: Vec<IndirectRef> = kids
                    .iter()
                    .filter_map(|obj| obj.as_reference().cloned())
                    .collect();

                for kid_ref in kid_refs {
                    walk_page_tree(doc, &kid_ref, &updated, pages)?;
                }
            }
        }
        _ if node_type == b"Page"
            || dict.contains_key(b"MediaBox")
            || inherited.media_box.is_some() =>
        {
            let updated = inherited.with_overrides(dict);

            let media_box = updated.media_box.unwrap_or(Rect {
                llx: 0.0,
                lly: 0.0,
                urx: 612.0,
                ury: 792.0,
            });

            let page_info = PageInfo {
                index: pages.len(),
                page_ref: node_ref.clone(),
                media_box,
                crop_box: updated
                    .crop_box
                    .or_else(|| dict.get_array(b"CropBox").and_then(Rect::from_pdf_array)),
                bleed_box: dict.get_array(b"BleedBox").and_then(Rect::from_pdf_array),
                trim_box: dict.get_array(b"TrimBox").and_then(Rect::from_pdf_array),
                art_box: dict.get_array(b"ArtBox").and_then(Rect::from_pdf_array),
                rotate: updated.rotate.unwrap_or(0),
                contents_ref: dict.get(b"Contents").cloned(),
                resources_ref: updated
                    .resources
                    .or_else(|| dict.get(b"Resources").cloned()),
            };

            pages.push(page_info);
        }
        _ => {
            // Unknown type, skip
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rect_from_array() {
        let arr = vec![
            PdfObject::Integer(0),
            PdfObject::Integer(0),
            PdfObject::Integer(612),
            PdfObject::Integer(792),
        ];
        let rect = Rect::from_pdf_array(&arr).unwrap();
        assert_eq!(rect.llx, 0.0);
        assert_eq!(rect.ury, 792.0);
        assert_eq!(rect.width(), 612.0);
        assert_eq!(rect.height(), 792.0);
    }

    #[test]
    fn test_rect_from_real_array() {
        let arr = vec![
            PdfObject::Real(10.5),
            PdfObject::Real(20.5),
            PdfObject::Real(595.0),
            PdfObject::Real(842.0),
        ];
        let rect = Rect::from_pdf_array(&arr).unwrap();
        assert_eq!(rect.llx, 10.5);
        assert_eq!(rect.width(), 584.5);
    }

    #[test]
    fn test_rect_too_short() {
        let arr = vec![PdfObject::Integer(0), PdfObject::Integer(0)];
        assert!(Rect::from_pdf_array(&arr).is_none());
    }

    #[test]
    fn test_rect_display() {
        let rect = Rect {
            llx: 0.0,
            lly: 0.0,
            urx: 612.0,
            ury: 792.0,
        };
        assert_eq!(rect.to_string(), "[0 0 612 792]");
    }
}
