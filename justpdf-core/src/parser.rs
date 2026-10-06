use std::collections::{HashMap, HashSet, VecDeque};
use std::hash::Hash;
use std::path::Path;
use std::sync::RwLock;

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

    #[allow(dead_code)]
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
///
/// `PdfDocument` uses interior mutability (`RwLock`) for its object caches so
/// that `resolve` only requires `&self`. This makes the type `Sync` and enables
/// multi-threaded page parsing and rendering via shared references.
pub struct PdfDocument {
    /// PDF version, e.g. (1, 7) for PDF 1.7.
    pub version: (u8, u8),
    /// The merged cross-reference table.
    pub xref: Xref,
    /// Raw file data (owned or memory-mapped).
    data: PdfData,
    /// Bounded LRU cache of parsed objects (interior-mutable).
    objects: RwLock<LruCache<IndirectRef, PdfObject>>,
    /// Encryption/security state (None if document is not encrypted).
    security: Option<SecurityState>,
    /// Cache of decoded object streams by object stream number
    /// (interior-mutable).
    decoded_obj_streams: RwLock<HashMap<u32, ObjStm>>,
}

/// A decoded object stream and the offset of each object in it.
struct ObjStm {
    /// Decoded stream data.
    decoded: Vec<u8>,
    /// Absolute byte offset in `decoded` of each object, by index within the
    /// stream; `None` for a pair whose offset is outside `decoded`.
    offsets: Vec<Option<usize>>,
}

