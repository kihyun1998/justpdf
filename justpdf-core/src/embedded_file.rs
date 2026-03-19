//! PDF embedded files (attachments) — PDF spec section 7.8.
//!
//! Supports reading embedded files from the catalog name tree, extracting
//! file data, and adding new embedded files to a document.

use md5::{Digest, Md5};

use crate::error::{JustPdfError, Result};
use crate::object::{IndirectRef, PdfDict, PdfObject};
use crate::parser::PdfDocument;
use crate::stream;
use crate::writer::encode::make_stream;
use crate::writer::modify::DocumentModifier;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A parsed file specification (PDF FileSpec dictionary).
#[derive(Debug, Clone)]
pub struct FileSpec {
    /// The filename (from /UF or /F).
    pub filename: String,
    /// Optional description (/Desc).
    pub description: Option<String>,
    /// MIME type (from the EF stream /Subtype, e.g. "application/pdf").
    pub mime_type: Option<String>,
    /// Uncompressed file size in bytes (/Params -> /Size).
    pub size: Option<usize>,
    /// MD5 checksum of the uncompressed data (/Params -> /CheckSum).
    pub checksum: Option<Vec<u8>>,
    /// Creation date string (/Params -> /CreationDate).
    pub creation_date: Option<String>,
    /// Modification date string (/Params -> /ModDate).
    pub mod_date: Option<String>,
    /// Reference to the embedded file stream object (/EF -> /F).
    pub ef_stream_ref: Option<IndirectRef>,
}

// ---------------------------------------------------------------------------
// Parsing helpers
// ---------------------------------------------------------------------------

/// Extract a UTF-8 string from a `PdfObject::String`.
fn obj_to_string(obj: &PdfObject) -> Option<String> {
    match obj {
        PdfObject::String(bytes) => {
            // Handle BOM-prefixed UTF-16BE strings
            if bytes.len() >= 2 && bytes[0] == 0xFE && bytes[1] == 0xFF {
                let chars: Vec<u16> = bytes[2..]
                    .chunks(2)
                    .filter_map(|c| {
                        if c.len() == 2 {
                            Some(u16::from_be_bytes([c[0], c[1]]))
                        } else {
                            None
                        }
                    })
                    .collect();
                String::from_utf16(&chars).ok()
            } else {
                Some(String::from_utf8_lossy(bytes).into_owned())
            }
        }
        _ => None,
    }
}

/// Parse a single FileSpec dictionary into a `FileSpec`.
fn parse_file_spec_dict(
    doc: &PdfDocument,
    dict: &PdfDict,
) -> Result<FileSpec> {
    // Filename: prefer /UF (Unicode), fall back to /F
    let filename = dict
        .get(b"UF")
        .and_then(obj_to_string)
        .or_else(|| dict.get(b"F").and_then(obj_to_string))
        .unwrap_or_default();

    let description = dict.get(b"Desc").and_then(obj_to_string);

    // /EF dict -> /F (reference to the embedded file stream)
    let mut ef_stream_ref: Option<IndirectRef> = None;
    let mut mime_type: Option<String> = None;
    let mut size: Option<usize> = None;
    let mut checksum: Option<Vec<u8>> = None;
    let mut creation_date: Option<String> = None;
    let mut mod_date: Option<String> = None;

    if let Some(ef_dict) = resolve_dict(doc, dict, b"EF")? {
        // The /F entry inside /EF is a reference to the embedded stream
        if let Some(r) = ef_dict.get_ref(b"F") {
            let stream_ref = r.clone();

            // Resolve the stream to extract params
            if let Ok(stream_obj) = doc.resolve(&stream_ref) {
                if let PdfObject::Stream { dict: s_dict, .. } = stream_obj {
                    // MIME type from /Subtype
                    if let Some(name) = s_dict.get_name(b"Subtype") {
                        let raw = String::from_utf8_lossy(name).into_owned();
                        // PDF uses #2F for '/' in names
                        mime_type = Some(raw.replace("#2F", "/"));
                    }

                    // /Params sub-dictionary
                    if let Some(params) = s_dict.get_dict(b"Params") {
                        size = params.get_i64(b"Size").map(|v| v as usize);
                        checksum = params.get_string(b"CheckSum").map(|b| b.to_vec());
                        creation_date = params.get(b"CreationDate").and_then(obj_to_string);
                        mod_date = params.get(b"ModDate").and_then(obj_to_string);
                    }
                }
            }

            ef_stream_ref = Some(stream_ref);
        }
    }

    Ok(FileSpec {
        filename,
        description,
        mime_type,
        size,
        checksum,
        creation_date,
        mod_date,
        ef_stream_ref,
    })
}

