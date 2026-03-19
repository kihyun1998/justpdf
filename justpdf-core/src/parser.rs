use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::crypto;
use crate::crypto::SecurityState;
use crate::error::{JustPdfError, Result};
use crate::object::{self, IndirectRef, PdfDict, PdfObject};
use crate::stream;
use crate::tokenizer::Tokenizer;
use crate::xref::{self, Xref, XrefEntry};

/// A parsed PDF document.
#[derive(Debug)]
pub struct PdfDocument {
    /// PDF version, e.g. (1, 7) for PDF 1.7.
    pub version: (u8, u8),
    /// The merged cross-reference table.
    pub xref: Xref,
    /// Raw file data.
    data: Vec<u8>,
    /// Cache of parsed objects.
    objects: HashMap<IndirectRef, PdfObject>,
    /// Encryption/security state (None if document is not encrypted).
    security: Option<SecurityState>,
}

impl PdfDocument {
    /// Open a PDF file from a path.
    pub fn open(path: &Path) -> Result<Self> {
        let data = std::fs::read(path)?;
        Self::from_bytes(data)
    }

    /// Parse a PDF from an in-memory byte vector.
    pub fn from_bytes(data: Vec<u8>) -> Result<Self> {
        if data.len() < 8 {
            return Err(JustPdfError::NotPdf);
        }

        // Parse version from header: %PDF-X.Y
        let version = parse_version(&data)?;

        // Load xref
        let xref = xref::load_xref(&data)?;

        let mut doc = Self {
            version,
            xref,
            data,
            objects: HashMap::new(),
            security: None,
        };

        // Detect encryption
        doc.detect_encryption()?;

        Ok(doc)
    }

    /// Detect and initialize encryption from the trailer.
    fn detect_encryption(&mut self) -> Result<()> {
        // Check for /Encrypt in trailer
        let encrypt_ref = match self.xref.trailer.get_ref(b"Encrypt") {
            Some(r) => r.clone(),
            None => {
                // Also check for inline /Encrypt dict
                if self.xref.trailer.get_dict(b"Encrypt").is_some() {
                    return self.detect_encryption_inline();
                }
                return Ok(());
            }
        };

        // Load the encryption dictionary object (without decryption!)
        let encrypt_obj = self.load_object_raw(&encrypt_ref, &mut HashSet::new())?;
        let encrypt_dict = match &encrypt_obj {
            PdfObject::Dict(d) => d,
            _ => {
                return Err(JustPdfError::EncryptionError {
                    detail: "encryption object is not a dictionary".into(),
                });
            }
        };

        let ed = crypto::EncryptionDict::from_dict(encrypt_dict)?;

        // Verify we support this encryption
        if ed.filter != b"Standard" {
            return Err(JustPdfError::UnsupportedEncryption {
                detail: format!(
                    "unsupported security handler: {}",
                    String::from_utf8_lossy(&ed.filter)
                ),
            });
        }

        // Extract file ID from trailer
        let file_id = self.extract_file_id();

        let mut state =
            SecurityState::new(ed, file_id, Some(encrypt_ref.obj_num));

        // Try empty password (very common for user-password-only PDFs)
        if let Ok(key) = crypto::auth::authenticate(&state, b"") {
            state.file_key = Some(key);
        }

        self.security = Some(state);
        Ok(())
    }

    /// Handle inline /Encrypt dict (not an indirect reference).
    fn detect_encryption_inline(&mut self) -> Result<()> {
        let encrypt_dict = self.xref.trailer.get_dict(b"Encrypt").unwrap().clone();
        let ed = crypto::EncryptionDict::from_dict(&encrypt_dict)?;

        if ed.filter != b"Standard" {
            return Err(JustPdfError::UnsupportedEncryption {
                detail: format!(
                    "unsupported security handler: {}",
                    String::from_utf8_lossy(&ed.filter)
                ),
            });
        }

        let file_id = self.extract_file_id();
        let mut state = SecurityState::new(ed, file_id, None);

        if let Ok(key) = crypto::auth::authenticate(&state, b"") {
            state.file_key = Some(key);
        }

        self.security = Some(state);
        Ok(())
    }

