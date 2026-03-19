//! Document modification: load existing PDF, modify, and save.
//! Also provides page merge/split operations.

use std::collections::HashMap;
use std::path::Path;

use crate::error::Result;
use crate::object::{IndirectRef, PdfDict, PdfObject};
use crate::page::{collect_pages, PageInfo};
use crate::parser::PdfDocument;
use crate::writer::page::PageBuilder;
use crate::writer::serialize::serialize_pdf;
use crate::writer::PdfWriter;

/// Modifier for existing PDF documents.
/// Loads all objects from a PdfDocument, allows modification, then saves.
pub struct DocumentModifier {
    writer: PdfWriter,
    catalog_ref: IndirectRef,
    info_ref: Option<IndirectRef>,
}

impl DocumentModifier {
    /// Create a modifier from an existing PdfDocument.
    /// Copies all objects from the document into the writer.
    pub fn from_document(doc: &PdfDocument) -> Result<Self> {
        let mut writer = PdfWriter::new();
        writer.version = doc.version;

        // Find catalog reference
        let catalog_ref = doc
            .catalog_ref()
            .cloned()
            .unwrap_or(IndirectRef {
                obj_num: 1,
                gen_num: 0,
            });

        // Find info reference from trailer
        let info_ref = doc
            .trailer()
            .get_ref(b"Info")
            .cloned();

        // Copy all objects
        let mut max_obj = 0u32;
        let refs: Vec<IndirectRef> = doc.object_refs().collect();
        for iref in &refs {
            if let Ok(obj) = doc.resolve(iref) {
                writer.objects.push((iref.obj_num, obj));
                max_obj = max_obj.max(iref.obj_num);
            }
        }
        writer.next_obj_num = max_obj + 1;

        Ok(Self {
            writer,
            catalog_ref,
            info_ref,
        })
    }

    /// Get a reference to the internal writer for low-level modifications.
    pub fn writer(&mut self) -> &mut PdfWriter {
        &mut self.writer
    }

    /// Get the catalog reference.
    pub fn catalog_ref(&self) -> &IndirectRef {
        &self.catalog_ref
    }

    /// Replace an object at a given object number.
    pub fn set_object(&mut self, obj_num: u32, obj: PdfObject) {
        self.writer.set_object(obj_num, obj);
    }

    /// Add a new object and return its reference.
    pub fn add_object(&mut self, obj: PdfObject) -> IndirectRef {
        self.writer.add_object(obj)
    }

    /// Find an object by object number (public accessor).
    pub fn find_object_pub(&self, obj_num: u32) -> Option<&PdfObject> {
        self.find_object(obj_num)
    }

    /// Delete a page by index (0-based).
    /// Modifies the Pages tree to remove the page reference.
    pub fn delete_page(&mut self, page_index: usize) -> Result<()> {
        let pages_ref = self.find_pages_ref()?;
        let pages_obj_num = pages_ref.obj_num;

        // Find the Pages dict
        let pages_obj = self.find_object(pages_obj_num)
            .cloned()
            .unwrap_or(PdfObject::Null);

        if let PdfObject::Dict(mut pages_dict) = pages_obj {
            if let Some(PdfObject::Array(mut kids)) = pages_dict.remove(b"Kids") {
                if page_index < kids.len() {
                    kids.remove(page_index);
                    let count = kids.len() as i64;
                    pages_dict.insert(b"Kids".to_vec(), PdfObject::Array(kids));
                    pages_dict.insert(b"Count".to_vec(), PdfObject::Integer(count));
                    self.writer.set_object(pages_obj_num, PdfObject::Dict(pages_dict));
                }
            }
        }

        Ok(())
    }

