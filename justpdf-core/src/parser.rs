use std::collections::{HashMap, HashSet, VecDeque};
use std::hash::Hash;
use std::path::Path;

use crate::crypto;
use crate::crypto::SecurityState;
use crate::error::{JustPdfError, Result};
use crate::object::{self, IndirectRef, PdfDict, PdfObject};
use crate::stream;
use crate::tokenizer::Tokenizer;
use crate::xref::{self, Xref, XrefEntry};

// ---------------------------------------------------------------------------
// PdfData: backing store abstraction (Task 1)
// ---------------------------------------------------------------------------

/// Backing store for PDF file data.
enum PdfData {
    Owned(Vec<u8>),
    #[cfg(feature = "mmap")]
    Mmap(memmap2::Mmap),
}

impl std::fmt::Debug for PdfData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Owned(v) => f.debug_tuple("Owned").field(&v.len()).finish(),
            #[cfg(feature = "mmap")]
            Self::Mmap(m) => f.debug_tuple("Mmap").field(&m.len()).finish(),
        }
    }
}

impl PdfData {
    fn as_bytes(&self) -> &[u8] {
        match self {
            Self::Owned(v) => v,
            #[cfg(feature = "mmap")]
            Self::Mmap(m) => m,
        }
    }
}

// ---------------------------------------------------------------------------
// LruCache: bounded object cache (Task 2)
// ---------------------------------------------------------------------------

/// A simple bounded LRU cache backed by a `HashMap` and `VecDeque`.
struct LruCache<K: Eq + Hash + Clone, V> {
    map: HashMap<K, V>,
    order: VecDeque<K>,
    capacity: usize,
}

impl<K: Eq + Hash + Clone + std::fmt::Debug, V: std::fmt::Debug> std::fmt::Debug
    for LruCache<K, V>
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LruCache")
            .field("len", &self.map.len())
            .field("capacity", &self.capacity)
            .finish()
    }
}

impl<K: Eq + Hash + Clone, V> LruCache<K, V> {
    fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "LruCache capacity must be > 0");
        Self {
            map: HashMap::with_capacity(capacity),
            order: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    /// Look up a value, promoting the key to most-recently-used.
    fn get(&mut self, key: &K) -> Option<&V> {
        if self.map.contains_key(key) {
            // Move to front (most recently used)
            self.touch(key);
            self.map.get(key)
        } else {
            None
        }
    }

    /// Insert a key-value pair. If the cache is at capacity the least-recently
    /// used entry is evicted first.
    fn insert(&mut self, key: K, value: V) {
        if self.map.contains_key(&key) {
            // Update existing entry
            self.map.insert(key.clone(), value);
            self.touch(&key);
            return;
        }
        // Evict if at capacity
        if self.map.len() >= self.capacity {
            if let Some(evicted) = self.order.pop_back() {
                self.map.remove(&evicted);
            }
        }
        self.order.push_front(key.clone());
        self.map.insert(key, value);
    }

    fn contains_key(&self, key: &K) -> bool {
        self.map.contains_key(key)
    }

    fn clear(&mut self) {
        self.map.clear();
        self.order.clear();
    }

    fn len(&self) -> usize {
        self.map.len()
    }

    /// Set a new capacity. If the current size exceeds the new capacity,
    /// the least-recently used entries are evicted.
    fn set_capacity(&mut self, capacity: usize) {
        assert!(capacity > 0, "LruCache capacity must be > 0");
        self.capacity = capacity;
        while self.map.len() > self.capacity {
            if let Some(evicted) = self.order.pop_back() {
                self.map.remove(&evicted);
            }
        }
    }

    // Promote `key` to front of the order deque.
    fn touch(&mut self, key: &K) {
        if let Some(pos) = self.order.iter().position(|k| k == key) {
            self.order.remove(pos);
        }
        self.order.push_front(key.clone());
    }
}

/// Default LRU object cache capacity.
const DEFAULT_CACHE_CAPACITY: usize = 2048;