    /// Extract the first element of the /ID array from the trailer.
    fn extract_file_id(&self) -> Vec<u8> {
        if let Some(PdfObject::Array(arr)) = self.xref.trailer.get(b"ID") {
            if let Some(PdfObject::String(id)) = arr.first() {
                return id.clone();
            }
        }
        Vec::new()
    }

    /// Whether the document is encrypted.
    pub fn is_encrypted(&self) -> bool {
        self.security.is_some()
    }

    /// Whether the document is encrypted and authentication has succeeded.
    pub fn is_authenticated(&self) -> bool {
        match &self.security {
            Some(s) => s.is_authenticated(),
            None => true, // Not encrypted = always accessible
        }
    }

    /// Authenticate with a password. Required for encrypted documents
    /// where the empty password doesn't work.
    pub fn authenticate(&mut self, password: &[u8]) -> Result<()> {
        let state = match &mut self.security {
            Some(s) => s,
            None => return Ok(()), // Not encrypted
        };

        if state.is_authenticated() {
            return Ok(()); // Already authenticated
        }

        let key = crypto::auth::authenticate(state, password)?;
        state.file_key = Some(key);

        // Clear cached objects — they need to be re-loaded with decryption
        self.objects.clear();

        Ok(())
    }

    /// Get the permission flags (if encrypted).
    pub fn permissions(&self) -> Option<crypto::Permissions> {
        self.security.as_ref().map(|s| s.permissions())
    }

    /// Get the security state (for advanced use).
    pub fn security_state(&self) -> Option<&SecurityState> {
        self.security.as_ref()
    }

    /// Number of objects declared in xref.
    pub fn object_count(&self) -> usize {
        self.xref.len()
    }

    /// The /Root (catalog) reference from the trailer.
    pub fn catalog_ref(&self) -> Option<&IndirectRef> {
        self.xref.trailer.get_ref(b"Root")
    }

    /// Get the trailer dictionary.
    pub fn trailer(&self) -> &PdfDict {
        &self.xref.trailer
    }

    /// Resolve an indirect reference to the actual object.
    /// Uses internal cache. Detects circular references.
    /// Automatically decrypts if the document is encrypted and authenticated.
    pub fn resolve(&mut self, iref: &IndirectRef) -> Result<&PdfObject> {
        if self.objects.contains_key(iref) {
            return Ok(self.objects.get(iref).unwrap());
        }

        // Check if we need authentication
        if let Some(ref sec) = self.security {
            if !sec.is_authenticated() {
                return Err(JustPdfError::EncryptedDocument);
            }
        }

        // Load the object
        let obj = self.load_object(iref, &mut HashSet::new())?;
        self.objects.insert(iref.clone(), obj);
        Ok(self.objects.get(iref).unwrap())
    }

    /// Load an object, tracking visited refs to detect cycles.
    /// Applies decryption if the document is encrypted.
    fn load_object(
        &self,
        iref: &IndirectRef,
        visited: &mut HashSet<IndirectRef>,
    ) -> Result<PdfObject> {
        let obj = self.load_object_raw(iref, visited)?;

        // Apply decryption if needed
        if let Some(ref sec) = self.security {
            if sec.is_authenticated() {
                return crypto::decrypt_object(obj, sec, iref.obj_num, iref.gen_num);
            }
        }

        Ok(obj)
    }

    /// Load an object without decryption (used for the encryption dict itself).
    fn load_object_raw(
        &self,
        iref: &IndirectRef,
        visited: &mut HashSet<IndirectRef>,
    ) -> Result<PdfObject> {
        if !visited.insert(iref.clone()) {
            return Err(JustPdfError::CircularReference {
                obj_num: iref.obj_num,
                gen_num: iref.gen_num,
            });
        }

        let entry = self
            .xref
            .get(iref.obj_num)
            .ok_or(JustPdfError::ObjectNotFound {
                obj_num: iref.obj_num,
                gen_num: iref.gen_num,
            })?
            .clone();

        match entry {
            XrefEntry::InUse { offset, .. } => {
                let mut tokenizer = Tokenizer::new_at(&self.data, offset as usize);
                let (_parsed_ref, obj) = object::parse_indirect_object(&mut tokenizer)?;
                Ok(obj)
            }
            XrefEntry::Compressed {
                obj_stream_num,
                index_within,
            } => self.load_compressed_object(obj_stream_num, index_within, visited),
            XrefEntry::Free { .. } => Ok(PdfObject::Null),
        }
    }