    /// Insert a new page at the given index.
    pub fn insert_page(&mut self, page_index: usize, page: PageBuilder) -> Result<()> {
        let pages_ref = self.find_pages_ref()?;
        let pages_obj_num = pages_ref.obj_num;

        let page_ref = page.build(&mut self.writer, &pages_ref);

        let pages_obj = self.find_object(pages_obj_num)
            .cloned()
            .unwrap_or(PdfObject::Null);

        if let PdfObject::Dict(mut pages_dict) = pages_obj {
            if let Some(PdfObject::Array(mut kids)) = pages_dict.remove(b"Kids") {
                let idx = page_index.min(kids.len());
                kids.insert(idx, PdfObject::Reference(page_ref));
                let count = kids.len() as i64;
                pages_dict.insert(b"Kids".to_vec(), PdfObject::Array(kids));
                pages_dict.insert(b"Count".to_vec(), PdfObject::Integer(count));
                self.writer.set_object(pages_obj_num, PdfObject::Dict(pages_dict));
            }
        }

        Ok(())
    }

    /// Reorder pages. `order` is a list of 0-based page indices in the desired order.
    pub fn reorder_pages(&mut self, order: &[usize]) -> Result<()> {
        let pages_ref = self.find_pages_ref()?;
        let pages_obj_num = pages_ref.obj_num;

        let pages_obj = self.find_object(pages_obj_num)
            .cloned()
            .unwrap_or(PdfObject::Null);

        if let PdfObject::Dict(mut pages_dict) = pages_obj {
            if let Some(PdfObject::Array(kids)) = pages_dict.remove(b"Kids") {
                let mut new_kids = Vec::with_capacity(order.len());
                for &idx in order {
                    if idx < kids.len() {
                        new_kids.push(kids[idx].clone());
                    }
                }
                let count = new_kids.len() as i64;
                pages_dict.insert(b"Kids".to_vec(), PdfObject::Array(new_kids));
                pages_dict.insert(b"Count".to_vec(), PdfObject::Integer(count));
                self.writer.set_object(pages_obj_num, PdfObject::Dict(pages_dict));
            }
        }

        Ok(())
    }

    /// Set or update a metadata field in the Info dictionary.
    pub fn set_info(&mut self, key: &[u8], value: &str) {
        let info_num = if let Some(ref r) = self.info_ref {
            r.obj_num
        } else {
            let num = self.writer.alloc_object_num();
            self.info_ref = Some(IndirectRef {
                obj_num: num,
                gen_num: 0,
            });
            num
        };

        // Get or create info dict
        let info_obj = self.find_object(info_num)
            .cloned()
            .unwrap_or(PdfObject::Dict(PdfDict::new()));

        if let PdfObject::Dict(mut info_dict) = info_obj {
            info_dict.insert(
                key.to_vec(),
                PdfObject::String(value.as_bytes().to_vec()),
            );
            self.writer.set_object(info_num, PdfObject::Dict(info_dict));
        }
    }

    /// Perform garbage collection: remove unreachable objects.
    ///
    /// Traverses all objects reachable from the catalog (and info dict),
    /// then removes any objects that are not reachable.
    pub fn garbage_collect(&mut self) {
        let mut reachable = std::collections::HashSet::new();

        // Mark catalog and info as roots
        reachable.insert(self.catalog_ref.obj_num);
        if let Some(ref info) = self.info_ref {
            reachable.insert(info.obj_num);
        }

        // Iteratively mark all reachable objects
        let mut work: Vec<u32> = reachable.iter().copied().collect();
        while let Some(obj_num) = work.pop() {
            if let Some(obj) = self.find_object(obj_num).cloned() {
                let refs = collect_references(&obj);
                for r in refs {
                    if reachable.insert(r) {
                        work.push(r);
                    }
                }
            }
        }

        // Remove unreachable objects
        self.writer.objects.retain(|(num, _)| reachable.contains(num));
    }

    /// Serialize to PDF bytes.
    pub fn build(self) -> Result<Vec<u8>> {
        serialize_pdf(
            &self.writer.objects,
            self.writer.version,
            &self.catalog_ref,
            self.info_ref.as_ref(),
        )
    }