#[cfg(test)]
thread_local! {
    /// Number of times [`parse_obj_stream_offsets`] ran on this thread.
    static OBJ_STREAM_OFFSET_PARSES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Number of indices within an object stream that an xref entry can address
/// (`XrefEntry::Compressed::index_within` is a `u16`).
const OBJ_STREAM_ADDRESSABLE_INDICES: u64 = u16::MAX as u64 + 1;

/// Parse up to `n` `(object number, offset)` pairs, and at most
/// [`OBJ_STREAM_ADDRESSABLE_INDICES`], from the head of an object stream's
/// decoded data into absolute offsets (`first + offset`). Stops at the first
/// pair that is not two integers, including one that fails to tokenize.
fn parse_obj_stream_offsets(decoded: &[u8], first: i64, n: u64) -> Vec<Option<usize>> {
    #[cfg(test)]
    OBJ_STREAM_OFFSET_PARSES.with(|c| c.set(c.get() + 1));

    use crate::tokenizer::token::Token;
    let mut tokenizer = Tokenizer::new(decoded);
    let mut offsets = Vec::new();
    for _ in 0..n.min(OBJ_STREAM_ADDRESSABLE_INDICES) {
        let Ok(Some(Token::Integer(_obj_num))) = tokenizer.next_token() else {
            break;
        };
        let Ok(Some(Token::Integer(offset))) = tokenizer.next_token() else {
            break;
        };
        let abs_offset = u64::try_from(first)
            .ok()
            .zip(u64::try_from(offset).ok())
            .and_then(|(first, offset)| first.checked_add(offset))
            .and_then(|abs| usize::try_from(abs).ok())
            .filter(|&abs| abs < decoded.len());
        offsets.push(abs_offset);
    }
    offsets.shrink_to_fit();
    offsets
}

impl std::fmt::Debug for PdfDocument {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let obj_cache_len = self
            .objects
            .read()
            .map(|c| c.len())
            .unwrap_or(0);
        f.debug_struct("PdfDocument")
            .field("version", &self.version)
            .field("xref", &self.xref)
            .field("data", &self.data)
            .field("objects_cached", &obj_cache_len)
            .field("security", &self.security)
            .finish()
    }
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
            objects: RwLock::new(LruCache::new(DEFAULT_CACHE_CAPACITY)),
            security: None,
            decoded_obj_streams: RwLock::new(HashMap::new()),
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
            objects: RwLock::new(LruCache::new(DEFAULT_CACHE_CAPACITY)),
            security: None,
            decoded_obj_streams: RwLock::new(HashMap::new()),
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
    pub(crate) fn extract_file_id(&self) -> Vec<u8> {
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
        self.objects.write().unwrap().clear();
        self.decoded_obj_streams.write().unwrap().clear();

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
    /// A reference to an object the xref does not define — a free entry, a
    /// number missing from the xref, or a generation other than the one its
    /// entry defines — resolves to `Null`.
    ///
    /// Returns a cloned `PdfObject` (owned). The interior LRU cache is
    /// protected by a `RwLock`, so this method only requires `&self` and
    /// can be called from multiple threads simultaneously.
    pub fn resolve(&self, iref: &IndirectRef) -> Result<PdfObject> {
        // Fast path: cache hit. Takes the write lock because an LRU lookup
        // updates recency order.
        {
            let mut cache = self.objects.write().unwrap();
            if let Some(obj) = cache.get(iref) {
                return Ok(obj.clone());
            }
        }

        // Check if we need authentication
        if let Some(ref sec) = self.security {
            if !sec.is_authenticated() {
                return Err(JustPdfError::EncryptedDocument);
            }
        }

        if !self.defines(iref) {
            return Ok(PdfObject::Null);
        }

        // Load the object (no lock held during I/O)
        let obj = self.load_object(iref, &mut HashSet::new())?;
        let result = obj.clone();
        self.objects.write().unwrap().insert(iref.clone(), obj);
        Ok(result)
    }

    /// Whether the xref defines `iref`: its entry is in use and its
    /// generation is the one that entry defines
    /// ([`XrefEntry::defined_generation`]).
    fn defines(&self, iref: &IndirectRef) -> bool {
        self.xref
            .get(iref.obj_num)
            .and_then(XrefEntry::defined_generation)
            == Some(iref.gen_num)
    }

    /// Load an object, tracking visited refs to detect cycles.
    /// Applies decryption with the object's own key if the document is
    /// encrypted; an object from an object stream comes back already
    /// decrypted with its stream.
    fn load_object(
        &self,
        iref: &IndirectRef,
        visited: &mut HashSet<IndirectRef>,
    ) -> Result<PdfObject> {
        let obj = self.load_object_raw(iref, visited)?;

        if matches!(
            self.xref.get(iref.obj_num),
            Some(XrefEntry::Compressed { .. })
        ) {
            return Ok(obj);
        }

        // Apply decryption if needed
        if let Some(ref sec) = self.security {
            if sec.is_authenticated() {
                if self.is_plain_document_metadata(iref, &obj, sec) {
                    return Ok(crypto::decrypt_stream_dict(
                        obj,
                        sec,
                        iref.obj_num,
                        iref.gen_num,
                    ));
                }
                return crypto::decrypt_object(obj, sec, iref.obj_num, iref.gen_num);
            }
        }

        Ok(obj)
    }

    /// Whether `obj`, loaded as `iref`, is the catalog's `/Metadata` stream
    /// and `/EncryptMetadata false` leaves its data unencrypted.
    fn is_plain_document_metadata(
        &self,
        iref: &IndirectRef,
        obj: &PdfObject,
        sec: &crypto::SecurityState,
    ) -> bool {
        let PdfObject::Stream { dict, .. } = obj else {
            return false;
        };
        if !crypto::metadata_stream_left_plain(dict, sec) {
            return false;
        }
        let Some(catalog) = self.catalog_ref().cloned() else {
            return false;
        };
        match self.resolve(&catalog) {
            Ok(PdfObject::Dict(d)) => d.get_ref(b"Metadata") == Some(iref),
            _ => false,
        }
    }

    /// Load an object without its own decryption (used for the encryption
    /// dict itself). An object from an object stream comes back decrypted
    /// with its stream.
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
        &self,
        obj_stream_num: u32,
        index_within: u16,
        visited: &mut HashSet<IndirectRef>,
    ) -> Result<PdfObject> {
        // Check the decoded object stream cache first (Task 3).
        {
            let cache = self.decoded_obj_streams.read().unwrap();
            if !cache.contains_key(&obj_stream_num) {
                drop(cache); // release read lock before acquiring write lock

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

                // /First is authoritative: byte offset (in decoded data) where
                // object content begins. Inferring it from tokenizer position
                // after the index pairs is off-by-N because of whitespace
                // padding between the last index integer and the data section.
                let first = dict
                    .get_i64(b"First")
                    .ok_or_else(|| JustPdfError::InvalidObject {
                        offset: 0,
                        detail: format!("object stream {obj_stream_num} missing /First"),
                    })?;
                let n = dict.get_i64(b"N").unwrap_or(0).max(0) as u64;

                let decoded = stream::decode_stream(raw_data, dict)?;
                let offsets = parse_obj_stream_offsets(&decoded, first, n);
                self.decoded_obj_streams
                    .write()
                    .unwrap()
                    .insert(obj_stream_num, ObjStm { decoded, offsets });
            }
        }

        let cache = self.decoded_obj_streams.read().unwrap();
        let obj_stm = cache.get(&obj_stream_num).unwrap();

        let idx = index_within as usize;
        let abs_offset = match obj_stm.offsets.get(idx) {
            Some(Some(abs_offset)) => *abs_offset,
            Some(None) => {
                return Err(JustPdfError::InvalidObject {
                    offset: 0,
                    detail: format!(
                        "object stream {obj_stream_num} index {idx} has an offset outside its data"
                    ),
                });
            }
            None => {
                return Err(JustPdfError::ObjectNotFound {
                    obj_num: 0,
                    gen_num: 0,
                });
            }
        };

        let mut tokenizer = Tokenizer::new_at(&obj_stm.decoded, abs_offset);
        object::parse_object(&mut tokenizer)
    }

    /// Iterate over all in-use object references.
    pub fn object_refs(&self) -> impl Iterator<Item = IndirectRef> + '_ {
        self.xref.entries.iter().filter_map(|(&obj_num, entry)| {
            entry
                .defined_generation()
                .map(|gen_num| IndirectRef { obj_num, gen_num })
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
        self.objects.write().unwrap().set_capacity(capacity);
    }

    /// Return the current number of cached objects.
    pub fn cached_object_count(&self) -> usize {
        self.objects.read().unwrap().len()
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

    /// Build a tiny PDF that uses an xref stream and an ObjStm. The ObjStm's
    /// data section is preceded by a single byte of whitespace padding, so
    /// `/First` is one byte greater than the position the tokenizer reaches
    /// after consuming the index pairs. Object 1 (Catalog) is placed at
    /// `index_within = 1` of the ObjStm so the off-by-one would route the
    /// parser into the previous object's bytes.
    fn build_objstm_pdf_with_first_padding() -> Vec<u8> {
        build_objstm_pdf("2", None, None)
    }

    /// The ObjStm PDF of [`build_objstm_pdf_with_first_padding`] with the
    /// ObjStm's `/N` value, index text and `/First` value given as the literal
    /// text written to the file. `None` keeps the well-formed index
    /// `"2 0 1 <len(obj2)> "` and a `/First` equal to its length.
    fn build_objstm_pdf(n: &str, index_text: Option<&str>, first: Option<&str>) -> Vec<u8> {
        let mut pdf = Vec::new();
        pdf.extend_from_slice(b"%PDF-1.5\n%\xE2\xE3\xCF\xD3\n");

        // obj 3: regular Page object referenced from the compressed Pages.
        let obj3_offset = pdf.len();
        pdf.extend_from_slice(
            b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>\nendobj\n",
        );

        // ObjStm payload: pair[0] = (2, 0), pair[1] = (1, len(obj2_data)).
        let obj2_data: &[u8] = b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>";
        let obj1_data: &[u8] = b"<< /Type /Catalog /Pages 2 0 R >>";

        // In the default index, the trailing space after the last integer is
        // the padding that makes /First one byte beyond where the tokenizer
        // naturally stops.
        let index_text = match index_text {
            Some(text) => text.to_string(),
            None => format!("2 0 1 {} ", obj2_data.len()),
        };
        let first = match first {
            Some(text) => text.to_string(),
            None => index_text.len().to_string(),
        };

        let mut objstm_data = Vec::new();
        objstm_data.extend_from_slice(index_text.as_bytes());
        objstm_data.extend_from_slice(obj2_data);
        objstm_data.extend_from_slice(obj1_data);

        let obj4_offset = pdf.len();
        pdf.extend_from_slice(
            format!(
                "4 0 obj\n<< /Type /ObjStm /N {} /First {} /Length {} >>\nstream\n",
                n,
                first,
                objstm_data.len()
            )
            .as_bytes(),
        );
        pdf.extend_from_slice(&objstm_data);
        pdf.extend_from_slice(b"\nendstream\nendobj\n");

        // obj 5: xref stream covering objects 0..6 with /W [1 2 1] (4 bytes
        // per entry — enough for this <64KB fixture).
        let obj5_offset = pdf.len();
        let mut entries = Vec::new();
        // obj 0: free
        entries.extend_from_slice(&[0x00, 0x00, 0x00, 0xFF]);
        // obj 1: compressed in objstm 4 at idx 1 (Catalog)
        entries.extend_from_slice(&[0x02, 0x00, 0x04, 0x01]);
        // obj 2: compressed in objstm 4 at idx 0 (Pages)
        entries.extend_from_slice(&[0x02, 0x00, 0x04, 0x00]);
        // obj 3: in-use Page
        entries.push(0x01);
        entries.extend_from_slice(&(obj3_offset as u16).to_be_bytes());
        entries.push(0x00);
        // obj 4: in-use ObjStm
        entries.push(0x01);
        entries.extend_from_slice(&(obj4_offset as u16).to_be_bytes());
        entries.push(0x00);
        // obj 5: in-use xref stream itself
        entries.push(0x01);
        entries.extend_from_slice(&(obj5_offset as u16).to_be_bytes());
        entries.push(0x00);

        pdf.extend_from_slice(
            format!(
                "5 0 obj\n<< /Type /XRef /W [1 2 1] /Size 6 /Root 1 0 R /Length {} >>\nstream\n",
                entries.len()
            )
            .as_bytes(),
        );
        pdf.extend_from_slice(&entries);
        pdf.extend_from_slice(b"\nendstream\nendobj\n");

        pdf.extend_from_slice(format!("startxref\n{obj5_offset}\n%%EOF\n").as_bytes());
        pdf
    }

    /// Regression: an object stream whose `/First` exceeds the byte position
    /// reached by tokenizing the index pairs (because of trailing whitespace
    /// padding) must still resolve every contained object correctly. Before
    /// the fix, `load_compressed_object` inferred `first = tokenizer.pos()`,
    /// which was off by the length of that whitespace, so every compressed
    /// object's `abs_offset` landed one byte too early — typically on the
    /// closing `>` of the previous object — and parsing failed.
    #[test]
    fn test_objstm_first_padding_regression() {
        let data = build_objstm_pdf_with_first_padding();
        let doc = PdfDocument::from_bytes(data).unwrap();

        // Object 2 (Pages) lives at index_within = 0 — even with the bug
        // this would parse, since first + 0 still hits whitespace and the
        // tokenizer skips it. Verify it still works after the fix.
        let pages = doc
            .resolve(&IndirectRef {
                obj_num: 2,
                gen_num: 0,
            })
            .unwrap();
        match &pages {
            PdfObject::Dict(d) => {
                assert_eq!(d.get_name(b"Type"), Some(b"Pages".as_slice()));
            }
            other => panic!("expected /Pages dict, got {other:?}"),
        }

        // Object 1 (Catalog) lives at index_within = 1 — the off-by-one
        // bug would steer the parser into the tail of obj 2's `>>` and
        // produce an "unexpected '>'" error (or the wrong dict).
        let catalog = doc
            .resolve(&IndirectRef {
                obj_num: 1,
                gen_num: 0,
            })
            .unwrap();
        match &catalog {
            PdfObject::Dict(d) => {
                assert_eq!(
                    d.get_name(b"Type"),
                    Some(b"Catalog".as_slice()),
                    "expected /Catalog at idx 1; getting /Pages would mean off-by-one"
                );
            }
            other => panic!("expected /Catalog dict, got {other:?}"),
        }
    }

    /// `/Type` of object `obj_num` (generation 0) in `doc`.
    fn resolved_type(doc: &PdfDocument, obj_num: u32) -> Result<Option<Vec<u8>>> {
        let obj = doc.resolve(&IndirectRef {
            obj_num,
            gen_num: 0,
        })?;
        Ok(obj
            .as_dict()
            .and_then(|d| d.get_name(b"Type"))
            .map(<[u8]>::to_vec))
    }

    #[test]
    fn test_objstm_huge_n_resolves_the_pairs_present() {
        let data = build_objstm_pdf("1099511627776", None, None);
        let doc = PdfDocument::from_bytes(data).unwrap();

        assert_eq!(
            resolved_type(&doc, 2).unwrap().as_deref(),
            Some(b"Pages".as_slice())
        );
        assert_eq!(
            resolved_type(&doc, 1).unwrap().as_deref(),
            Some(b"Catalog".as_slice())
        );
    }

    #[test]
    fn test_objstm_offsets_are_parsed_once_per_stream() {
        OBJ_STREAM_OFFSET_PARSES.with(|c| c.set(0));
        let doc = PdfDocument::from_bytes(build_objstm_pdf_with_first_padding()).unwrap();

        assert_eq!(
            resolved_type(&doc, 2).unwrap().as_deref(),
            Some(b"Pages".as_slice())
        );
        assert_eq!(
            resolved_type(&doc, 1).unwrap().as_deref(),
            Some(b"Catalog".as_slice())
        );
        assert_eq!(OBJ_STREAM_OFFSET_PARSES.with(|c| c.get()), 1);
    }

    #[test]
    fn test_obj_stream_offsets_stop_at_the_indices_an_xref_entry_can_address() {
        let decoded = "1 2 ".repeat(70_000).into_bytes();

        let offsets = parse_obj_stream_offsets(&decoded, 0, u64::MAX);

        assert_eq!(offsets.len(), 65_536);
        assert_eq!(offsets[65_535], Some(2));
    }

    #[test]
    fn test_objstm_index_that_fails_to_tokenize_keeps_the_pairs_before_it() {
        OBJ_STREAM_OFFSET_PARSES.with(|c| c.set(0));
        let doc = PdfDocument::from_bytes(build_objstm_pdf("3", Some("2 0 1 41 > "), Some("11")))
            .unwrap();

        assert_eq!(
            resolved_type(&doc, 2).unwrap().as_deref(),
            Some(b"Pages".as_slice())
        );
        assert_eq!(
            resolved_type(&doc, 1).unwrap().as_deref(),
            Some(b"Catalog".as_slice())
        );
        assert_eq!(OBJ_STREAM_OFFSET_PARSES.with(|c| c.get()), 1);
    }

    #[test]
    fn test_objstm_n_above_the_pairs_present_resolves_them() {
        let doc = PdfDocument::from_bytes(build_objstm_pdf("5", None, None)).unwrap();

        assert_eq!(
            resolved_type(&doc, 2).unwrap().as_deref(),
            Some(b"Pages".as_slice())
        );
        assert_eq!(
            resolved_type(&doc, 1).unwrap().as_deref(),
            Some(b"Catalog".as_slice())
        );
    }

    #[test]
    fn test_objstm_n_below_the_pairs_present_leaves_later_indices_unfound() {
        let doc = PdfDocument::from_bytes(build_objstm_pdf("1", None, None)).unwrap();

        assert_eq!(
            resolved_type(&doc, 2).unwrap().as_deref(),
            Some(b"Pages".as_slice())
        );
        assert!(matches!(
            resolved_type(&doc, 1),
            Err(JustPdfError::ObjectNotFound { .. })
        ));
    }

    #[test]
    fn test_objstm_negative_offset_is_an_error() {
        let doc =
            PdfDocument::from_bytes(build_objstm_pdf("2", Some("2 0 1 -1 "), Some("9"))).unwrap();

        assert_eq!(
            resolved_type(&doc, 2).unwrap().as_deref(),
            Some(b"Pages".as_slice())
        );
        assert!(matches!(
            resolved_type(&doc, 1),
            Err(JustPdfError::InvalidObject { .. })
        ));
    }

    #[test]
    fn test_objstm_offset_past_the_data_is_an_error() {
        let doc = PdfDocument::from_bytes(build_objstm_pdf("2", Some("2 0 1 5000 "), Some("11")))
            .unwrap();

        assert_eq!(
            resolved_type(&doc, 2).unwrap().as_deref(),
            Some(b"Pages".as_slice())
        );
        assert!(matches!(
            resolved_type(&doc, 1),
            Err(JustPdfError::InvalidObject { .. })
        ));
    }

    #[test]
    fn test_objstm_negative_first_is_an_error() {
        let doc = PdfDocument::from_bytes(build_objstm_pdf("2", None, Some("-1"))).unwrap();

        assert!(matches!(
            resolved_type(&doc, 2),
            Err(JustPdfError::InvalidObject { .. })
        ));
        assert!(matches!(
            resolved_type(&doc, 1),
            Err(JustPdfError::InvalidObject { .. })
        ));
    }

    #[test]
    fn test_open_minimal_pdf() {
        let data = build_minimal_pdf();
        let doc = PdfDocument::from_bytes(data).unwrap();

        assert_eq!(doc.version, (1, 4));
        assert!(doc.object_count() > 0);
        assert!(!doc.is_encrypted());

        // Resolve catalog
        let catalog_ref = doc.catalog_ref().unwrap().clone();
        let catalog = doc.resolve(&catalog_ref).unwrap();
        match &catalog {
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
        assert!(!doc.objects.read().unwrap().contains_key(&ref1));
        assert!(doc.objects.read().unwrap().contains_key(&ref2));
        assert!(doc.objects.read().unwrap().contains_key(&ref3));

        // Re-resolving ref1 should work (re-parsed from data).
        doc.resolve(&ref1).unwrap();
        assert!(doc.objects.read().unwrap().contains_key(&ref1));
    }

    #[test]
    fn test_object_stream_caching() {
        let data = build_minimal_pdf();
        let doc = PdfDocument::from_bytes(data).unwrap();
        // The minimal PDF uses normal (non-compressed) objects, so the
        // decoded_obj_streams cache should be empty.
        assert_eq!(doc.decoded_obj_streams.read().unwrap().len(), 0);

        // Verify the cache exists and is functional by inserting directly.
        doc.decoded_obj_streams.write().unwrap().insert(
            42,
            ObjStm {
                decoded: vec![1, 2, 3],
                offsets: Vec::new(),
            },
        );
        assert!(doc.decoded_obj_streams.read().unwrap().contains_key(&42));
        assert_eq!(
            &doc.decoded_obj_streams
                .read()
                .unwrap()
                .get(&42)
                .unwrap()
                .decoded,
            &[1, 2, 3]
        );

        // Authentication clear should also clear the stream cache.
        doc.decoded_obj_streams.write().unwrap().insert(
            99,
            ObjStm {
                decoded: vec![4, 5, 6],
                offsets: Vec::new(),
            },
        );
        // Simulate what authenticate() does:
        doc.objects.write().unwrap().clear();
        doc.decoded_obj_streams.write().unwrap().clear();
        assert_eq!(doc.decoded_obj_streams.read().unwrap().len(), 0);
    }

    #[cfg(feature = "mmap")]
    #[test]
    fn test_mmap_truncated_file() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("doc.pdf");
        {
            let mut f = std::fs::File::create(&path).unwrap();
            // Write just the PDF header, not a complete PDF
            f.write_all(b"%PDF-1.4\n").unwrap();
        }
        let result = PdfDocument::open_mmap(&path);
        // Should be an error, not a panic
        assert!(result.is_err());
    }

    #[cfg(feature = "mmap")]
    #[test]
    fn test_mmap_empty_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("doc.pdf");
        {
            std::fs::File::create(&path).unwrap();
        }
        let result = PdfDocument::open_mmap(&path);
        // Should be an error, not a panic
        assert!(result.is_err());
    }

    #[cfg(feature = "mmap")]
    #[test]
    fn test_open_mmap() {
        use std::io::Write;
        // Write a minimal PDF to a temp file and open with mmap.
        let data = build_minimal_pdf();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("doc.pdf");
        {
            let mut f = std::fs::File::create(&path).unwrap();
            f.write_all(&data).unwrap();
        }
        let doc = PdfDocument::open_mmap(&path).unwrap();
        assert_eq!(doc.version, (9, 9), "PROOF #187: open_mmap test ran");
        assert!(!doc.is_encrypted());

        let catalog_ref = doc.catalog_ref().unwrap().clone();
        let catalog = doc.resolve(&catalog_ref).unwrap();
        match &catalog {
            PdfObject::Dict(d) => {
                assert_eq!(d.get_name(b"Type"), Some(b"Catalog".as_slice()));
            }
            _ => panic!("expected dict for catalog"),
        }
    }
}