/// Resolve a dict entry that might be an indirect reference to a dict.
fn resolve_dict<'a>(
    doc: &'a PdfDocument,
    parent: &PdfDict,
    key: &[u8],
) -> Result<Option<PdfDict>> {
    match parent.get(key) {
        Some(PdfObject::Dict(d)) => Ok(Some(d.clone())),
        Some(PdfObject::Reference(r)) => {
            let r = r.clone();
            let obj = doc.resolve(&r)?;
            match obj {
                PdfObject::Dict(d) => Ok(Some(d)),
                _ => Ok(None),
            }
        }
        _ => Ok(None),
    }
}

// ---------------------------------------------------------------------------
// Parsing: read embedded files from catalog
// ---------------------------------------------------------------------------

/// Read all embedded file specifications from the document catalog.
///
/// Parses the Catalog -> /Names -> /EmbeddedFiles name tree and returns
/// a `Vec<FileSpec>` for each attachment found.
pub fn read_embedded_files(doc: &PdfDocument) -> Result<Vec<FileSpec>> {
    // Get catalog
    let catalog_ref = match doc.catalog_ref() {
        Some(r) => r.clone(),
        None => return Ok(Vec::new()),
    };
    let catalog = match doc.resolve(&catalog_ref)? {
        PdfObject::Dict(d) => d,
        _ => return Ok(Vec::new()),
    };

    // Catalog -> /Names
    let names_dict = match resolve_dict(doc, &catalog, b"Names")? {
        Some(d) => d,
        None => return Ok(Vec::new()),
    };

    // /Names -> /EmbeddedFiles (name tree root)
    let ef_tree = match resolve_dict(doc, &names_dict, b"EmbeddedFiles")? {
        Some(d) => d,
        None => return Ok(Vec::new()),
    };

    // Collect leaf values from the name tree
    let mut file_specs = Vec::new();
    collect_name_tree_values(doc, &ef_tree, &mut file_specs)?;

    Ok(file_specs)
}