    /// Save to file.
    pub fn save(self, path: &Path) -> Result<()> {
        let bytes = self.build()?;
        std::fs::write(path, bytes)?;
        Ok(())
    }

    // --- helpers ---

    fn find_pages_ref(&self) -> Result<IndirectRef> {
        // Look up Catalog → /Pages
        if let Some(PdfObject::Dict(catalog)) = self.find_object(self.catalog_ref.obj_num) {
            if let Some(PdfObject::Reference(r)) = catalog.get(b"Pages") {
                return Ok(r.clone());
            }
        }
        // Fallback: guess object 2
        Ok(IndirectRef {
            obj_num: 2,
            gen_num: 0,
        })
    }

    fn find_object(&self, obj_num: u32) -> Option<&PdfObject> {
        self.writer
            .objects
            .iter()
            .find(|(n, _)| *n == obj_num)
            .map(|(_, o)| o)
    }
}

/// Collect all indirect reference object numbers from a PdfObject recursively.
fn collect_references(obj: &PdfObject) -> Vec<u32> {
    let mut refs = Vec::new();
    collect_references_inner(obj, &mut refs);
    refs
}

fn collect_references_inner(obj: &PdfObject, refs: &mut Vec<u32>) {
    match obj {
        PdfObject::Reference(r) => {
            refs.push(r.obj_num);
        }
        PdfObject::Dict(d) => {
            for (_, val) in d.iter() {
                collect_references_inner(val, refs);
            }
        }
        PdfObject::Array(arr) => {
            for item in arr {
                collect_references_inner(item, refs);
            }
        }
        PdfObject::Stream { dict, .. } => {
            for (_, val) in dict.iter() {
                collect_references_inner(val, refs);
            }
        }
        _ => {}
    }
}

/// Perform an incremental save: append modified objects to the original PDF data.
///
/// This preserves the original bytes and appends only modified/new objects,
/// a new xref table, and a new trailer with /Prev pointing to the old xref.
pub fn incremental_save(original_data: &[u8], modifier: DocumentModifier) -> Result<Vec<u8>> {
    use std::io::Write;

    // Find old startxref
    let old_startxref = crate::xref::find_startxref(original_data)?;

    let mut buf = original_data.to_vec();

    // Determine max object number for xref size
    let max_obj_num = modifier
        .writer
        .objects
        .iter()
        .map(|(n, _)| *n)
        .max()
        .unwrap_or(0);
    let xref_size = max_obj_num + 1;

    // Write each object and track offsets
    let mut offsets: Vec<(u32, usize)> = Vec::new();
    for (obj_num, obj) in &modifier.writer.objects {
        let offset = buf.len();
        offsets.push((*obj_num, offset));
        write!(buf, "{} 0 obj\n", obj_num)?;
        // Use the serialize module's logic inline
        write!(buf, "{}", obj)?;
        write!(buf, "\nendobj\n")?;
    }

    // Write new xref table
    let new_xref_offset = buf.len();
    write!(buf, "xref\n")?;

    // Write subsections for each modified object
    // Sort offsets by object number
    let mut sorted_offsets = offsets.clone();
    sorted_offsets.sort_by_key(|(n, _)| *n);

    // Write as individual subsections
    for (obj_num, offset) in &sorted_offsets {
        write!(buf, "{} 1\n", obj_num)?;
        write!(buf, "{:010} {:05} n \r\n", offset, 0)?;
    }

    // Write trailer
    let mut trailer = PdfDict::new();
    trailer.insert(b"Size".to_vec(), PdfObject::Integer(xref_size as i64));
    trailer.insert(
        b"Root".to_vec(),
        PdfObject::Reference(modifier.catalog_ref.clone()),
    );
    if let Some(ref info) = modifier.info_ref {
        trailer.insert(b"Info".to_vec(), PdfObject::Reference(info.clone()));
    }
    trailer.insert(
        b"Prev".to_vec(),
        PdfObject::Integer(old_startxref as i64),
    );

    write!(buf, "trailer\n")?;
    write!(buf, "{}", PdfObject::Dict(trailer))?;
    write!(buf, "\n")?;

    write!(buf, "startxref\n{}\n%%EOF\n", new_xref_offset)?;

    Ok(buf)
}