    /// Load an object from a compressed object stream.
    fn load_compressed_object(
        &self,
        obj_stream_num: u32,
        index_within: u16,
        visited: &mut HashSet<IndirectRef>,
    ) -> Result<PdfObject> {
        let stream_ref = IndirectRef {
            obj_num: obj_stream_num,
            gen_num: 0,
        };

        // Load the object stream itself (which may need decryption)
        let stream_obj = {
            let raw = self.load_object_raw(&stream_ref, visited)?;
            // Decrypt the object stream if needed
            if let Some(ref sec) = self.security {
                if sec.is_authenticated() {
                    crypto::decrypt_object(raw, sec, obj_stream_num, 0)?
                } else {
                    raw
                }
            } else {
                raw
            }
        };

        let (dict, raw_data) = match &stream_obj {
            PdfObject::Stream { dict, data } => (dict, data),
            _ => {
                return Err(JustPdfError::InvalidObject {
                    offset: 0,
                    detail: format!("object stream {obj_stream_num} is not a stream"),
                });
            }
        };

        let decoded = stream::decode_stream(raw_data, dict)?;
        let n = dict.get_i64(b"N").unwrap_or(0) as usize;
        let first = dict.get_i64(b"First").unwrap_or(0) as usize;

        // Parse the N pairs of (obj_num, offset) from the beginning
        let mut tokenizer = Tokenizer::new(&decoded);
        let mut obj_offsets = Vec::with_capacity(n);
        for _ in 0..n {
            let obj_num = match tokenizer.next_token()? {
                Some(crate::tokenizer::token::Token::Integer(v)) => v as u32,
                _ => break,
            };
            let offset = match tokenizer.next_token()? {
                Some(crate::tokenizer::token::Token::Integer(v)) => v as usize,
                _ => break,
            };
            obj_offsets.push((obj_num, offset));
        }

        // Find the object at index_within
        let idx = index_within as usize;
        if idx >= obj_offsets.len() {
            return Err(JustPdfError::ObjectNotFound {
                obj_num: 0,
                gen_num: 0,
            });
        }

        let (_obj_num, obj_offset) = obj_offsets[idx];
        let abs_offset = first + obj_offset;

        let mut tokenizer = Tokenizer::new_at(&decoded, abs_offset);
        object::parse_object(&mut tokenizer)
    }

    /// Iterate over all in-use object references.
    pub fn object_refs(&self) -> impl Iterator<Item = IndirectRef> + '_ {
        self.xref
            .entries
            .iter()
            .filter_map(|(&obj_num, entry)| match entry {
                XrefEntry::InUse { gen_num, .. } => Some(IndirectRef {
                    obj_num,
                    gen_num: *gen_num,
                }),
                XrefEntry::Compressed { .. } => Some(IndirectRef {
                    obj_num,
                    gen_num: 0,
                }),
                XrefEntry::Free { .. } => None,
            })
    }

    /// Decode a stream object's data.
    pub fn decode_stream(&self, dict: &PdfDict, raw_data: &[u8]) -> Result<Vec<u8>> {
        stream::decode_stream(raw_data, dict)
    }

    /// Get the raw file data.
    pub fn raw_data(&self) -> &[u8] {
        &self.data
    }
}