/// Recursively collect FileSpec values from a name tree node.
fn collect_name_tree_values(
    doc: &PdfDocument,
    node: &PdfDict,
    out: &mut Vec<FileSpec>,
) -> Result<()> {
    // Leaf node: /Names array of [name1, value1, name2, value2, ...]
    if let Some(names_arr) = node.get_array(b"Names") {
        let pairs: Vec<PdfObject> = names_arr.to_vec();
        let mut i = 0;
        while i + 1 < pairs.len() {
            // pairs[i] is the name key (string), pairs[i+1] is the value (dict or ref)
            let value = &pairs[i + 1];
            let fs_dict = match value {
                PdfObject::Dict(d) => Some(d.clone()),
                PdfObject::Reference(r) => {
                    let r = r.clone();
                    match doc.resolve(&r)? {
                        PdfObject::Dict(d) => Some(d),
                        _ => None,
                    }
                }
                _ => None,
            };
            if let Some(d) = fs_dict {
                out.push(parse_file_spec_dict(doc, &d)?);
            }
            i += 2;
        }
    }

    // Intermediate node: /Kids array of child node references
    if let Some(kids_arr) = node.get_array(b"Kids") {
        let kids: Vec<PdfObject> = kids_arr.to_vec();
        for kid in &kids {
            if let PdfObject::Reference(r) = kid {
                let r = r.clone();
                let child = doc.resolve(&r)?;
                if let PdfObject::Dict(d) = child {
                    collect_name_tree_values(doc, &d, out)?;
                }
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Extraction
// ---------------------------------------------------------------------------

/// Extract the raw (decoded) file data for an embedded file.
///
/// Resolves the EF stream reference, decodes the stream through its filter
/// chain, and optionally verifies the MD5 checksum when present.
pub fn extract_file(doc: &PdfDocument, file_spec: &FileSpec) -> Result<Vec<u8>> {
    let stream_ref = file_spec.ef_stream_ref.as_ref().ok_or_else(|| {
        JustPdfError::StreamDecode {
            filter: String::new(),
            detail: "FileSpec has no embedded file stream reference".into(),
        }
    })?;

    let stream_obj = doc.resolve(stream_ref)?;
    let (dict, raw_data) = match &stream_obj {
        PdfObject::Stream { dict, data } => (dict, data.as_slice()),
        _ => {
            return Err(JustPdfError::StreamDecode {
                filter: String::new(),
                detail: "EF stream reference does not point to a stream object".into(),
            });
        }
    };

    let decoded = stream::decode_stream(raw_data, dict)?;

    // Verify checksum if present
    if let Some(expected) = &file_spec.checksum {
        let mut hasher = Md5::new();
        hasher.update(&decoded);
        let computed = hasher.finalize();
        if computed.as_slice() != expected.as_slice() {
            return Err(JustPdfError::StreamDecode {
                filter: String::new(),
                detail: "embedded file MD5 checksum mismatch".into(),
            });
        }
    }

    Ok(decoded)
}

// ---------------------------------------------------------------------------
// Builder: add embedded file
// ---------------------------------------------------------------------------

/// Add an embedded file (attachment) to the document.
///
/// Creates the embedded file stream, a FileSpec dictionary, and wires it
/// into the Catalog -> /Names -> /EmbeddedFiles name tree. Returns the
/// `IndirectRef` of the new FileSpec dictionary object.
pub fn add_embedded_file(
    modifier: &mut DocumentModifier,
    filename: &str,
    data: &[u8],
    mime_type: Option<&str>,
    description: Option<&str>,
) -> Result<IndirectRef> {
    // 1. Compute MD5 checksum of uncompressed data
    let mut hasher = Md5::new();
    hasher.update(data);
    let checksum = hasher.finalize().to_vec();

    // 2. Build the embedded file stream with FlateDecode compression
    let (mut stream_dict, compressed) = make_stream(data, true);

    // /Type /EmbeddedFile
    stream_dict.insert(b"Type".to_vec(), PdfObject::Name(b"EmbeddedFile".to_vec()));

    // /Subtype (MIME type encoded as a PDF name, with '/' -> '#2F')
    if let Some(mt) = mime_type {
        let name_encoded = mt.replace('/', "#2F");
        stream_dict.insert(
            b"Subtype".to_vec(),
            PdfObject::Name(name_encoded.into_bytes()),
        );
    }

    // /Params dict
    let mut params = PdfDict::new();
    params.insert(b"Size".to_vec(), PdfObject::Integer(data.len() as i64));
    params.insert(b"CheckSum".to_vec(), PdfObject::String(checksum));
    stream_dict.insert(b"Params".to_vec(), PdfObject::Dict(params));

    let stream_ref = modifier.add_object(PdfObject::Stream {
        dict: stream_dict,
        data: compressed,
    });

    // 3. Build the FileSpec dictionary
    let mut fs_dict = PdfDict::new();
    fs_dict.insert(b"Type".to_vec(), PdfObject::Name(b"Filespec".to_vec()));
    fs_dict.insert(
        b"F".to_vec(),
        PdfObject::String(filename.as_bytes().to_vec()),
    );
    fs_dict.insert(
        b"UF".to_vec(),
        PdfObject::String(filename.as_bytes().to_vec()),
    );

    if let Some(desc) = description {
        fs_dict.insert(
            b"Desc".to_vec(),
            PdfObject::String(desc.as_bytes().to_vec()),
        );
    }

    // /EF << /F stream_ref >>
    let mut ef_dict = PdfDict::new();
    ef_dict.insert(
        b"F".to_vec(),
        PdfObject::Reference(stream_ref),
    );
    fs_dict.insert(b"EF".to_vec(), PdfObject::Dict(ef_dict));

    let fs_ref = modifier.add_object(PdfObject::Dict(fs_dict));

    // 4. Wire into Catalog -> /Names -> /EmbeddedFiles
    wire_into_name_tree(modifier, filename, &fs_ref)?;

    Ok(fs_ref)
}

/// Ensure the catalog has a /Names -> /EmbeddedFiles name tree and append
/// the new entry to it.
fn wire_into_name_tree(
    modifier: &mut DocumentModifier,
    filename: &str,
    fs_ref: &IndirectRef,
) -> Result<()> {
    let catalog_obj_num = modifier.catalog_ref().obj_num;

    // Load catalog dict
    let mut catalog = match modifier.find_object_pub(catalog_obj_num) {
        Some(PdfObject::Dict(d)) => d.clone(),
        _ => PdfDict::new(),
    };

    // Get or create /Names dict
    let (names_obj_num, mut names_dict) = match catalog.get(b"Names") {
        Some(PdfObject::Reference(r)) => {
            let num = r.obj_num;
            match modifier.find_object_pub(num) {
                Some(PdfObject::Dict(d)) => (Some(num), d.clone()),
                _ => (Some(num), PdfDict::new()),
            }
        }
        Some(PdfObject::Dict(d)) => (None, d.clone()),
        _ => (None, PdfDict::new()),
    };

    // Get or create /EmbeddedFiles name tree root
    let (ef_obj_num, mut ef_dict) = match names_dict.get(b"EmbeddedFiles") {
        Some(PdfObject::Reference(r)) => {
            let num = r.obj_num;
            match modifier.find_object_pub(num) {
                Some(PdfObject::Dict(d)) => (Some(num), d.clone()),
                _ => (Some(num), PdfDict::new()),
            }
        }
        Some(PdfObject::Dict(d)) => (None, d.clone()),
        _ => (None, PdfDict::new()),
    };

    // Append to the /Names array inside the EmbeddedFiles tree root
    let mut names_arr = match ef_dict.get(b"Names") {
        Some(PdfObject::Array(a)) => a.clone(),
        _ => Vec::new(),
    };
    names_arr.push(PdfObject::String(filename.as_bytes().to_vec()));
    names_arr.push(PdfObject::Reference(fs_ref.clone()));
    ef_dict.insert(b"Names".to_vec(), PdfObject::Array(names_arr));

    // Store the EmbeddedFiles dict (as indirect or inline)
    match ef_obj_num {
        Some(num) => {
            modifier.set_object(num, PdfObject::Dict(ef_dict));
        }
        None => {
            let ef_ref = modifier.add_object(PdfObject::Dict(ef_dict));
            names_dict.insert(
                b"EmbeddedFiles".to_vec(),
                PdfObject::Reference(ef_ref),
            );
        }
    }

    // Store the Names dict
    match names_obj_num {
        Some(num) => {
            modifier.set_object(num, PdfObject::Dict(names_dict));
        }
        None => {
            let names_ref = modifier.add_object(PdfObject::Dict(names_dict));
            catalog.insert(b"Names".to_vec(), PdfObject::Reference(names_ref));
        }
    }

    // Update catalog
    modifier.set_object(catalog_obj_num, PdfObject::Dict(catalog));

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: create a FileSpec from a manually constructed dict.
    fn make_sample_fs_dict(
        filename: &str,
        desc: Option<&str>,
        stream_ref: Option<IndirectRef>,
    ) -> PdfDict {
        let mut dict = PdfDict::new();
        dict.insert(b"Type".to_vec(), PdfObject::Name(b"Filespec".to_vec()));
        dict.insert(
            b"UF".to_vec(),
            PdfObject::String(filename.as_bytes().to_vec()),
        );
        dict.insert(
            b"F".to_vec(),
            PdfObject::String(filename.as_bytes().to_vec()),
        );

        if let Some(d) = desc {
            dict.insert(
                b"Desc".to_vec(),
                PdfObject::String(d.as_bytes().to_vec()),
            );
        }

        if let Some(sr) = stream_ref {
            let mut ef = PdfDict::new();
            ef.insert(b"F".to_vec(), PdfObject::Reference(sr));
            dict.insert(b"EF".to_vec(), PdfObject::Dict(ef));
        }

        dict
    }

    #[test]
    fn test_parse_file_spec_minimal() {
        // Minimal FileSpec: just a filename
        let mut dict = PdfDict::new();
        dict.insert(
            b"F".to_vec(),
            PdfObject::String(b"report.pdf".to_vec()),
        );

        // We cannot call parse_file_spec_dict without a PdfDocument, so test
        // the helper `obj_to_string` and dict access directly.
        let filename = dict
            .get(b"UF")
            .and_then(obj_to_string)
            .or_else(|| dict.get(b"F").and_then(obj_to_string))
            .unwrap_or_default();

        assert_eq!(filename, "report.pdf");
        assert!(dict.get(b"Desc").is_none());
        assert!(dict.get(b"EF").is_none());
    }

    #[test]
    fn test_parse_file_spec_all_fields() {
        let dict = make_sample_fs_dict(
            "attachment.txt",
            Some("A text attachment"),
            Some(IndirectRef { obj_num: 42, gen_num: 0 }),
        );

        // Filename from /UF
        let filename = dict
            .get(b"UF")
            .and_then(obj_to_string)
            .unwrap();
        assert_eq!(filename, "attachment.txt");

        // Description
        let desc = dict.get(b"Desc").and_then(obj_to_string).unwrap();
        assert_eq!(desc, "A text attachment");

        // EF -> F reference
        let ef = dict.get_dict(b"EF").unwrap();
        let stream_ref = ef.get_ref(b"F").unwrap();
        assert_eq!(stream_ref.obj_num, 42);
        assert_eq!(stream_ref.gen_num, 0);
    }

    #[test]
    fn test_empty_embedded_files_list() {
        // An empty name tree /Names array should yield no results.
        let mut ef_tree = PdfDict::new();
        ef_tree.insert(b"Names".to_vec(), PdfObject::Array(Vec::new()));

        // Manually test with empty names array — no pairs means no results.
        let names_arr = ef_tree.get_array(b"Names").unwrap();
        assert!(names_arr.is_empty());
    }

    #[test]
    fn test_file_spec_struct_defaults() {
        let fs = FileSpec {
            filename: "test.pdf".into(),
            description: None,
            mime_type: None,
            size: None,
            checksum: None,
            creation_date: None,
            mod_date: None,
            ef_stream_ref: None,
        };

        assert_eq!(fs.filename, "test.pdf");
        assert!(fs.description.is_none());
        assert!(fs.mime_type.is_none());
        assert!(fs.size.is_none());
        assert!(fs.checksum.is_none());
        assert!(fs.creation_date.is_none());
        assert!(fs.mod_date.is_none());
        assert!(fs.ef_stream_ref.is_none());
    }

    #[test]
    fn test_file_spec_struct_all_populated() {
        let checksum = vec![0xAB, 0xCD, 0xEF, 0x01];
        let fs = FileSpec {
            filename: "data.csv".into(),
            description: Some("CSV export".into()),
            mime_type: Some("text/csv".into()),
            size: Some(1024),
            checksum: Some(checksum.clone()),
            creation_date: Some("D:20260101120000".into()),
            mod_date: Some("D:20260315090000".into()),
            ef_stream_ref: Some(IndirectRef { obj_num: 99, gen_num: 0 }),
        };

        assert_eq!(fs.filename, "data.csv");
        assert_eq!(fs.description.as_deref(), Some("CSV export"));
        assert_eq!(fs.mime_type.as_deref(), Some("text/csv"));
        assert_eq!(fs.size, Some(1024));
        assert_eq!(fs.checksum.as_deref(), Some(checksum.as_slice()));
        assert_eq!(fs.creation_date.as_deref(), Some("D:20260101120000"));
        assert_eq!(fs.mod_date.as_deref(), Some("D:20260315090000"));
        assert_eq!(fs.ef_stream_ref.as_ref().unwrap().obj_num, 99);
    }

    #[test]
    fn test_obj_to_string_latin() {
        let obj = PdfObject::String(b"hello.txt".to_vec());
        assert_eq!(obj_to_string(&obj), Some("hello.txt".into()));
    }

    #[test]
    fn test_obj_to_string_utf16be() {
        // BOM (FE FF) + "AB" in UTF-16BE
        let bytes = vec![0xFE, 0xFF, 0x00, 0x41, 0x00, 0x42];
        let obj = PdfObject::String(bytes);
        assert_eq!(obj_to_string(&obj), Some("AB".into()));
    }

    #[test]
    fn test_obj_to_string_non_string() {
        let obj = PdfObject::Integer(42);
        assert_eq!(obj_to_string(&obj), None);
    }

    #[test]
    fn test_mime_type_name_encoding() {
        // Verify MIME name encoding roundtrip ('#2F' <-> '/')
        let mime = "application/pdf";
        let encoded = mime.replace('/', "#2F");
        assert_eq!(encoded, "application#2Fpdf");
        let decoded = encoded.replace("#2F", "/");
        assert_eq!(decoded, mime);
    }

    #[test]
    fn test_md5_checksum_computation() {
        let data = b"Hello, embedded file!";
        let mut hasher = Md5::new();
        hasher.update(data);
        let digest = hasher.finalize();

        // MD5 produces 16 bytes
        assert_eq!(digest.len(), 16);

        // Same input should yield same digest
        let mut hasher2 = Md5::new();
        hasher2.update(data);
        let digest2 = hasher2.finalize();
        assert_eq!(digest.as_slice(), digest2.as_slice());
    }

    #[test]
    fn test_make_sample_fs_dict_structure() {
        let dict = make_sample_fs_dict("test.pdf", Some("Test"), None);

        assert_eq!(dict.get_name(b"Type"), Some(b"Filespec".as_slice()));
        assert_eq!(
            dict.get_string(b"UF"),
            Some(b"test.pdf".as_slice())
        );
        assert_eq!(
            dict.get_string(b"F"),
            Some(b"test.pdf".as_slice())
        );
        assert_eq!(
            dict.get_string(b"Desc"),
            Some(b"Test".as_slice())
        );
        assert!(dict.get(b"EF").is_none());
    }

    #[test]
    fn test_make_sample_fs_dict_with_ef() {
        let dict = make_sample_fs_dict(
            "data.bin",
            None,
            Some(IndirectRef { obj_num: 7, gen_num: 0 }),
        );

        assert!(dict.get(b"Desc").is_none());
        let ef = dict.get_dict(b"EF").unwrap();
        let r = ef.get_ref(b"F").unwrap();
        assert_eq!(r.obj_num, 7);
    }
}