/// Merge pages from multiple PDF documents into one.
///
/// Returns the merged PDF as bytes. Pages are concatenated in order:
/// all pages from doc1, then all from doc2, etc.
pub fn merge_documents(docs: &[&PdfDocument]) -> Result<Vec<u8>> {
    let mut writer = PdfWriter::new();
    let pages_obj_num = writer.alloc_object_num();
    let pages_ref = IndirectRef {
        obj_num: pages_obj_num,
        gen_num: 0,
    };

    let mut all_page_refs: Vec<IndirectRef> = Vec::new();

    for doc in docs.iter() {
        let pages = collect_pages(*doc)?;
        for page_info in &pages {
            let page_ref = graft_page(&mut writer, *doc, page_info, &pages_ref)?;
            all_page_refs.push(page_ref);
        }
    }

    // Create Pages dict
    let kids: Vec<PdfObject> = all_page_refs
        .iter()
        .map(|r| PdfObject::Reference(r.clone()))
        .collect();
    let count = kids.len() as i64;

    let mut pages_dict = PdfDict::new();
    pages_dict.insert(b"Type".to_vec(), PdfObject::Name(b"Pages".to_vec()));
    pages_dict.insert(b"Kids".to_vec(), PdfObject::Array(kids));
    pages_dict.insert(b"Count".to_vec(), PdfObject::Integer(count));
    writer.set_object(pages_obj_num, PdfObject::Dict(pages_dict));

    // Create Catalog
    let mut catalog_dict = PdfDict::new();
    catalog_dict.insert(b"Type".to_vec(), PdfObject::Name(b"Catalog".to_vec()));
    catalog_dict.insert(b"Pages".to_vec(), PdfObject::Reference(pages_ref));
    let catalog_ref = writer.add_object(PdfObject::Dict(catalog_dict));

    serialize_pdf(&writer.objects, (1, 7), &catalog_ref, None)
}

/// Graft a single page from a source document into the writer.
/// Copies the page dict and all referenced objects with remapped object numbers.
fn graft_page(
    writer: &mut PdfWriter,
    doc: &PdfDocument,
    page_info: &PageInfo,
    new_pages_ref: &IndirectRef,
) -> Result<IndirectRef> {
    let mut remap: HashMap<u32, u32> = HashMap::new();

    // Resolve the page object
    let page_obj = doc.resolve(&page_info.page_ref)?;

    // Deep-copy the page and all referenced objects
    let new_page_obj = deep_copy_object(writer, doc, &page_obj, &mut remap)?;

    // Update Parent reference to point to our new Pages
    if let PdfObject::Dict(mut page_dict) = new_page_obj {
        page_dict.insert(
            b"Parent".to_vec(),
            PdfObject::Reference(new_pages_ref.clone()),
        );
        Ok(writer.add_object(PdfObject::Dict(page_dict)))
    } else {
        Ok(writer.add_object(new_page_obj))
    }
}