/// Parse PDF version from the header line.
fn parse_version(data: &[u8]) -> Result<(u8, u8)> {
    // Look for %PDF-X.Y in the first 1024 bytes
    let search_len = data.len().min(1024);
    let needle = b"%PDF-";

    for i in 0..search_len.saturating_sub(needle.len() + 3) {
        if &data[i..i + needle.len()] == needle {
            let major = data.get(i + 5).copied().unwrap_or(0);
            let dot = data.get(i + 6).copied().unwrap_or(0);
            let minor = data.get(i + 7).copied().unwrap_or(0);

            if major.is_ascii_digit() && dot == b'.' && minor.is_ascii_digit() {
                return Ok((major - b'0', minor - b'0'));
            }
        }
    }

    Err(JustPdfError::NotPdf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_version() {
        assert_eq!(parse_version(b"%PDF-1.7\n").unwrap(), (1, 7));
        assert_eq!(parse_version(b"%PDF-2.0\n").unwrap(), (2, 0));
        assert_eq!(parse_version(b"%PDF-1.4 stuff").unwrap(), (1, 4));
    }

    #[test]
    fn test_parse_version_not_pdf() {
        assert!(parse_version(b"Hello World").is_err());
        assert!(parse_version(b"").is_err());
    }

    #[test]
    fn test_parse_version_offset() {
        // Some PDFs have garbage before %PDF-
        assert_eq!(parse_version(b"\xEF\xBB\xBF%PDF-1.7\n").unwrap(), (1, 7));
    }

    /// Build a minimal valid PDF in memory for testing.
    fn build_minimal_pdf() -> Vec<u8> {
        let mut pdf = Vec::new();
        // Header
        pdf.extend_from_slice(b"%PDF-1.4\n");

        // Object 1: Catalog
        let obj1_offset = pdf.len();
        pdf.extend_from_slice(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n");

        // Object 2: Pages
        let obj2_offset = pdf.len();
        pdf.extend_from_slice(b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n");

        // Object 3: Page
        let obj3_offset = pdf.len();
        pdf.extend_from_slice(
            b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>\nendobj\n",
        );

        // Xref table
        let xref_offset = pdf.len();
        pdf.extend_from_slice(b"xref\n");
        pdf.extend_from_slice(b"0 4\n");
        pdf.extend_from_slice(b"0000000000 65535 f \r\n");
        pdf.extend_from_slice(format!("{:010} 00000 n \r\n", obj1_offset).as_bytes());
        pdf.extend_from_slice(format!("{:010} 00000 n \r\n", obj2_offset).as_bytes());
        pdf.extend_from_slice(format!("{:010} 00000 n \r\n", obj3_offset).as_bytes());

        // Trailer
        pdf.extend_from_slice(b"trailer\n<< /Size 4 /Root 1 0 R >>\n");
        pdf.extend_from_slice(format!("startxref\n{xref_offset}\n%%EOF\n").as_bytes());

        pdf
    }

    #[test]
    fn test_open_minimal_pdf() {
        let data = build_minimal_pdf();
        let mut doc = PdfDocument::from_bytes(data).unwrap();

        assert_eq!(doc.version, (1, 4));
        assert!(doc.object_count() > 0);
        assert!(!doc.is_encrypted());

        // Resolve catalog
        let catalog_ref = doc.catalog_ref().unwrap().clone();
        let catalog = doc.resolve(&catalog_ref).unwrap();
        match catalog {
            PdfObject::Dict(d) => {
                assert_eq!(d.get_name(b"Type"), Some(b"Catalog".as_slice()));
            }
            _ => panic!("expected dict for catalog"),
        }
    }

    #[test]
    fn test_not_pdf() {
        let result = PdfDocument::from_bytes(b"Hello World, not a PDF".to_vec());
        assert!(result.is_err());
    }

    #[test]
    fn test_empty_file() {
        let result = PdfDocument::from_bytes(vec![]);
        assert!(result.is_err());
    }

    #[test]
    fn test_truncated_pdf() {
        let result = PdfDocument::from_bytes(b"%PDF-1.4\n".to_vec());
        assert!(result.is_err());
    }

    #[test]
    fn test_object_not_found() {
        let data = build_minimal_pdf();
        let mut doc = PdfDocument::from_bytes(data).unwrap();
        let result = doc.resolve(&IndirectRef {
            obj_num: 999,
            gen_num: 0,
        });
        assert!(result.is_err());
    }

    #[test]
    fn test_unencrypted_pdf_is_authenticated() {
        let data = build_minimal_pdf();
        let doc = PdfDocument::from_bytes(data).unwrap();
        assert!(!doc.is_encrypted());
        assert!(doc.is_authenticated());
    }
}