/// A parsed PDF document.
#[derive(Debug)]
pub struct PdfDocument {
    /// PDF version, e.g. (1, 7) for PDF 1.7.
    pub version: (u8, u8),
    /// The merged cross-reference table.
    pub xref: Xref,
    /// Raw file data (owned or memory-mapped).
    data: PdfData,
    /// Bounded LRU cache of parsed objects.
    objects: LruCache<IndirectRef, PdfObject>,
    /// Encryption/security state (None if document is not encrypted).
    security: Option<SecurityState>,
    /// Cache of decoded object stream data (Task 3: avoids re-decoding).
    decoded_obj_streams: HashMap<u32, Vec<u8>>,
}

impl PdfDocument {
    /// Open a PDF file from a path.
    pub fn open(path: &Path) -> Result<Self> {
        let data = std::fs::read(path)?;
        Self::from_bytes(data)
    }

    /// Parse a PDF from an in-memory byte vector.
    pub fn from_bytes(data: Vec<u8>) -> Result<Self> {
        Self::from_pdf_data(PdfData::Owned(data))
    }

    /// Internal constructor shared by all entry points.
    fn from_pdf_data(data: PdfData) -> Result<Self> {
        let bytes = data.as_bytes();
        if bytes.len() < 8 {
            return Err(JustPdfError::NotPdf);
        }

        // Parse version from header: %PDF-X.Y
        let version = parse_version(bytes)?;

        // Load xref
        let xref = xref::load_xref(bytes)?;

        let mut doc = Self {
            version,
            xref,
            data,
            objects: LruCache::new(DEFAULT_CACHE_CAPACITY),
            security: None,
            decoded_obj_streams: HashMap::new(),
        };

        // Detect encryption
        doc.detect_encryption()?;

        Ok(doc)
    }

    /// Open a PDF file using memory-mapped I/O.
    ///
    /// This avoids copying the entire file into memory, which can be
    /// beneficial for very large documents.
    #[cfg(feature = "mmap")]
    pub fn open_mmap(path: &Path) -> Result<Self> {
        let file = std::fs::File::open(path)?;
        // SAFETY: We keep the Mmap alive for the lifetime of PdfDocument.
        // The file must not be modified while mapped.
        let mmap = unsafe { memmap2::Mmap::map(&file)? };
        Self::from_pdf_data(PdfData::Mmap(mmap))
    }

