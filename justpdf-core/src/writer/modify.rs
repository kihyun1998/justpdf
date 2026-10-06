//! Document modification: load existing PDF, modify, and save.
//! Also provides page merge/split operations.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::error::Result;
use crate::object::{IndirectRef, PdfDict, PdfObject};
use crate::page::{PageInfo, collect_pages};
use crate::parser::PdfDocument;
use crate::writer::PdfWriter;
use crate::writer::page::PageBuilder;
use crate::writer::serialize::serialize_pdf;

/// Modifier for existing PDF documents.
/// Loads all objects from a PdfDocument, allows modification, then saves.
pub struct DocumentModifier {
    writer: PdfWriter,
    /// Object number of the catalog.
    catalog_num: u32,
    /// Object number of the document info dictionary, when there is one.
    info_num: Option<u32>,
    /// First element of the source trailer's `/ID`, empty when absent.
    source_file_id: Vec<u8>,
    /// The source's security state and `/Encrypt` dictionary, when the source
    /// is encrypted.
    source_encryption: Option<(crate::crypto::SecurityState, Option<PdfDict>)>,
    /// Encryption applied by `build`.
    encryption: Encryption,
}

/// Encryption applied by [`DocumentModifier::build`].
enum Encryption {
    /// Written without encryption.
    None,
    /// Encrypted with a new configuration (`set_encryption`).
    New(crate::crypto::EncryptionConfig),
    /// Encrypted as the source is (`preserve_encryption`).
    Source,
}

impl DocumentModifier {
    /// Create a modifier from an existing PdfDocument.
    /// Copies all objects from the document into the writer, except the
    /// trailer's `/Encrypt` dictionary; each keeps its generation number, which
    /// `build`, `build_with_xref_stream` and `incremental_save` write it at.
    /// Objects added later are generation 0. An encrypted document must be
    /// authenticated first (`JustPdfError::EncryptedDocument` otherwise).
    pub fn from_document(doc: &PdfDocument) -> Result<Self> {
        if doc.is_encrypted() && !doc.is_authenticated() {
            return Err(crate::error::JustPdfError::EncryptedDocument);
        }
        let mut writer = PdfWriter::new();
        writer.version = doc.version;

        let catalog_num = doc.catalog_ref().map_or(1, |r| r.obj_num);
        let info_num = doc.trailer().get_ref(b"Info").map(|r| r.obj_num);

        // Copy all objects; new numbers start above every number the source uses
        let max_obj = doc.object_refs().map(|r| r.obj_num).max().unwrap_or(0);
        writer.objects.extend(source_objects(doc));
        writer.generations = doc
            .object_refs()
            .filter(|r| r.gen_num != 0)
            .map(|r| (r.obj_num, r.gen_num))
            .collect();
        writer.next_obj_num = max_obj + 1;

        Ok(Self {
            writer,
            catalog_num,
            info_num,
            source_file_id: doc.extract_file_id(),
            source_encryption: doc
                .security_state()
                .map(|state| (state.clone(), source_encrypt_dict(doc))),
            encryption: Encryption::None,
        })
    }

    /// Get a reference to the internal writer for low-level modifications.
    pub fn writer(&mut self) -> &mut PdfWriter {
        &mut self.writer
    }

