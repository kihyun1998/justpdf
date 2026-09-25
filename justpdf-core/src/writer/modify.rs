//! Document modification: load existing PDF, modify, and save.
//! Also provides page merge/split operations.

use std::collections::{HashMap, HashSet};
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
    /// First element of the source trailer's `/ID`, empty when absent.
    source_file_id: Vec<u8>,
    /// Encryption applied by `build`, when set.
    encryption: Option<crate::crypto::EncryptionConfig>,
}

impl DocumentModifier {
    /// Create a modifier from an existing PdfDocument.
    /// Copies all objects from the document into the writer, except the
    /// trailer's `/Encrypt` dictionary. An encrypted document must be
    /// authenticated first (`JustPdfError::EncryptedDocument` otherwise).
    pub fn from_document(doc: &PdfDocument) -> Result<Self> {
        if doc.is_encrypted() && !doc.is_authenticated() {
            return Err(crate::error::JustPdfError::EncryptedDocument);
        }
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

        // Copy all objects; new numbers start above every number the source uses
        let max_obj = doc.object_refs().map(|r| r.obj_num).max().unwrap_or(0);
        writer.objects.extend(source_objects(doc));
        writer.next_obj_num = max_obj + 1;

        Ok(Self {
            writer,
            catalog_ref,
            info_ref,
            source_file_id: doc.extract_file_id(),
            encryption: None,
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

    /// Encrypt the document written by `build` with `config`.
    ///
    /// The trailer `/ID` keeps the source's first element as the permanent
    /// identifier and gets a new random changing identifier; a source without
    /// a usable `/ID` gets a new random identifier in both elements.
    pub fn set_encryption(&mut self, config: crate::crypto::EncryptionConfig) {
        self.encryption = Some(config);
    }

    /// Serialize to PDF bytes, encrypted when `set_encryption` was called.
    pub fn build(mut self) -> Result<Vec<u8>> {
        let Some(config) = self.encryption.take() else {
            return serialize_pdf(
                &self.writer.objects,
                self.writer.version,
                &self.catalog_ref,
                self.info_ref.as_ref(),
            );
        };
        let (permanent_id, changing_id) = if self.source_file_id.is_empty() {
            let id = crate::crypto::random_file_id()?;
            (id.clone(), id)
        } else {
            (
                std::mem::take(&mut self.source_file_id),
                crate::crypto::random_file_id()?,
            )
        };
        crate::writer::serialize::serialize_writer_encrypted(
            &mut self.writer,
            &self.catalog_ref,
            self.info_ref.as_ref(),
            &config,
            &permanent_id,
            &changing_id,
        )
    }

    /// Serialize to PDF bytes using xref streams (PDF 1.5+).
    /// `compressed` contains info about objects packed into object streams.
    /// Returns an error when `set_encryption` was called.
    pub fn build_with_xref_stream(
        self,
        compressed: &[crate::writer::object_stream::CompressedObjInfo],
    ) -> Result<Vec<u8>> {
        if self.encryption.is_some() {
            return Err(crate::error::JustPdfError::UnsupportedEncryption {
                detail: "encryption is not supported when writing xref streams".into(),
            });
        }
        crate::writer::serialize::serialize_pdf_with_xref_stream(
            &self.writer.objects,
            compressed,
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

/// The objects `from_document` copies: every in-use object of `doc` that
/// resolves, except the trailer's `/Encrypt` dictionary.
fn source_objects(doc: &PdfDocument) -> impl Iterator<Item = (u32, PdfObject)> + '_ {
    let encrypt_obj_num = doc.trailer().get_ref(b"Encrypt").map(|r| r.obj_num);
    doc.object_refs()
        .filter(move |r| Some(r.obj_num) != encrypt_obj_num)
        .filter_map(|r| doc.resolve(&r).ok().map(|obj| (r.obj_num, obj)))
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

/// Perform an incremental save of `doc`: append the objects the modifier changed
/// or added, mark the ones it removed as free, and follow them with a new xref
/// table and a trailer whose `/Prev` points at the original xref.
///
/// `modifier` must have been created from `doc`. An object is changed when its
/// value differs from `doc`'s. A removed object is one `from_document` copied
/// that the modifier no longer holds, other than an object stream that still
/// holds an unchanged object; its entry is `0000000000 65535 f`. With
/// nothing changed or removed, the original bytes are returned unchanged. The
/// trailer carries the original trailer's keys ([`incremental_trailer`]). When
/// `doc` is encrypted, the appended objects are encrypted with its file key, so
/// `doc` must be authenticated.
pub fn incremental_save(doc: &PdfDocument, modifier: DocumentModifier) -> Result<Vec<u8>> {
    use std::io::Write;

    if modifier.encryption.is_some() {
        return Err(crate::error::JustPdfError::UnsupportedEncryption {
            detail: "set_encryption applies to a full rewrite, not an incremental save".into(),
        });
    }

    let original_data = doc.raw_data();
    let old_startxref = crate::xref::find_startxref(original_data)?;
    let previous_trailer = crate::xref::load_xref(original_data)?.trailer;

    let security = match doc.security_state() {
        Some(state) if state.file_key.is_none() => {
            return Err(crate::error::JustPdfError::EncryptionError {
                detail: "incremental save of an encrypted document requires authentication".into(),
            });
        }
        other => other,
    };

    let current: HashMap<u32, &PdfObject> = modifier
        .writer
        .objects
        .iter()
        .map(|(num, obj)| (*num, obj))
        .collect();
    let mut unchanged: HashSet<u32> = HashSet::new();
    let mut removed: Vec<u32> = Vec::new();
    for (num, source_obj) in source_objects(doc) {
        match current.get(&num) {
            None => removed.push(num),
            Some(obj) if **obj == source_obj => {
                unchanged.insert(num);
            }
            Some(_) => {}
        }
    }
    let live_object_streams: HashSet<u32> = unchanged
        .iter()
        .filter_map(|num| match doc.xref.get(*num) {
            Some(crate::xref::XrefEntry::Compressed { obj_stream_num, .. }) => {
                Some(*obj_stream_num)
            }
            _ => None,
        })
        .collect();
    removed.retain(|num| !live_object_streams.contains(num));
    let changed: Vec<&(u32, PdfObject)> = modifier
        .writer
        .objects
        .iter()
        .filter(|(num, _)| !unchanged.contains(num))
        .collect();
    if changed.is_empty() && removed.is_empty() {
        return Ok(original_data.to_vec());
    }

    let mut buf = original_data.to_vec();
    if !buf.ends_with(b"\n") {
        buf.push(b'\n');
    }

    let mut offsets: Vec<(u32, usize)> = Vec::new();
    for (obj_num, obj) in changed {
        let write_obj = match security {
            Some(state) => crate::crypto::encrypt_object(obj, state, *obj_num, 0)?,
            None => obj.clone(),
        };
        offsets.push((*obj_num, buf.len()));
        write!(buf, "{} 0 obj\n", obj_num)?;
        crate::writer::serialize::serialize_object(&mut buf, &write_obj)?;
        write!(buf, "\nendobj\n")?;
    }

    let new_xref_offset = buf.len();
    write!(buf, "xref\n")?;
    let mut entries: Vec<(u32, Option<usize>)> = offsets
        .into_iter()
        .map(|(n, offset)| (n, Some(offset)))
        .chain(removed.into_iter().map(|n| (n, None)))
        .collect();
    entries.sort_by_key(|(n, _)| *n);
    for (obj_num, offset) in &entries {
        write!(buf, "{} 1\n", obj_num)?;
        match offset {
            Some(offset) => write!(buf, "{:010} {:05} n \r\n", offset, 0)?,
            None => write!(buf, "{:010} {:05} f \r\n", 0, 65535)?,
        }
    }

    let max_obj_num = entries.last().map(|(n, _)| *n).unwrap_or(0);
    let trailer = incremental_trailer(
        &previous_trailer,
        max_obj_num + 1,
        &modifier.catalog_ref,
        modifier.info_ref.as_ref(),
        old_startxref,
    );
    write!(buf, "trailer\n")?;
    crate::writer::serialize::serialize_dict(&mut buf, &trailer)?;
    write!(buf, "\nstartxref\n{}\n%%EOF\n", new_xref_offset)?;

    Ok(buf)
}

/// The trailer of an incremental update section: every key of `previous` (the
/// trailer being updated) except the cross-reference-stream keys and
/// `/XRefStm`, with `/Size` raised to at least `size`, `/Root` and — when given —
/// `/Info` replaced, and `/Prev` set to `prev_offset`.
pub(crate) fn incremental_trailer(
    previous: &PdfDict,
    size: u32,
    root: &IndirectRef,
    info: Option<&IndirectRef>,
    prev_offset: usize,
) -> PdfDict {
    const NOT_CARRIED: &[&[u8]] = &[
        b"Prev",
        b"XRefStm",
        b"Type",
        b"W",
        b"Index",
        b"Filter",
        b"DecodeParms",
        b"Length",
        b"F",
        b"FFilter",
        b"FDecodeParms",
        b"DL",
    ];

    let mut trailer = PdfDict::new();
    for (key, value) in previous.iter() {
        if !NOT_CARRIED.contains(&key.as_slice()) {
            trailer.insert(key.clone(), value.clone());
        }
    }
    let previous_size = previous.get_i64(b"Size").unwrap_or(0);
    trailer.insert(
        b"Size".to_vec(),
        PdfObject::Integer(previous_size.max(i64::from(size))),
    );
    trailer.insert(b"Root".to_vec(), PdfObject::Reference(root.clone()));
    if let Some(info) = info {
        trailer.insert(b"Info".to_vec(), PdfObject::Reference(info.clone()));
    }
    trailer.insert(b"Prev".to_vec(), PdfObject::Integer(prev_offset as i64));
    trailer
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

    /// Text of the first page of `doc`.
    fn first_page_text(doc: &PdfDocument) -> String {
        let pages = collect_pages(doc).unwrap();
        crate::text::extract_page_text_string(doc, &pages[0]).unwrap()
    }

    /// Assert that every object listed in `doc`'s xref resolves.
    fn assert_all_objects_resolve(doc: &PdfDocument) {
        for r in doc.object_refs().collect::<Vec<_>>() {
            if let Err(e) = doc.resolve(&r) {
                panic!("object {} {} does not resolve: {e}", r.obj_num, r.gen_num);
            }
        }
    }

    /// A one-page PDF reading "Secret page", encrypted with user password
    /// `user` and owner password `owner`.
    fn create_encrypted_pdf(method: crate::crypto::EncryptionMethod) -> Vec<u8> {
        let mut doc = DocumentBuilder::new();
        let font = doc.add_standard_font("Helvetica");
        let mut page = PageBuilder::new(612.0, 792.0);
        page.add_font(&font, "Helvetica");
        page.begin_text();
        page.set_font(&font, 12.0);
        page.move_to(72.0, 720.0);
        page.show_text("Secret page");
        page.end_text();
        doc.add_page(page);
        doc.set_encryption(crate::crypto::EncryptionConfig {
            user_password: b"user".to_vec(),
            owner_password: b"owner".to_vec(),
            permissions: crate::crypto::Permissions::allow_all(),
            method,
            encrypt_metadata: true,
        });
        doc.build().unwrap()
    }

    #[test]
    fn test_incremental_save_keeps_encryption() {
        use crate::crypto::EncryptionMethod;
        for method in [
            EncryptionMethod::RC4_128,
            EncryptionMethod::AES128,
            EncryptionMethod::AES256,
        ] {
            let original = create_encrypted_pdf(method);
            let mut doc = PdfDocument::from_bytes(original.clone()).unwrap();
            doc.authenticate(b"user").unwrap();
            let mut modifier = DocumentModifier::from_document(&doc).unwrap();
            modifier.set_info(b"Title", "Updated Title");

            let result = incremental_save(&doc, modifier).unwrap();
            assert_eq!(&result[..original.len()], &original[..]);
            let appended = &result[original.len()..];
            assert!(
                !appended.windows(13).any(|w| w == b"Updated Title"),
                "{method:?}: appended objects are plaintext"
            );

            let mut reopened = PdfDocument::from_bytes(result).unwrap();
            assert!(reopened.is_encrypted(), "{method:?}");
            assert!(!reopened.is_authenticated(), "{method:?}");
            assert_eq!(
                reopened.trailer().get(b"Encrypt"),
                doc.trailer().get(b"Encrypt")
            );
            assert!(doc.trailer().get(b"ID").is_some());
            assert_eq!(reopened.trailer().get(b"ID"), doc.trailer().get(b"ID"));
            assert!(reopened.authenticate(b"wrong").is_err(), "{method:?}");
            reopened.authenticate(b"user").unwrap();

            assert_all_objects_resolve(&reopened);
            assert_eq!(
                first_page_text(&reopened).trim(),
                "Secret page",
                "{method:?}"
            );
            let info = match reopened.trailer().get(b"Info") {
                Some(PdfObject::Reference(r)) => r.clone(),
                other => panic!("{method:?}: unexpected /Info {other:?}"),
            };
            let title = match reopened.resolve(&info).unwrap() {
                PdfObject::Dict(d) => d.get(b"Title").cloned(),
                other => panic!("{method:?}: unexpected Info {other:?}"),
            };
            assert_eq!(
                title,
                Some(PdfObject::String(b"Updated Title".to_vec())),
                "{method:?}"
            );
        }
    }

    #[test]
    fn test_from_document_requires_authentication() {
        let original = create_encrypted_pdf(crate::crypto::EncryptionMethod::AES128);
        let doc = PdfDocument::from_bytes(original).unwrap();
        assert!(!doc.is_authenticated());
        assert!(matches!(
            DocumentModifier::from_document(&doc),
            Err(crate::error::JustPdfError::EncryptedDocument)
        ));
    }

    #[test]
    fn test_incremental_save_reopens_with_intact_objects() {
        let original = create_test_pdf("Original", 2);
        let doc = PdfDocument::from_bytes(original.clone()).unwrap();
        let mut modifier = DocumentModifier::from_document(&doc).unwrap();
        modifier.set_info(b"Title", "Updated Title");

        let result = incremental_save(&doc, modifier).unwrap();
        let reopened = PdfDocument::from_bytes(result).unwrap();
        assert_all_objects_resolve(&reopened);
        assert!(first_page_text(&reopened).contains("Original"));
    }

    /// Byte offset of object `obj_num` in `doc`, when it is an uncompressed in-use entry.
    fn object_offset(doc: &PdfDocument, obj_num: u32) -> Option<usize> {
        match doc.xref.get(obj_num) {
            Some(crate::xref::XrefEntry::InUse { offset, .. }) => Some(*offset as usize),
            _ => None,
        }
    }

    #[test]
    fn test_incremental_save_appends_only_changed_objects() {
        let original = create_test_pdf("Original", 2);
        let doc = PdfDocument::from_bytes(original.clone()).unwrap();
        let source_nums: Vec<u32> = doc.object_refs().map(|r| r.obj_num).collect();
        let mut modifier = DocumentModifier::from_document(&doc).unwrap();
        modifier.set_info(b"Title", "Updated Title");
        let info_num = modifier.info_ref.as_ref().unwrap().obj_num;

        let result = incremental_save(&doc, modifier).unwrap();
        let reopened = PdfDocument::from_bytes(result).unwrap();

        assert!(object_offset(&reopened, info_num).unwrap() >= original.len());
        let rewritten: Vec<u32> = source_nums
            .iter()
            .copied()
            .filter(|&n| n != info_num)
            .filter(|&n| object_offset(&reopened, n).unwrap() >= original.len())
            .collect();
        assert!(
            rewritten.is_empty(),
            "unchanged objects appended: {rewritten:?}"
        );
        assert!(source_nums.len() > 3);
        assert_all_objects_resolve(&reopened);
        assert!(first_page_text(&reopened).contains("Original"));
    }

    #[test]
    fn test_incremental_save_without_changes_returns_the_original() {
        let original = create_test_pdf("Original", 1);
        let doc = PdfDocument::from_bytes(original.clone()).unwrap();
        let modifier = DocumentModifier::from_document(&doc).unwrap();

        assert_eq!(incremental_save(&doc, modifier).unwrap(), original);
    }

    #[test]
    fn test_incremental_save_frees_removed_objects() {
        let original = create_test_pdf("Original", 2);
        let doc = PdfDocument::from_bytes(original.clone()).unwrap();
        let second_page = collect_pages(&doc).unwrap()[1].page_ref.clone();
        let second_contents = match doc.resolve(&second_page).unwrap() {
            PdfObject::Dict(d) => d.get_ref(b"Contents").unwrap().obj_num,
            other => panic!("unexpected page {other:?}"),
        };
        let mut modifier = DocumentModifier::from_document(&doc).unwrap();
        modifier.delete_page(1).unwrap();
        modifier.garbage_collect();

        let result = incremental_save(&doc, modifier).unwrap();
        let reopened = PdfDocument::from_bytes(result).unwrap();

        for num in [second_page.obj_num, second_contents] {
            match reopened.xref.get(num) {
                Some(crate::xref::XrefEntry::Free {
                    next_free: 0,
                    gen_num: 65535,
                }) => {}
                other => panic!("object {num}: expected a free entry, got {other:?}"),
            }
        }
        let pages = collect_pages(&reopened).unwrap();
        assert_eq!(pages.len(), 1);
        assert!(object_offset(&reopened, pages[0].page_ref.obj_num).unwrap() < original.len());
        assert_all_objects_resolve(&reopened);
        assert!(first_page_text(&reopened).contains("Page 1"));
    }

    /// `data` with an update section whose xref lists object `obj_num` at bytes
    /// that are not an object.
    fn append_unresolvable_object(data: &[u8], obj_num: u32) -> Vec<u8> {
        use std::io::Write;
        let doc = PdfDocument::from_bytes(data.to_vec()).unwrap();
        let mut buf = data.to_vec();
        let garbage_offset = buf.len();
        buf.extend_from_slice(b"garbage\n");
        let xref_offset = buf.len();
        write!(
            buf,
            "xref\n{obj_num} 1\n{garbage_offset:010} 00000 n \r\ntrailer\n"
        )
        .unwrap();
        let trailer = incremental_trailer(
            doc.trailer(),
            obj_num + 1,
            doc.catalog_ref().unwrap(),
            None,
            crate::xref::find_startxref(data).unwrap(),
        );
        crate::writer::serialize::serialize_dict(&mut buf, &trailer).unwrap();
        write!(buf, "\nstartxref\n{xref_offset}\n%%EOF\n").unwrap();
        buf
    }

    #[test]
    fn test_incremental_save_keeps_an_object_that_does_not_resolve() {
        let plain = create_test_pdf("Original", 1);
        let broken_num = PdfDocument::from_bytes(plain.clone())
            .unwrap()
            .object_refs()
            .map(|r| r.obj_num)
            .max()
            .unwrap()
            + 1;
        let original = append_unresolvable_object(&plain, broken_num);
        let doc = PdfDocument::from_bytes(original.clone()).unwrap();
        assert!(object_offset(&doc, broken_num).is_some());
        let mut modifier = DocumentModifier::from_document(&doc).unwrap();
        modifier.set_info(b"Title", "Updated Title");

        let result = incremental_save(&doc, modifier).unwrap();
        let reopened = PdfDocument::from_bytes(result).unwrap();

        assert_eq!(
            object_offset(&reopened, broken_num),
            object_offset(&doc, broken_num)
        );
        assert!(first_page_text(&reopened).contains("Original"));
    }

    /// A two-page PDF whose eligible objects are packed into object streams,
    /// written with an xref stream.
    fn create_packed_pdf() -> Vec<u8> {
        let doc = PdfDocument::from_bytes(create_test_pdf("Packed", 2)).unwrap();
        let mut modifier = DocumentModifier::from_document(&doc).unwrap();
        let catalog_num = modifier.catalog_ref.obj_num;
        let pages_num = modifier.find_pages_ref().unwrap().obj_num;
        let packed = crate::writer::object_stream::pack_object_streams(
            &modifier.writer.objects,
            100,
            catalog_num,
            Some(pages_num),
            None,
        )
        .unwrap();
        assert!(!packed.compressed.is_empty());
        modifier.writer.objects = packed.objects;
        modifier.build_with_xref_stream(&packed.compressed).unwrap()
    }

    #[test]
    fn test_incremental_save_keeps_object_streams_that_hold_unchanged_objects() {
        let original = create_packed_pdf();
        let doc = PdfDocument::from_bytes(original).unwrap();
        let containers: Vec<u32> = doc
            .object_refs()
            .filter_map(|r| match doc.xref.get(r.obj_num) {
                Some(crate::xref::XrefEntry::Compressed { obj_stream_num, .. }) => {
                    Some(*obj_stream_num)
                }
                _ => None,
            })
            .collect();
        assert!(!containers.is_empty());
        let mut modifier = DocumentModifier::from_document(&doc).unwrap();
        modifier.set_info(b"Title", "Updated Title");
        modifier.garbage_collect();

        let result = incremental_save(&doc, modifier).unwrap();
        let reopened = PdfDocument::from_bytes(result).unwrap();

        for num in containers {
            assert!(
                !matches!(
                    reopened.xref.get(num),
                    Some(crate::xref::XrefEntry::Free { .. })
                ),
                "object stream {num} freed"
            );
        }
        assert_all_objects_resolve(&reopened);
        assert_eq!(collect_pages(&reopened).unwrap().len(), 2);
        assert!(first_page_text(&reopened).contains("Packed"));
    }

    #[test]
    fn test_incremental_save_deletes_a_page_of_a_packed_document() {
        let original = create_packed_pdf();
        let doc = PdfDocument::from_bytes(original).unwrap();
        let second_page = collect_pages(&doc).unwrap()[1].page_ref.obj_num;
        let mut modifier = DocumentModifier::from_document(&doc).unwrap();
        modifier.delete_page(1).unwrap();
        modifier.garbage_collect();

        let result = incremental_save(&doc, modifier).unwrap();
        let reopened = PdfDocument::from_bytes(result).unwrap();

        assert!(matches!(
            reopened.xref.get(second_page),
            Some(crate::xref::XrefEntry::Free { .. })
        ));
        assert_all_objects_resolve(&reopened);
        assert_eq!(collect_pages(&reopened).unwrap().len(), 1);
        assert!(first_page_text(&reopened).contains("Packed - Page 1"));
    }

    #[test]
    fn test_incremental_save_of_an_encrypted_document() {
        let original = create_test_pdf("Secret", 2);
        let original = encrypt_with_modifier(original, crate::crypto::EncryptionMethod::AES128);
        let mut doc = PdfDocument::from_bytes(original.clone()).unwrap();
        doc.authenticate(b"user").unwrap();
        let encrypt_num = doc.trailer().get_ref(b"Encrypt").unwrap().obj_num;

        let unchanged = DocumentModifier::from_document(&doc).unwrap();
        assert_eq!(incremental_save(&doc, unchanged).unwrap(), original);

        let mut modifier = DocumentModifier::from_document(&doc).unwrap();
        modifier.delete_page(1).unwrap();
        modifier.garbage_collect();
        let result = incremental_save(&doc, modifier).unwrap();
        let mut reopened = PdfDocument::from_bytes(result).unwrap();

        assert!(matches!(
            reopened.xref.get(encrypt_num),
            Some(crate::xref::XrefEntry::InUse { .. })
        ));
        reopened.authenticate(b"user").unwrap();
        assert_all_objects_resolve(&reopened);
        assert_eq!(collect_pages(&reopened).unwrap().len(), 1);
        assert!(first_page_text(&reopened).contains("Secret - Page 1"));
    }

    #[test]
    fn test_incremental_save_appends_an_object_edited_in_place() {
        let original = create_test_pdf("Original", 1);
        let doc = PdfDocument::from_bytes(original.clone()).unwrap();
        let page_num = collect_pages(&doc).unwrap()[0].page_ref.obj_num;
        let mut modifier = DocumentModifier::from_document(&doc).unwrap();
        for (num, obj) in modifier.writer().objects.iter_mut() {
            if let (true, PdfObject::Dict(page)) = (*num == page_num, obj) {
                page.insert(b"Rotate".to_vec(), PdfObject::Integer(90));
            }
        }

        let result = incremental_save(&doc, modifier).unwrap();
        let reopened = PdfDocument::from_bytes(result).unwrap();

        assert!(object_offset(&reopened, page_num).unwrap() >= original.len());
        let page = collect_pages(&reopened).unwrap()[0].page_ref.clone();
        match reopened.resolve(&page).unwrap() {
            PdfObject::Dict(d) => assert_eq!(d.get(b"Rotate"), Some(&PdfObject::Integer(90))),
            other => panic!("unexpected page {other:?}"),
        }
        assert_all_objects_resolve(&reopened);
    }

    #[test]
    fn test_incremental_save_after_xref_stream_section() {
        let doc = PdfDocument::from_bytes(create_test_pdf("Streamed", 1)).unwrap();
        let original = DocumentModifier::from_document(&doc)
            .unwrap()
            .build_with_xref_stream(&[])
            .unwrap();
        let doc = PdfDocument::from_bytes(original.clone()).unwrap();
        assert_eq!(doc.trailer().get_name(b"Type"), Some(b"XRef".as_slice()));
        let mut modifier = DocumentModifier::from_document(&doc).unwrap();
        modifier.set_info(b"Title", "Updated Title");

        let result = incremental_save(&doc, modifier).unwrap();
        assert!(result.len() > original.len());
        let reopened = PdfDocument::from_bytes(result).unwrap();
        for key in [
            &b"Type"[..],
            b"W",
            b"Index",
            b"Filter",
            b"DecodeParms",
            b"Length",
        ] {
            assert!(
                reopened.trailer().get(key).is_none(),
                "stream key /{} copied into the trailer",
                String::from_utf8_lossy(key)
            );
        }
        assert_all_objects_resolve(&reopened);
        assert!(first_page_text(&reopened).contains("Streamed"));
    }

    #[test]
    fn test_incremental_trailer_copies_previous_keys() {
        let mut previous = PdfDict::new();
        for (key, value) in [
            (&b"Size"[..], PdfObject::Integer(40)),
            (
                b"Root",
                PdfObject::Reference(IndirectRef {
                    obj_num: 1,
                    gen_num: 0,
                }),
            ),
            (
                b"Info",
                PdfObject::Reference(IndirectRef {
                    obj_num: 2,
                    gen_num: 0,
                }),
            ),
            (
                b"Encrypt",
                PdfObject::Reference(IndirectRef {
                    obj_num: 3,
                    gen_num: 0,
                }),
            ),
            (
                b"ID",
                PdfObject::Array(vec![
                    PdfObject::String(vec![1; 16]),
                    PdfObject::String(vec![2; 16]),
                ]),
            ),
            (b"Custom", PdfObject::Name(b"Kept".to_vec())),
            (b"Prev", PdfObject::Integer(9)),
            (b"XRefStm", PdfObject::Integer(99)),
            (b"Type", PdfObject::Name(b"XRef".to_vec())),
            (b"W", PdfObject::Array(vec![PdfObject::Integer(1)])),
            (b"Index", PdfObject::Array(vec![PdfObject::Integer(0)])),
            (b"Filter", PdfObject::Name(b"FlateDecode".to_vec())),
            (b"DecodeParms", PdfObject::Dict(PdfDict::new())),
            (b"Length", PdfObject::Integer(12)),
        ] {
            previous.insert(key.to_vec(), value);
        }
        let root = IndirectRef {
            obj_num: 7,
            gen_num: 0,
        };

        let trailer = incremental_trailer(&previous, 20, &root, None, 1234);
        assert_eq!(trailer.get(b"Size"), Some(&PdfObject::Integer(40)));
        assert_eq!(
            trailer.get(b"Root"),
            Some(&PdfObject::Reference(root.clone()))
        );
        assert_eq!(trailer.get(b"Prev"), Some(&PdfObject::Integer(1234)));
        for key in [&b"Info"[..], b"Encrypt", b"ID", b"Custom"] {
            assert_eq!(
                trailer.get(key),
                previous.get(key),
                "/{}",
                String::from_utf8_lossy(key)
            );
        }
        for key in [
            &b"XRefStm"[..],
            b"Type",
            b"W",
            b"Index",
            b"Filter",
            b"DecodeParms",
            b"Length",
        ] {
            assert!(
                trailer.get(key).is_none(),
                "/{}",
                String::from_utf8_lossy(key)
            );
        }

        let info = IndirectRef {
            obj_num: 8,
            gen_num: 0,
        };
        let trailer = incremental_trailer(&previous, 50, &root, Some(&info), 1234);
        assert_eq!(trailer.get(b"Size"), Some(&PdfObject::Integer(50)));
        assert_eq!(trailer.get(b"Info"), Some(&PdfObject::Reference(info)));
    }

    #[test]
    fn test_incremental_save() {
        let original = create_test_pdf("Original", 1);
        let original_len = original.len();

        let mut doc = PdfDocument::from_bytes(original.clone()).unwrap();
        let mut modifier = DocumentModifier::from_document(&doc).unwrap();
        modifier.set_info(b"Title", "Updated Title");

        let result = incremental_save(&doc, modifier).unwrap();

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

    /// A plain one-page PDF reading "Secret page" with `/Title (Plain title)`,
    /// and `/ID [<first> <second>]` in its trailer when `id` is given.
    fn create_plain_pdf(id: Option<(&[u8], &[u8])>) -> Vec<u8> {
        let mut doc = DocumentBuilder::new();
        let font = doc.add_standard_font("Helvetica");
        let mut page = PageBuilder::new(612.0, 792.0);
        page.add_font(&font, "Helvetica");
        page.begin_text();
        page.set_font(&font, 12.0);
        page.move_to(72.0, 720.0);
        page.show_text("Secret page");
        page.end_text();
        doc.add_page(page);
        doc.set_title("Plain title");
        let bytes = doc.build().unwrap();
        let Some((first, second)) = id else {
            return bytes;
        };
        let hex = |b: &[u8]| b.iter().map(|x| format!("{x:02X}")).collect::<String>();
        let at = bytes
            .windows(10)
            .rposition(|w| w == b"trailer\n<<")
            .unwrap()
            + 10;
        let mut out = bytes[..at].to_vec();
        out.extend_from_slice(format!(" /ID [<{}> <{}>]", hex(first), hex(second)).as_bytes());
        out.extend_from_slice(&bytes[at..]);
        out
    }

    fn encryption_config(
        method: crate::crypto::EncryptionMethod,
    ) -> crate::crypto::EncryptionConfig {
        crate::crypto::EncryptionConfig {
            user_password: b"user".to_vec(),
            owner_password: b"owner".to_vec(),
            permissions: crate::crypto::Permissions::allow_all(),
            method,
            encrypt_metadata: true,
        }
    }

    /// Encrypt `source` through `DocumentModifier::set_encryption` + `build`.
    fn encrypt_with_modifier(source: Vec<u8>, method: crate::crypto::EncryptionMethod) -> Vec<u8> {
        let doc = PdfDocument::from_bytes(source).unwrap();
        let mut modifier = DocumentModifier::from_document(&doc).unwrap();
        modifier.set_encryption(encryption_config(method));
        modifier.build().unwrap()
    }

    fn trailer_id(doc: &PdfDocument) -> Vec<Vec<u8>> {
        match doc.trailer().get(b"ID") {
            Some(PdfObject::Array(arr)) => arr
                .iter()
                .map(|o| match o {
                    PdfObject::String(s) => s.clone(),
                    other => panic!("/ID element is not a string: {other:?}"),
                })
                .collect(),
            other => panic!("trailer /ID missing: {other:?}"),
        }
    }

    fn info_title(doc: &PdfDocument) -> Option<PdfObject> {
        let info = match doc.trailer().get(b"Info") {
            Some(PdfObject::Reference(r)) => r.clone(),
            other => panic!("unexpected /Info {other:?}"),
        };
        match doc.resolve(&info).unwrap() {
            PdfObject::Dict(d) => d.get(b"Title").cloned(),
            other => panic!("unexpected Info {other:?}"),
        }
    }

    #[test]
    fn test_build_with_encryption_roundtrip() {
        use crate::crypto::EncryptionMethod;
        for method in [
            EncryptionMethod::RC4_128,
            EncryptionMethod::AES128,
            EncryptionMethod::AES256,
        ] {
            let bytes = encrypt_with_modifier(create_plain_pdf(None), method);
            assert!(
                !bytes.windows(11).any(|w| w == b"Plain title"),
                "{method:?}: /Info written in plaintext"
            );

            let mut doc = PdfDocument::from_bytes(bytes.clone()).unwrap();
            assert!(doc.is_encrypted(), "{method:?}");
            assert!(!doc.is_authenticated(), "{method:?}");
            assert!(doc.authenticate(b"wrong").is_err(), "{method:?}");
            for password in [&b"user"[..], b"owner"] {
                let mut doc = PdfDocument::from_bytes(bytes.clone()).unwrap();
                doc.authenticate(password).unwrap();
                assert_all_objects_resolve(&doc);
                assert_eq!(first_page_text(&doc).trim(), "Secret page", "{method:?}");
                assert_eq!(
                    info_title(&doc),
                    Some(PdfObject::String(b"Plain title".to_vec())),
                    "{method:?}"
                );
            }

            doc.authenticate(b"user").unwrap();
            let id = trailer_id(&doc);
            assert_eq!(id.len(), 2, "{method:?}");
            assert_eq!(id[0].len(), 16, "{method:?}");
            assert_eq!(
                id[0], id[1],
                "{method:?}: a file without /ID is written as new"
            );
        }
    }

    #[test]
    fn test_build_with_encryption_keeps_a_permanent_id_that_holds_a_carriage_return() {
        use crate::crypto::EncryptionMethod;
        let permanent = *b"AB\rCDEFGHIPQRSTU";
        for method in [EncryptionMethod::RC4_128, EncryptionMethod::AES128] {
            let bytes = encrypt_with_modifier(
                create_plain_pdf(Some((&permanent, b"changing-id-0002"))),
                method,
            );
            let mut doc = PdfDocument::from_bytes(bytes).unwrap();
            doc.authenticate(b"user").unwrap();

            assert_eq!(trailer_id(&doc)[0], permanent, "{method:?}");
            assert_eq!(first_page_text(&doc).trim(), "Secret page", "{method:?}");
        }
    }

    #[test]
    fn test_build_with_encryption_keeps_permanent_id_and_changes_the_other() {
        use crate::crypto::EncryptionMethod;
        let permanent = *b"permanent-id-001";
        let changing = *b"changing-id-0002";
        for method in [
            EncryptionMethod::RC4_128,
            EncryptionMethod::AES128,
            EncryptionMethod::AES256,
        ] {
            let bytes =
                encrypt_with_modifier(create_plain_pdf(Some((&permanent, &changing))), method);
            let mut doc = PdfDocument::from_bytes(bytes).unwrap();
            doc.authenticate(b"user").unwrap();
            assert_eq!(first_page_text(&doc).trim(), "Secret page", "{method:?}");

            let id = trailer_id(&doc);
            assert_eq!(id[0], permanent, "{method:?}");
            assert_eq!(id[1].len(), 16, "{method:?}");
            assert_ne!(
                id[1], permanent,
                "{method:?}: changing identifier not updated"
            );
            assert_ne!(
                id[1], changing,
                "{method:?}: changing identifier not updated"
            );
        }
    }

    #[test]
    fn test_build_with_encryption_treats_empty_id_as_absent() {
        let bytes = encrypt_with_modifier(
            create_plain_pdf(Some((b"", b""))),
            crate::crypto::EncryptionMethod::AES128,
        );
        let mut doc = PdfDocument::from_bytes(bytes).unwrap();
        doc.authenticate(b"user").unwrap();
        let id = trailer_id(&doc);
        assert_eq!(id[0].len(), 16);
        assert_eq!(id[0], id[1]);
    }

    #[test]
    fn test_build_without_encryption_decrypts_an_authenticated_source() {
        let original = create_encrypted_pdf(crate::crypto::EncryptionMethod::AES128);
        let mut doc = PdfDocument::from_bytes(original).unwrap();
        doc.authenticate(b"user").unwrap();
        let bytes = DocumentModifier::from_document(&doc)
            .unwrap()
            .build()
            .unwrap();

        let reopened = PdfDocument::from_bytes(bytes).unwrap();
        assert!(!reopened.is_encrypted());
        assert!(reopened.trailer().get(b"Encrypt").is_none());
        assert_eq!(first_page_text(&reopened).trim(), "Secret page");
    }

    #[test]
    fn test_build_with_xref_stream_refuses_encryption() {
        let doc = PdfDocument::from_bytes(create_plain_pdf(None)).unwrap();
        let mut modifier = DocumentModifier::from_document(&doc).unwrap();
        modifier.set_encryption(encryption_config(crate::crypto::EncryptionMethod::AES128));
        assert!(matches!(
            modifier.build_with_xref_stream(&[]),
            Err(crate::error::JustPdfError::UnsupportedEncryption { .. })
        ));
    }

    #[test]
    fn test_incremental_save_refuses_encryption() {
        let original = create_plain_pdf(None);
        let doc = PdfDocument::from_bytes(original.clone()).unwrap();
        let mut modifier = DocumentModifier::from_document(&doc).unwrap();
        modifier.set_encryption(encryption_config(crate::crypto::EncryptionMethod::AES128));
        assert!(matches!(
            incremental_save(&doc, modifier),
            Err(crate::error::JustPdfError::UnsupportedEncryption { .. })
        ));
    }

    #[test]
    fn test_build_with_encryption_reencrypts_an_authenticated_source() {
        use crate::crypto::EncryptionMethod;
        let original = create_encrypted_pdf(EncryptionMethod::AES128);
        let mut source = PdfDocument::from_bytes(original).unwrap();
        source.authenticate(b"user").unwrap();
        let source_id = trailer_id(&source);

        let mut modifier = DocumentModifier::from_document(&source).unwrap();
        modifier.set_encryption(crate::crypto::EncryptionConfig {
            user_password: b"new-user".to_vec(),
            owner_password: b"new-owner".to_vec(),
            permissions: crate::crypto::Permissions::allow_all(),
            method: EncryptionMethod::AES256,
            encrypt_metadata: true,
        });
        let bytes = modifier.build().unwrap();

        let mut doc = PdfDocument::from_bytes(bytes).unwrap();
        assert!(
            doc.authenticate(b"user").is_err(),
            "old password still opens it"
        );
        doc.authenticate(b"new-user").unwrap();
        assert_eq!(first_page_text(&doc).trim(), "Secret page");
        let id = trailer_id(&doc);
        assert_eq!(id[0], source_id[0]);
        assert_ne!(id[1], source_id[1]);
    }

    #[test]
    fn test_build_with_encryption_encrypts_an_object_set_at_a_new_number() {
        let doc = PdfDocument::from_bytes(create_plain_pdf(None)).unwrap();
        let mut modifier = DocumentModifier::from_document(&doc).unwrap();
        let next = modifier
            .writer()
            .objects
            .iter()
            .map(|(n, _)| *n)
            .max()
            .unwrap()
            + 1;
        modifier.set_object(next, PdfObject::String(b"TOPSECRET".to_vec()));
        modifier.set_encryption(encryption_config(crate::crypto::EncryptionMethod::AES128));
        let bytes = modifier.build().unwrap();
        assert!(
            !bytes.windows(9).any(|w| w == b"TOPSECRET"),
            "object written in plaintext"
        );

        let mut doc = PdfDocument::from_bytes(bytes).unwrap();
        doc.authenticate(b"user").unwrap();
        let secret = doc
            .resolve(&IndirectRef {
                obj_num: next,
                gen_num: 0,
            })
            .unwrap();
        assert_eq!(secret, PdfObject::String(b"TOPSECRET".to_vec()));
    }

    /// Occurrences of a standard security handler dictionary in `bytes`.
    fn count_standard_handlers(bytes: &[u8]) -> usize {
        bytes
            .windows(17)
            .filter(|w| w == b"/Filter /Standard")
            .count()
    }

    #[test]
    fn test_rewrite_drops_the_source_encrypt_dictionary() {
        use crate::crypto::EncryptionMethod;
        let original = create_encrypted_pdf(EncryptionMethod::AES128);
        assert_eq!(count_standard_handlers(&original), 1);
        let mut doc = PdfDocument::from_bytes(original).unwrap();
        doc.authenticate(b"user").unwrap();

        let plain = DocumentModifier::from_document(&doc)
            .unwrap()
            .build()
            .unwrap();
        assert_eq!(count_standard_handlers(&plain), 0, "old /Encrypt copied");

        let mut modifier = DocumentModifier::from_document(&doc).unwrap();
        modifier.set_encryption(encryption_config(EncryptionMethod::AES256));
        let reencrypted = modifier.build().unwrap();
        assert_eq!(
            count_standard_handlers(&reencrypted),
            1,
            "old /Encrypt copied"
        );
    }
}