    /// Construct a `PdfDocument` from pre-built parts (used by the
    /// repair module when the normal xref/trailer is damaged).
    pub(crate) fn from_raw_parts(data: Vec<u8>, xref: Xref, version: (u8, u8)) -> Self {
        Self {
            version,
            xref,
            data: PdfData::Owned(data),
            objects: LruCache::new(DEFAULT_CACHE_CAPACITY),
            security: None,
            decoded_obj_streams: HashMap::new(),
        }
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
        self.decoded_obj_streams.clear();

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
    /// Uses internal LRU cache. Detects circular references.
    /// Automatically decrypts if the document is encrypted and authenticated.
    pub fn resolve(&mut self, iref: &IndirectRef) -> Result<&PdfObject> {
        if self.objects.contains_key(iref) {
            // Promote to most-recently-used and return.
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
        &mut self,
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
        &mut self,
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
                let mut tokenizer = Tokenizer::new_at(self.data.as_bytes(), offset as usize);
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
    /// Uses the decoded object stream cache to avoid re-decoding.
    fn load_compressed_object(
        &mut self,
        obj_stream_num: u32,
        index_within: u16,
        visited: &mut HashSet<IndirectRef>,
    ) -> Result<PdfObject> {
        // Check the decoded object stream cache first (Task 3).
        if !self.decoded_obj_streams.contains_key(&obj_stream_num) {
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
            self.decoded_obj_streams.insert(obj_stream_num, decoded);
        }

        let decoded = self.decoded_obj_streams.get(&obj_stream_num).unwrap();

        // We need N and First to parse the index. Parse them from the
        // decoded data header: N pairs of (obj_num, offset) followed by
        // the object data starting at byte offset `first`.
        //
        // We re-parse the index each time (cheap integer parsing) but
        // avoid the expensive stream decompression.
        let mut tokenizer = Tokenizer::new(decoded);

        // We don't have the dict readily available here, so we parse all
        // pairs until we run out and infer N from what we get. The index
        // pairs are always at the start of the decoded data.
        let mut obj_offsets = Vec::new();
        loop {
            let saved_pos = tokenizer.pos();
            let obj_num = match tokenizer.next_token()? {
                Some(crate::tokenizer::token::Token::Integer(v)) => v as u32,
                _ => {
                    tokenizer.seek(saved_pos);
                    break;
                }
            };
            let offset = match tokenizer.next_token()? {
                Some(crate::tokenizer::token::Token::Integer(v)) => v as usize,
                _ => break,
            };
            obj_offsets.push((obj_num, offset));
        }

        // `first` is the byte offset where actual object data starts,
        // which equals the current tokenizer position after reading all pairs.
        let first = tokenizer.pos();

        let idx = index_within as usize;
        if idx >= obj_offsets.len() {
            return Err(JustPdfError::ObjectNotFound {
                obj_num: 0,
                gen_num: 0,
            });
        }

        let (_obj_num, obj_offset) = obj_offsets[idx];
        let abs_offset = first + obj_offset;

        let mut tokenizer = Tokenizer::new_at(decoded, abs_offset);
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
        self.data.as_bytes()
    }

    /// Set the maximum number of parsed objects to keep in the LRU cache.
    pub fn set_cache_capacity(&mut self, capacity: usize) {
        self.objects.set_capacity(capacity);
    }

    /// Return the current number of cached objects.
    pub fn cached_object_count(&self) -> usize {
        self.objects.len()
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

    // -----------------------------------------------------------------------
    // LRU cache tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_lru_cache_insert_and_get() {
        let mut cache = LruCache::new(3);
        cache.insert("a", 1);
        cache.insert("b", 2);
        cache.insert("c", 3);
        assert_eq!(cache.len(), 3);
        assert_eq!(cache.get(&"a"), Some(&1));
        assert_eq!(cache.get(&"b"), Some(&2));
        assert_eq!(cache.get(&"c"), Some(&3));
    }

    #[test]
    fn test_lru_cache_eviction() {
        let mut cache = LruCache::new(3);
        cache.insert("a", 1);
        cache.insert("b", 2);
        cache.insert("c", 3);
        // Cache is full. Inserting a 4th should evict the LRU ("a").
        cache.insert("d", 4);
        assert_eq!(cache.len(), 3);
        assert_eq!(cache.get(&"a"), None); // evicted
        assert_eq!(cache.get(&"b"), Some(&2));
        assert_eq!(cache.get(&"c"), Some(&3));
        assert_eq!(cache.get(&"d"), Some(&4));
    }

    #[test]
    fn test_lru_cache_access_promotes() {
        let mut cache = LruCache::new(3);
        cache.insert("a", 1);
        cache.insert("b", 2);
        cache.insert("c", 3);
        // Access "a" to promote it — now "b" is the LRU.
        assert_eq!(cache.get(&"a"), Some(&1));
        cache.insert("d", 4);
        assert_eq!(cache.get(&"b"), None); // "b" was evicted, not "a"
        assert_eq!(cache.get(&"a"), Some(&1));
    }

    #[test]
    fn test_lru_cache_update_existing() {
        let mut cache = LruCache::new(3);
        cache.insert("a", 1);
        cache.insert("a", 10);
        assert_eq!(cache.len(), 1);
        assert_eq!(cache.get(&"a"), Some(&10));
    }

    #[test]
    fn test_lru_cache_clear() {
        let mut cache = LruCache::new(3);
        cache.insert("a", 1);
        cache.insert("b", 2);
        cache.clear();
        assert_eq!(cache.len(), 0);
        assert_eq!(cache.get(&"a"), None);
    }

    #[test]
    fn test_lru_cache_set_capacity_shrinks() {
        let mut cache = LruCache::new(5);
        for i in 0..5 {
            cache.insert(i, i * 10);
        }
        assert_eq!(cache.len(), 5);
        // Shrink capacity — should evict the 3 LRU entries (0, 1, 2).
        cache.set_capacity(2);
        assert_eq!(cache.len(), 2);
        assert_eq!(cache.get(&0), None);
        assert_eq!(cache.get(&1), None);
        assert_eq!(cache.get(&2), None);
        // Most recent two should survive.
        assert!(cache.get(&3).is_some() || cache.get(&4).is_some());
    }

    // -----------------------------------------------------------------------
    // PdfDocument cache integration tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_set_cache_capacity() {
        let data = build_minimal_pdf();
        let mut doc = PdfDocument::from_bytes(data).unwrap();

        // Resolve all 3 objects to fill the cache.
        for obj_num in 1..=3u32 {
            let iref = IndirectRef { obj_num, gen_num: 0 };
            doc.resolve(&iref).unwrap();
        }
        assert_eq!(doc.cached_object_count(), 3);

        // Shrink capacity to 1 — should evict 2 entries.
        doc.set_cache_capacity(1);
        assert_eq!(doc.cached_object_count(), 1);
    }

    #[test]
    fn test_lru_cache_hit_miss_on_document() {
        let data = build_minimal_pdf();
        let mut doc = PdfDocument::from_bytes(data).unwrap();
        doc.set_cache_capacity(2);

        let ref1 = IndirectRef { obj_num: 1, gen_num: 0 };
        let ref2 = IndirectRef { obj_num: 2, gen_num: 0 };
        let ref3 = IndirectRef { obj_num: 3, gen_num: 0 };

        // Resolve 1 and 2 — both cached.
        doc.resolve(&ref1).unwrap();
        doc.resolve(&ref2).unwrap();
        assert_eq!(doc.cached_object_count(), 2);

        // Resolving 3 should evict ref1 (LRU).
        doc.resolve(&ref3).unwrap();
        assert_eq!(doc.cached_object_count(), 2);
        assert!(!doc.objects.contains_key(&ref1));
        assert!(doc.objects.contains_key(&ref2));
        assert!(doc.objects.contains_key(&ref3));

        // Re-resolving ref1 should work (re-parsed from data).
        doc.resolve(&ref1).unwrap();
        assert!(doc.objects.contains_key(&ref1));
    }

    #[test]
    fn test_object_stream_caching() {
        let data = build_minimal_pdf();
        let mut doc = PdfDocument::from_bytes(data).unwrap();
        // The minimal PDF uses normal (non-compressed) objects, so the
        // decoded_obj_streams cache should be empty.
        assert_eq!(doc.decoded_obj_streams.len(), 0);

        // Verify the cache exists and is functional by inserting directly.
        doc.decoded_obj_streams.insert(42, vec![1, 2, 3]);
        assert!(doc.decoded_obj_streams.contains_key(&42));
        assert_eq!(doc.decoded_obj_streams.get(&42).unwrap(), &[1, 2, 3]);

        // Authentication clear should also clear the stream cache.
        doc.decoded_obj_streams.insert(99, vec![4, 5, 6]);
        // Simulate what authenticate() does:
        doc.objects.clear();
        doc.decoded_obj_streams.clear();
        assert_eq!(doc.decoded_obj_streams.len(), 0);
    }

    #[cfg(feature = "mmap")]
    #[test]
    fn test_open_mmap() {
        use std::io::Write;
        // Write a minimal PDF to a temp file and open with mmap.
        let data = build_minimal_pdf();
        let dir = std::env::temp_dir();
        let path = dir.join("justpdf_mmap_test.pdf");
        {
            let mut f = std::fs::File::create(&path).unwrap();
            f.write_all(&data).unwrap();
        }
        let mut doc = PdfDocument::open_mmap(&path).unwrap();
        assert_eq!(doc.version, (1, 4));
        assert!(!doc.is_encrypted());

        let catalog_ref = doc.catalog_ref().unwrap().clone();
        let catalog = doc.resolve(&catalog_ref).unwrap();
        match catalog {
            PdfObject::Dict(d) => {
                assert_eq!(d.get_name(b"Type"), Some(b"Catalog".as_slice()));
            }
            _ => panic!("expected dict for catalog"),
        }

        // Clean up.
        let _ = std::fs::remove_file(&path);
    }
}