/// Deep-copy a PdfObject, resolving all references and remapping object numbers.
fn deep_copy_object(
    writer: &mut PdfWriter,
    doc: &PdfDocument,
    obj: &PdfObject,
    remap: &mut HashMap<u32, u32>,
) -> Result<PdfObject> {
    match obj {
        PdfObject::Reference(r) => {
            // Check if already remapped
            if let Some(&new_num) = remap.get(&r.obj_num) {
                return Ok(PdfObject::Reference(IndirectRef {
                    obj_num: new_num,
                    gen_num: 0,
                }));
            }

            // Allocate new number first (for circular reference prevention)
            let new_num = writer.alloc_object_num();
            remap.insert(r.obj_num, new_num);

            // Resolve and deep-copy
            let resolved = doc.resolve(r)?;
            let copied = deep_copy_object(writer, doc, &resolved, remap)?;
            writer.set_object(new_num, copied);

            Ok(PdfObject::Reference(IndirectRef {
                obj_num: new_num,
                gen_num: 0,
            }))
        }
        PdfObject::Dict(d) => {
            let mut new_dict = PdfDict::new();
            for (key, val) in d.iter() {
                let new_val = deep_copy_object(writer, doc, val, remap)?;
                new_dict.insert(key.clone(), new_val);
            }
            Ok(PdfObject::Dict(new_dict))
        }
        PdfObject::Array(arr) => {
            let mut new_arr = Vec::with_capacity(arr.len());
            for item in arr {
                new_arr.push(deep_copy_object(writer, doc, item, remap)?);
            }
            Ok(PdfObject::Array(new_arr))
        }
        PdfObject::Stream { dict, data } => {
            let mut new_dict = PdfDict::new();
            for (key, val) in dict.iter() {
                let new_val = deep_copy_object(writer, doc, val, remap)?;
                new_dict.insert(key.clone(), new_val);
            }
            Ok(PdfObject::Stream {
                dict: new_dict,
                data: data.clone(),
            })
        }
        // Primitive types: just clone
        other => Ok(other.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::writer::document::DocumentBuilder;
    use crate::writer::page::PageBuilder;

    fn create_test_pdf(text: &str, num_pages: usize) -> Vec<u8> {
        let mut doc = DocumentBuilder::new();
        let font = doc.add_standard_font("Helvetica");

        for i in 0..num_pages {
            let mut page = PageBuilder::new(612.0, 792.0);
            page.add_font(&font, "Helvetica");
            page.begin_text();
            page.set_font(&font, 12.0);
            page.move_to(72.0, 720.0);
            page.show_text(&format!("{} - Page {}", text, i + 1));
            page.end_text();
            doc.add_page(page);
        }

        doc.build().unwrap()
    }

    #[test]
    fn test_modifier_roundtrip() {
        let bytes = create_test_pdf("Hello", 2);
        let mut doc = PdfDocument::from_bytes(bytes).unwrap();

        let modifier = DocumentModifier::from_document(&doc).unwrap();
        let new_bytes = modifier.build().unwrap();

        let mut reparsed = PdfDocument::from_bytes(new_bytes).unwrap();
        let pages = collect_pages(&reparsed).unwrap();
        assert_eq!(pages.len(), 2);
    }

    #[test]
    fn test_delete_page() {
        let bytes = create_test_pdf("Test", 3);
        let mut doc = PdfDocument::from_bytes(bytes).unwrap();

        let mut modifier = DocumentModifier::from_document(&doc).unwrap();
        modifier.delete_page(1).unwrap(); // remove middle page

        let new_bytes = modifier.build().unwrap();
        let mut reparsed = PdfDocument::from_bytes(new_bytes).unwrap();
        let pages = collect_pages(&reparsed).unwrap();
        assert_eq!(pages.len(), 2);
    }

    #[test]
    fn test_reorder_pages() {
        let bytes = create_test_pdf("Reorder", 3);
        let mut doc = PdfDocument::from_bytes(bytes).unwrap();

        let mut modifier = DocumentModifier::from_document(&doc).unwrap();
        modifier.reorder_pages(&[2, 0, 1]).unwrap(); // reverse-ish

        let new_bytes = modifier.build().unwrap();
        let mut reparsed = PdfDocument::from_bytes(new_bytes).unwrap();
        let pages = collect_pages(&reparsed).unwrap();
        assert_eq!(pages.len(), 3);
    }

    #[test]
    fn test_set_info() {
        let bytes = create_test_pdf("Info", 1);
        let mut doc = PdfDocument::from_bytes(bytes).unwrap();

        let mut modifier = DocumentModifier::from_document(&doc).unwrap();
        modifier.set_info(b"Title", "New Title");
        modifier.set_info(b"Author", "New Author");

        let new_bytes = modifier.build().unwrap();
        let text = String::from_utf8_lossy(&new_bytes);
        assert!(text.contains("New Title"));
        assert!(text.contains("New Author"));
    }

    #[test]
    fn test_merge_documents() {
        let bytes1 = create_test_pdf("Doc1", 2);
        let bytes2 = create_test_pdf("Doc2", 3);

        let mut doc1 = PdfDocument::from_bytes(bytes1).unwrap();
        let mut doc2 = PdfDocument::from_bytes(bytes2).unwrap();

        let merged = merge_documents(&[&doc1, &doc2]).unwrap();

        let mut reparsed = PdfDocument::from_bytes(merged).unwrap();
        let pages = collect_pages(&reparsed).unwrap();
        assert_eq!(pages.len(), 5); // 2 + 3
    }

    #[test]
    fn test_incremental_save() {
        let original = create_test_pdf("Original", 1);
        let original_len = original.len();

        let mut doc = PdfDocument::from_bytes(original.clone()).unwrap();
        let mut modifier = DocumentModifier::from_document(&doc).unwrap();
        modifier.set_info(b"Title", "Updated Title");

        let result = incremental_save(&original, modifier).unwrap();

        // The result should start with the original bytes
        assert!(result.len() > original_len);
        assert_eq!(&result[..original_len], &original[..]);

        // Should contain the new title
        let text = String::from_utf8_lossy(&result);
        assert!(text.contains("Updated Title"));

        // Should contain /Prev
        assert!(text.contains("/Prev"));

        // Should end with %%EOF
        let tail = String::from_utf8_lossy(&result[result.len().saturating_sub(50)..]);
        assert!(tail.contains("%%EOF"));
    }

    #[test]
    fn test_garbage_collect() {
        let bytes = create_test_pdf("GC Test", 1);
        let mut doc = PdfDocument::from_bytes(bytes).unwrap();
        let mut modifier = DocumentModifier::from_document(&doc).unwrap();

        // Run GC first to establish baseline (some objects from parsing may be unreachable)
        modifier.garbage_collect();
        let count_baseline = modifier.writer.objects.len();

        // Add unreachable (orphan) objects
        modifier.add_object(PdfObject::Integer(999));
        modifier.add_object(PdfObject::String(b"orphan".to_vec()));
        let count_with_orphans = modifier.writer.objects.len();
        assert_eq!(count_with_orphans, count_baseline + 2);

        // Run GC again
        modifier.garbage_collect();
        let count_after = modifier.writer.objects.len();

        // The orphan objects should be removed, back to baseline
        assert_eq!(count_after, count_baseline);
    }

    #[test]
    fn test_resource_conflict_merge() {
        // Create two docs that both use "F1" as font resource name.
        // The deep_copy approach assigns new object numbers, so each page
        // keeps its own independent Resources dict. No conflict occurs.
        let bytes1 = create_test_pdf("Doc1", 1);
        let bytes2 = create_test_pdf("Doc2", 1);

        let mut doc1 = PdfDocument::from_bytes(bytes1).unwrap();
        let mut doc2 = PdfDocument::from_bytes(bytes2).unwrap();

        let merged = merge_documents(&[&doc1, &doc2]).unwrap();

        let mut reparsed = PdfDocument::from_bytes(merged).unwrap();
        let pages = collect_pages(&reparsed).unwrap();
        assert_eq!(pages.len(), 2);

        // Both pages should be independently valid (each has its own Resources)
        // Verify the merged PDF is parseable
        assert!(reparsed.catalog_ref().is_some());
    }
}