    /// Reference to the catalog at the generation `build` writes it at — the
    /// trailer's `/Root`.
    pub fn catalog_ref(&self) -> IndirectRef {
        self.writer.reference_to(self.catalog_num)
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
        let pages_obj = self
            .find_object(pages_obj_num)
            .cloned()
            .unwrap_or(PdfObject::Null);

        if let PdfObject::Dict(mut pages_dict) = pages_obj {
            if let Some(PdfObject::Array(mut kids)) = pages_dict.remove(b"Kids") {
                if page_index < kids.len() {
                    kids.remove(page_index);
                    let count = kids.len() as i64;
                    pages_dict.insert(b"Kids".to_vec(), PdfObject::Array(kids));
                    pages_dict.insert(b"Count".to_vec(), PdfObject::Integer(count));
                    self.writer
                        .set_object(pages_obj_num, PdfObject::Dict(pages_dict));
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

        let pages_obj = self
            .find_object(pages_obj_num)
            .cloned()
            .unwrap_or(PdfObject::Null);

        if let PdfObject::Dict(mut pages_dict) = pages_obj {
            if let Some(PdfObject::Array(mut kids)) = pages_dict.remove(b"Kids") {
                let idx = page_index.min(kids.len());
                kids.insert(idx, PdfObject::Reference(page_ref));
                let count = kids.len() as i64;
                pages_dict.insert(b"Kids".to_vec(), PdfObject::Array(kids));
                pages_dict.insert(b"Count".to_vec(), PdfObject::Integer(count));
                self.writer
                    .set_object(pages_obj_num, PdfObject::Dict(pages_dict));
            }
        }

        Ok(())
    }

    /// Reorder pages. `order` is a list of 0-based page indices in the desired order.
    pub fn reorder_pages(&mut self, order: &[usize]) -> Result<()> {
        let pages_ref = self.find_pages_ref()?;
        let pages_obj_num = pages_ref.obj_num;

        let pages_obj = self
            .find_object(pages_obj_num)
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
                self.writer
                    .set_object(pages_obj_num, PdfObject::Dict(pages_dict));
            }
        }

        Ok(())
    }

    /// Set or update a metadata field in the Info dictionary.
    pub fn set_info(&mut self, key: &[u8], value: &str) {
        let info_num = match self.info_num {
            Some(num) => num,
            None => {
                let num = self.writer.alloc_object_num();
                self.info_num = Some(num);
                num
            }
        };

        // Get or create info dict
        let info_obj = self
            .find_object(info_num)
            .cloned()
            .unwrap_or(PdfObject::Dict(PdfDict::new()));

        if let PdfObject::Dict(mut info_dict) = info_obj {
            info_dict.insert(key.to_vec(), PdfObject::String(value.as_bytes().to_vec()));
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
        reachable.insert(self.catalog_num);
        if let Some(info_num) = self.info_num {
            reachable.insert(info_num);
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
        self.writer
            .objects
            .retain(|(num, _)| reachable.contains(num));
    }

    /// Encrypt the document written by `build` with `config`.
    ///
    /// The trailer `/ID` keeps the source's first element as the permanent
    /// identifier and gets a new random changing identifier; a source without
    /// a usable `/ID` gets a new random identifier in both elements.
    ///
    /// Replaces an earlier `preserve_encryption`.
    pub fn set_encryption(&mut self, config: crate::crypto::EncryptionConfig) {
        self.encryption = Encryption::New(config);
    }

    /// Encrypt the document written by `build` as the source document is:
    /// with the source's `/Encrypt` dictionary and file key, so the source's
    /// user and owner passwords both open it. The trailer `/ID` keeps the
    /// source's first element and gets a new random changing identifier. An
    /// unencrypted source is written unencrypted.
    ///
    /// Replaces an earlier `set_encryption`.
    pub fn preserve_encryption(&mut self) {
        self.encryption = Encryption::Source;
    }

    /// Serialize to PDF bytes, encrypted when `set_encryption` or
    /// `preserve_encryption` was called.
    pub fn build(mut self) -> Result<Vec<u8>> {
        let config = match std::mem::replace(&mut self.encryption, Encryption::None) {
            Encryption::New(config) => config,
            Encryption::Source if self.source_encryption.is_some() => {
                return self.build_with_source_encryption();
            }
            Encryption::None | Encryption::Source => {
                return crate::writer::serialize::serialize_writer(
                    &self.writer,
                    self.catalog_num,
                    self.info_num,
                );
            }
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
            self.catalog_num,
            self.info_num,
            &config,
            &permanent_id,
            &changing_id,
        )
    }

    /// Serialize with the source's `/Encrypt` dictionary and file key.
    fn build_with_source_encryption(mut self) -> Result<Vec<u8>> {
        let Some((mut state, Some(encrypt_dict))) = self.source_encryption.take() else {
            return Err(crate::error::JustPdfError::EncryptionError {
                detail: "the source /Encrypt dictionary could not be read".into(),
            });
        };
        let id_array = [
            PdfObject::String(state.file_id.clone()),
            PdfObject::String(crate::crypto::random_file_id()?),
        ];
        let encrypt_ref = self.writer.add_object(PdfObject::Dict(encrypt_dict));
        state.encrypt_obj_num = Some(encrypt_ref.obj_num);
        crate::writer::serialize::serialize_writer_with_state(
            &self.writer,
            self.catalog_num,
            self.info_num,
            &encrypt_ref,
            &state,
            &id_array,
        )
    }

    /// Pack eligible objects at generation 0 into object streams (PDF 1.5+),
    /// at most `max_objects_per_stream` per stream, each container at a newly
    /// allocated object number. Returns the compressed-object entries for
    /// [`Self::build_with_xref_stream`], or an error, with the writer
    /// unchanged, when `max_objects_per_stream` is 0.
    pub fn pack_object_streams(
        &mut self,
        max_objects_per_stream: usize,
    ) -> Result<Vec<crate::writer::object_stream::CompressedObjInfo>> {
        let catalog_num = self.catalog_num;
        let pages_num = match self.find_object(catalog_num) {
            Some(PdfObject::Dict(catalog)) => catalog.get_ref(b"Pages").map(|r| r.obj_num),
            _ => None,
        };
        let writer = &self.writer;
        let packed = crate::writer::object_stream::pack_objects(
            &writer.objects,
            max_objects_per_stream,
            |obj_num, obj| {
                writer.generation(obj_num) == 0
                    && crate::writer::object_stream::is_eligible(
                        obj_num,
                        obj,
                        catalog_num,
                        pages_num,
                        None,
                    )
            },
            writer.next_obj_num,
        )?;
        self.writer.objects = packed.objects;
        if let Some(last) = packed.compressed.iter().map(|c| c.objstm_num).max() {
            self.writer.next_obj_num = last + 1;
        }
        Ok(packed.compressed)
    }

    /// Serialize to PDF bytes using xref streams (PDF 1.5+).
    /// `compressed` contains info about objects packed into object streams.
    /// Returns an error when `set_encryption` was called, or
    /// `preserve_encryption` on an encrypted source.
    pub fn build_with_xref_stream(
        self,
        compressed: &[crate::writer::object_stream::CompressedObjInfo],
    ) -> Result<Vec<u8>> {
        let encrypted = match self.encryption {
            Encryption::None => false,
            Encryption::New(_) => true,
            Encryption::Source => self.source_encryption.is_some(),
        };
        if encrypted {
            return Err(crate::error::JustPdfError::UnsupportedEncryption {
                detail: "encryption is not supported when writing xref streams".into(),
            });
        }
        crate::writer::serialize::serialize_writer_with_xref_stream(
            &self.writer,
            compressed,
            self.catalog_num,
            self.info_num,
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
        if let Some(PdfObject::Dict(catalog)) = self.find_object(self.catalog_num) {
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

/// The trailer's `/Encrypt` dictionary of `doc`, as written, when it can be
/// read.
fn source_encrypt_dict(doc: &PdfDocument) -> Option<PdfDict> {
    match doc.trailer().get(b"Encrypt")? {
        PdfObject::Dict(d) => Some(d.clone()),
        PdfObject::Reference(r) => match doc.resolve(r).ok()? {
            PdfObject::Dict(d) => Some(d),
            _ => None,
        },
        _ => None,
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

    if matches!(modifier.encryption, Encryption::New(_)) {
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
            Some(obj)
                if **obj == source_obj
                    && doc
                        .xref
                        .get(num)
                        .and_then(crate::xref::XrefEntry::defined_generation)
                        == Some(modifier.writer.generation(num)) =>
            {
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

    let metadata_num = crate::writer::serialize::document_metadata_num(
        &modifier.writer.objects,
        modifier.catalog_ref().obj_num,
    );
    let mut offsets: Vec<(u32, usize)> = Vec::new();
    for (obj_num, obj) in changed {
        let gen_num = modifier.writer.generation(*obj_num);
        let write_obj = match security {
            Some(state) => crate::crypto::encrypt_object_for_writing(
                obj,
                state,
                *obj_num,
                gen_num,
                Some(*obj_num) == metadata_num,
            )?,
            None => obj.clone(),
        };
        offsets.push((*obj_num, buf.len()));
        write!(buf, "{} {} obj\n", obj_num, gen_num)?;
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
            Some(offset) => writeln!(
                buf,
                "{:010} {:05} n ",
                offset,
                modifier.writer.generation(*obj_num)
            )?,
            None => writeln!(buf, "{:010} {:05} f ", 0, 65535)?,
        }
    }

    let max_obj_num = entries.last().map(|(n, _)| *n).unwrap_or(0);
    let trailer = incremental_trailer(
        &previous_trailer,
        max_obj_num + 1,
        &modifier.catalog_ref(),
        modifier
            .info_num
            .map(|n| modifier.writer.reference_to(n))
            .as_ref(),
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
        let info_num = modifier.info_num.unwrap();

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
        let catalog_num = modifier.catalog_num;
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

    /// `create_encrypted_pdf(AES128)` authenticated with `user`.
    fn authenticated_aes128_source() -> PdfDocument {
        let original = create_encrypted_pdf(crate::crypto::EncryptionMethod::AES128);
        let mut doc = PdfDocument::from_bytes(original).unwrap();
        doc.authenticate(b"user").unwrap();
        doc
    }

    #[test]
    fn test_preserve_encryption_on_an_unencrypted_source_writes_it_plain() {
        let doc = PdfDocument::from_bytes(create_plain_pdf(None)).unwrap();
        let mut modifier = DocumentModifier::from_document(&doc).unwrap();
        modifier.preserve_encryption();
        let reopened = PdfDocument::from_bytes(modifier.build().unwrap()).unwrap();
        assert!(!reopened.is_encrypted());
        assert_eq!(first_page_text(&reopened).trim(), "Secret page");
    }

    #[test]
    fn test_set_encryption_after_preserve_encryption_wins() {
        let source = authenticated_aes128_source();
        let mut modifier = DocumentModifier::from_document(&source).unwrap();
        modifier.preserve_encryption();
        modifier.set_encryption(crate::crypto::EncryptionConfig {
            user_password: b"new-user".to_vec(),
            owner_password: b"new-owner".to_vec(),
            permissions: crate::crypto::Permissions::allow_all(),
            method: crate::crypto::EncryptionMethod::AES256,
            encrypt_metadata: true,
        });
        let mut doc = PdfDocument::from_bytes(modifier.build().unwrap()).unwrap();
        assert!(doc.authenticate(b"user").is_err());
        doc.authenticate(b"new-user").unwrap();
        assert_eq!(first_page_text(&doc).trim(), "Secret page");
    }

    #[test]
    fn test_preserve_encryption_after_set_encryption_wins() {
        let source = authenticated_aes128_source();
        let mut modifier = DocumentModifier::from_document(&source).unwrap();
        modifier.set_encryption(encryption_config(crate::crypto::EncryptionMethod::AES256));
        modifier.preserve_encryption();
        let mut doc = PdfDocument::from_bytes(modifier.build().unwrap()).unwrap();
        assert!(!doc.is_authenticated());
        assert_eq!(doc.security_state().unwrap().encrypt_dict.r, 4);
        doc.authenticate(b"user").unwrap();
        assert_eq!(first_page_text(&doc).trim(), "Secret page");
    }

    #[test]
    fn test_preserve_encryption_encrypts_an_object_set_at_the_source_encrypt_number() {
        let source = authenticated_aes128_source();
        let encrypt_num = source.trailer().get_ref(b"Encrypt").unwrap().obj_num;
        let mut modifier = DocumentModifier::from_document(&source).unwrap();
        assert!(modifier.find_object_pub(encrypt_num).is_none());
        modifier.set_object(encrypt_num, PdfObject::String(b"TOPSECRET".to_vec()));
        modifier.preserve_encryption();
        let bytes = modifier.build().unwrap();
        assert!(
            !bytes.windows(9).any(|w| w == b"TOPSECRET"),
            "object written in plaintext"
        );

        let mut doc = PdfDocument::from_bytes(bytes).unwrap();
        doc.authenticate(b"user").unwrap();
        let secret = doc
            .resolve(&IndirectRef {
                obj_num: encrypt_num,
                gen_num: 0,
            })
            .unwrap();
        assert_eq!(secret, PdfObject::String(b"TOPSECRET".to_vec()));
    }

    #[test]
    fn test_build_with_xref_stream_refuses_preserved_encryption() {
        let source = authenticated_aes128_source();
        let mut modifier = DocumentModifier::from_document(&source).unwrap();
        modifier.preserve_encryption();
        assert!(matches!(
            modifier.build_with_xref_stream(&[]),
            Err(crate::error::JustPdfError::UnsupportedEncryption { .. })
        ));
    }

    #[test]
    fn test_incremental_save_accepts_preserved_encryption() {
        let source = authenticated_aes128_source();
        let mut modifier = DocumentModifier::from_document(&source).unwrap();
        modifier.set_info(b"Title", "Incremental title");
        modifier.preserve_encryption();
        let bytes = incremental_save(&source, modifier).unwrap();
        assert!(!bytes.windows(17).any(|w| w == b"Incremental title"));
        let mut doc = PdfDocument::from_bytes(bytes).unwrap();
        assert!(!doc.is_authenticated());
        doc.authenticate(b"user").unwrap();
        assert_eq!(
            info_title(&doc),
            Some(PdfObject::String(b"Incremental title".to_vec()))
        );
    }

    /// `source` with an update section that redefines its `/Info` dictionary at
    /// generation `gen_num` as `<< /Title (Generation one) >>`, the trailer's
    /// `/Info` reading `N gen_num R`. Encrypted with the source's file key when
    /// `password` is given.
    fn with_info_at_generation(source: Vec<u8>, password: Option<&[u8]>, gen_num: u16) -> Vec<u8> {
        use std::io::Write;
        let mut doc = PdfDocument::from_bytes(source.clone()).unwrap();
        if let Some(password) = password {
            doc.authenticate(password).unwrap();
        }
        let info_num = doc.trailer().get_ref(b"Info").unwrap().obj_num;
        let mut dict = PdfDict::new();
        dict.insert(
            b"Title".to_vec(),
            PdfObject::String(b"Generation one".to_vec()),
        );
        let info = match doc.security_state() {
            Some(state) => crate::crypto::encrypt_object_for_writing(
                &PdfObject::Dict(dict),
                state,
                info_num,
                gen_num,
                false,
            )
            .unwrap(),
            None => PdfObject::Dict(dict),
        };

        let mut buf = source.clone();
        let info_offset = buf.len();
        write!(buf, "{info_num} {gen_num} obj\n").unwrap();
        crate::writer::serialize::serialize_object(&mut buf, &info).unwrap();
        write!(buf, "\nendobj\n").unwrap();
        let xref_offset = buf.len();
        write!(
            buf,
            "xref\n{info_num} 1\n{info_offset:010} {gen_num:05} n \r\ntrailer\n"
        )
        .unwrap();
        let trailer = incremental_trailer(
            doc.trailer(),
            info_num + 1,
            doc.catalog_ref().unwrap(),
            Some(&IndirectRef {
                obj_num: info_num,
                gen_num,
            }),
            crate::xref::find_startxref(&source).unwrap(),
        );
        crate::writer::serialize::serialize_dict(&mut buf, &trailer).unwrap();
        write!(buf, "\nstartxref\n{xref_offset}\n%%EOF\n").unwrap();
        buf
    }

    /// Assert that `bytes` keep the `/Info` of `with_info_at_generation` at
    /// generation `gen_num` — trailer reference, object header and xref entry —
    /// and that it reads `title` once opened with `password`.
    fn assert_info_at_generation(
        bytes: Vec<u8>,
        password: Option<&[u8]>,
        gen_num: u16,
        title: &[u8],
    ) {
        let mut doc = PdfDocument::from_bytes(bytes.clone()).unwrap();
        if let Some(password) = password {
            doc.authenticate(password).unwrap();
        }
        let info = doc.trailer().get_ref(b"Info").unwrap().clone();
        assert_eq!(info.gen_num, gen_num, "trailer /Info");
        let header = format!("\n{} {gen_num} obj", info.obj_num);
        assert!(
            bytes.windows(header.len()).any(|w| w == header.as_bytes()),
            "no `{} {gen_num} obj` header",
            info.obj_num
        );
        assert!(
            matches!(
                doc.xref.get(info.obj_num),
                Some(crate::xref::XrefEntry::InUse { gen_num: g, .. }) if *g == gen_num
            ),
            "xref entry {:?}",
            doc.xref.get(info.obj_num)
        );
        assert_all_objects_resolve(&doc);
        assert_eq!(info_title(&doc), Some(PdfObject::String(title.to_vec())));
    }

    /// `create_plain_pdf` with its `/Info` at generation 1, plain or encrypted
    /// with user password `user`.
    fn generation_one_sources() -> Vec<(Vec<u8>, Option<&'static [u8]>)> {
        use crate::crypto::EncryptionMethod;
        let mut sources = vec![(
            with_info_at_generation(create_plain_pdf(None), None, 1),
            None,
        )];
        for method in [EncryptionMethod::RC4_128, EncryptionMethod::AES128] {
            let encrypted = encrypt_with_modifier(create_plain_pdf(None), method);
            let user: &'static [u8] = b"user";
            sources.push((
                with_info_at_generation(encrypted, Some(user), 1),
                Some(user),
            ));
        }
        sources
    }

    fn open(bytes: Vec<u8>, password: Option<&[u8]>) -> PdfDocument {
        let mut doc = PdfDocument::from_bytes(bytes).unwrap();
        if let Some(password) = password {
            doc.authenticate(password).unwrap();
        }
        doc
    }

    #[test]
    fn test_generation_one_source_reads_its_info() {
        for (source, password) in generation_one_sources() {
            assert_info_at_generation(source, password, 1, b"Generation one");
        }
    }

    #[test]
    fn test_build_keeps_a_source_generation() {
        for (source, password) in generation_one_sources() {
            let doc = open(source, password);
            let mut modifier = DocumentModifier::from_document(&doc).unwrap();
            modifier.preserve_encryption();
            assert_info_at_generation(modifier.build().unwrap(), password, 1, b"Generation one");
        }
    }

    #[test]
    fn test_build_keeps_the_generation_of_a_replaced_object() {
        let (source, _) = generation_one_sources().remove(0);
        let doc = open(source, None);
        let mut modifier = DocumentModifier::from_document(&doc).unwrap();
        modifier.set_info(b"Title", "Replaced");
        assert_info_at_generation(modifier.build().unwrap(), None, 1, b"Replaced");
    }

    #[test]
    fn test_build_with_encryption_keeps_a_source_generation() {
        use crate::crypto::EncryptionMethod;
        for method in [EncryptionMethod::RC4_128, EncryptionMethod::AES128] {
            let (source, _) = generation_one_sources().remove(0);
            let doc = open(source, None);
            let mut modifier = DocumentModifier::from_document(&doc).unwrap();
            modifier.set_encryption(encryption_config(method));
            assert_info_at_generation(
                modifier.build().unwrap(),
                Some(b"user"),
                1,
                b"Generation one",
            );
        }
    }

    #[test]
    fn test_build_with_xref_stream_keeps_a_source_generation() {
        let (source, _) = generation_one_sources().remove(0);
        let doc = open(source, None);
        let modifier = DocumentModifier::from_document(&doc).unwrap();
        assert_info_at_generation(
            modifier.build_with_xref_stream(&[]).unwrap(),
            None,
            1,
            b"Generation one",
        );
    }

    #[test]
    fn test_pack_object_streams_leaves_a_source_generation_unpacked() {
        let (source, _) = generation_one_sources().remove(0);
        let doc = open(source, None);
        let info_num = doc.trailer().get_ref(b"Info").unwrap().obj_num;
        let mut modifier = DocumentModifier::from_document(&doc).unwrap();
        let compressed = modifier.pack_object_streams(100).unwrap();
        assert!(!compressed.is_empty());
        assert!(compressed.iter().all(|c| c.obj_num != info_num));
        assert_info_at_generation(
            modifier.build_with_xref_stream(&compressed).unwrap(),
            None,
            1,
            b"Generation one",
        );
    }

    #[test]
    fn test_pack_object_streams_numbers_containers_above_every_source_number() {
        let (source, _) = generation_one_sources().remove(0);
        let doc = open(source, None);
        let max_source_num = doc.object_refs().map(|r| r.obj_num).max().unwrap();
        let mut modifier = DocumentModifier::from_document(&doc).unwrap();
        modifier
            .writer
            .objects
            .retain(|(n, _)| *n != max_source_num);
        let compressed = modifier.pack_object_streams(100).unwrap();
        assert!(!compressed.is_empty());
        for c in &compressed {
            assert!(c.objstm_num > max_source_num, "{c:?}");
        }
        let next = modifier.writer.add_object(PdfObject::Null).obj_num;
        assert!(compressed.iter().all(|c| c.objstm_num < next), "{next}");
    }

    #[test]
    fn test_pack_object_streams_refuses_zero_objects_per_stream_and_keeps_the_writer() {
        let doc = PdfDocument::from_bytes(create_test_pdf("Zero", 2)).unwrap();
        let mut modifier = DocumentModifier::from_document(&doc).unwrap();
        let objects = modifier.writer.objects.clone();
        let next_obj_num = modifier.writer.next_obj_num;
        assert!(matches!(
            modifier.pack_object_streams(0),
            Err(crate::error::JustPdfError::InvalidObject { .. })
        ));
        assert_eq!(modifier.writer.objects, objects);
        assert_eq!(modifier.writer.next_obj_num, next_obj_num);
    }

    #[test]
    fn test_build_with_xref_stream_refuses_a_compressed_object_at_a_source_generation() {
        let (source, _) = generation_one_sources().remove(0);
        let doc = open(source, None);
        let info_num = doc.trailer().get_ref(b"Info").unwrap().obj_num;
        let mut modifier = DocumentModifier::from_document(&doc).unwrap();
        let packed = crate::writer::object_stream::pack_object_streams(
            &modifier.writer.objects,
            100,
            modifier.catalog_num,
            Some(modifier.find_pages_ref().unwrap().obj_num),
            None,
        )
        .unwrap();
        assert!(packed.compressed.iter().any(|c| c.obj_num == info_num));
        modifier.writer.objects = packed.objects;
        assert!(matches!(
            modifier.build_with_xref_stream(&packed.compressed),
            Err(crate::error::JustPdfError::InvalidObject { .. })
        ));
    }

    #[test]
    fn test_build_with_xref_stream_refuses_an_object_stream_at_a_source_generation() {
        let (source, _) = generation_one_sources().remove(0);
        let doc = open(source, None);
        let info_num = doc.trailer().get_ref(b"Info").unwrap().obj_num;
        let mut modifier = DocumentModifier::from_document(&doc).unwrap();
        let packed = crate::writer::object_stream::pack_object_streams(
            &[(1, PdfObject::Integer(7))],
            100,
            0,
            None,
            None,
        )
        .unwrap();
        let container = packed.objects.into_iter().next().unwrap().1;
        for (num, obj) in modifier.writer.objects.iter_mut() {
            if *num == info_num {
                *obj = container.clone();
            }
        }
        let compressed = [crate::writer::object_stream::CompressedObjInfo {
            obj_num: modifier.writer.next_obj_num,
            objstm_num: info_num,
            index: 0,
        }];
        assert!(matches!(
            modifier.build_with_xref_stream(&compressed),
            Err(crate::error::JustPdfError::InvalidObject { .. })
        ));
    }

    #[test]
    fn test_build_with_xref_stream_keeps_a_generation_wider_than_a_byte() {
        let source = with_info_at_generation(create_plain_pdf(None), None, 300);
        assert_info_at_generation(source.clone(), None, 300, b"Generation one");
        let doc = open(source, None);
        let modifier = DocumentModifier::from_document(&doc).unwrap();
        assert_info_at_generation(
            modifier.build_with_xref_stream(&[]).unwrap(),
            None,
            300,
            b"Generation one",
        );
    }

    #[test]
    fn test_incremental_save_keeps_a_source_generation() {
        for (source, password) in generation_one_sources() {
            let doc = open(source.clone(), password);
            let mut modifier = DocumentModifier::from_document(&doc).unwrap();
            modifier.set_info(b"Title", "Incremental");
            let bytes = incremental_save(&doc, modifier).unwrap();
            let appended = &bytes[source.len()..];
            let info_num = doc.trailer().get_ref(b"Info").unwrap().obj_num;
            let header = format!("{info_num} 1 obj");
            assert!(
                appended
                    .windows(header.len())
                    .any(|w| w == header.as_bytes()),
                "appended /Info is not `{header}`"
            );
            assert_info_at_generation(bytes, password, 1, b"Incremental");
        }
    }

    #[test]
    fn test_set_object_at_a_number_no_longer_held_is_generation_zero() {
        let (source, _) = generation_one_sources().remove(0);
        let doc = open(source, None);
        let info_num = doc.trailer().get_ref(b"Info").unwrap().obj_num;
        let mut modifier = DocumentModifier::from_document(&doc).unwrap();
        modifier.writer().objects.retain(|(n, _)| *n != info_num);
        modifier.set_object(info_num, PdfObject::String(b"new".to_vec()));
        let bytes = modifier.build().unwrap();
        let header = format!("\n{info_num} 0 obj");
        assert!(bytes.windows(header.len()).any(|w| w == header.as_bytes()));
        let reopened = PdfDocument::from_bytes(bytes).unwrap();
        assert!(matches!(
            reopened.xref.get(info_num),
            Some(crate::xref::XrefEntry::InUse { gen_num: 0, .. })
        ));
    }

    /// `create_plain_pdf(None)` with an update section that redefines its
    /// catalog, unchanged, at generation 1, the trailer's `/Root` reading
    /// `N 1 R`.
    fn with_catalog_at_generation_one() -> Vec<u8> {
        use std::io::Write;
        let source = create_plain_pdf(None);
        let doc = PdfDocument::from_bytes(source.clone()).unwrap();
        let catalog_num = doc.catalog_ref().unwrap().obj_num;
        let catalog = doc.resolve(doc.catalog_ref().unwrap()).unwrap();

        let mut buf = source.clone();
        let offset = buf.len();
        write!(buf, "{catalog_num} 1 obj\n").unwrap();
        crate::writer::serialize::serialize_object(&mut buf, &catalog).unwrap();
        write!(buf, "\nendobj\n").unwrap();
        let xref_offset = buf.len();
        write!(
            buf,
            "xref\n{catalog_num} 1\n{offset:010} 00001 n \ntrailer\n"
        )
        .unwrap();
        let trailer = incremental_trailer(
            doc.trailer(),
            catalog_num + 1,
            &IndirectRef {
                obj_num: catalog_num,
                gen_num: 1,
            },
            None,
            crate::xref::find_startxref(&source).unwrap(),
        );
        crate::writer::serialize::serialize_dict(&mut buf, &trailer).unwrap();
        write!(buf, "\nstartxref\n{xref_offset}\n%%EOF\n").unwrap();
        buf
    }

    /// The trailer's `/Root` of `bytes` resolves to a catalog.
    fn assert_root_is_a_catalog(bytes: Vec<u8>) {
        let doc = PdfDocument::from_bytes(bytes).unwrap();
        let root = doc.catalog_ref().unwrap().clone();
        match doc.resolve(&root).unwrap() {
            PdfObject::Dict(d) => assert_eq!(d.get_name(b"Type"), Some(b"Catalog".as_slice())),
            other => panic!("/Root {} {} R is {other:?}", root.obj_num, root.gen_num),
        }
    }

    #[test]
    fn test_catalog_at_generation_one_source_reads_its_root() {
        let bytes = with_catalog_at_generation_one();
        let doc = PdfDocument::from_bytes(bytes.clone()).unwrap();
        assert_eq!(doc.catalog_ref().unwrap().gen_num, 1);
        assert_root_is_a_catalog(bytes);
    }

    #[test]
    fn test_catalog_ref_is_the_root_the_modifier_writes() {
        let doc = PdfDocument::from_bytes(with_catalog_at_generation_one()).unwrap();
        let catalog_num = doc.catalog_ref().unwrap().obj_num;
        let catalog = doc.resolve(doc.catalog_ref().unwrap()).unwrap();
        let written_root = |modifier: DocumentModifier| {
            let bytes = modifier.build().unwrap();
            PdfDocument::from_bytes(bytes)
                .unwrap()
                .catalog_ref()
                .unwrap()
                .clone()
        };

        let held = DocumentModifier::from_document(&doc).unwrap();
        let reported = held.catalog_ref();
        assert_eq!(
            reported,
            IndirectRef {
                obj_num: catalog_num,
                gen_num: 1
            }
        );
        assert_eq!(reported, written_root(held));

        let mut set_again = DocumentModifier::from_document(&doc).unwrap();
        set_again
            .writer()
            .objects
            .retain(|(n, _)| *n != catalog_num);
        set_again.set_object(catalog_num, catalog);
        let reported = set_again.catalog_ref();
        assert_eq!(reported, written_root(set_again));
    }

    #[test]
    fn test_incremental_save_keeps_the_root_of_an_unchanged_catalog_at_generation_one() {
        let doc = PdfDocument::from_bytes(with_catalog_at_generation_one()).unwrap();
        let mut modifier = DocumentModifier::from_document(&doc).unwrap();
        modifier.set_info(b"Title", "Changed");
        let bytes = incremental_save(&doc, modifier).unwrap();
        let reopened = PdfDocument::from_bytes(bytes.clone()).unwrap();
        assert_eq!(reopened.catalog_ref().unwrap().gen_num, 1);
        assert_root_is_a_catalog(bytes);
    }

    #[test]
    fn test_set_info_when_the_source_info_is_not_held_is_read_through_the_trailer() {
        for (source, password) in generation_one_sources() {
            let doc = open(source, password);
            let info_num = doc.trailer().get_ref(b"Info").unwrap().obj_num;
            let make = || {
                let mut modifier = DocumentModifier::from_document(&doc).unwrap();
                modifier.writer().objects.retain(|(n, _)| *n != info_num);
                modifier.set_info(b"Title", "Rebuilt");
                modifier.preserve_encryption();
                modifier
            };
            let rewritten = open(make().build().unwrap(), password);
            assert_eq!(
                info_title(&rewritten),
                Some(PdfObject::String(b"Rebuilt".to_vec()))
            );
            let appended = open(incremental_save(&doc, make()).unwrap(), password);
            assert_eq!(
                info_title(&appended),
                Some(PdfObject::String(b"Rebuilt".to_vec()))
            );
        }
    }

    #[test]
    fn test_catalog_set_when_the_source_catalog_is_not_held_is_read_through_the_trailer() {
        let source = with_catalog_at_generation_one();
        let doc = PdfDocument::from_bytes(source).unwrap();
        let catalog_num = doc.catalog_ref().unwrap().obj_num;
        let catalog = doc.resolve(doc.catalog_ref().unwrap()).unwrap();
        let make = || {
            let mut modifier = DocumentModifier::from_document(&doc).unwrap();
            modifier.writer().objects.retain(|(n, _)| *n != catalog_num);
            modifier.set_object(catalog_num, catalog.clone());
            modifier
        };
        assert_root_is_a_catalog(make().build().unwrap());
        assert_root_is_a_catalog(make().build_with_xref_stream(&[]).unwrap());

        // The catalog set to its source value, and /Info changed.
        let mut modifier = make();
        modifier.set_info(b"Title", "Changed");
        let bytes = incremental_save(&doc, modifier).unwrap();
        assert_root_is_a_catalog(bytes.clone());
        assert_eq!(
            info_title(&PdfDocument::from_bytes(bytes).unwrap()),
            Some(PdfObject::String(b"Changed".to_vec()))
        );
    }

    #[test]
    fn test_add_object_after_from_document_is_generation_zero() {
        let (source, _) = generation_one_sources().remove(0);
        let doc = open(source, None);
        let mut modifier = DocumentModifier::from_document(&doc).unwrap();
        let added = modifier.add_object(PdfObject::String(b"added".to_vec()));
        assert_eq!(added.gen_num, 0);
        let text = format!("\n{} 0 obj", added.obj_num);
        let bytes = modifier.build().unwrap();
        assert!(bytes.windows(text.len()).any(|w| w == text.as_bytes()));
    }
}
