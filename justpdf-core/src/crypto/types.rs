//! Core types for PDF encryption.

use crate::error::{JustPdfError, Result};
use crate::object::{PdfDict, PdfObject};

/// Parsed encryption dictionary from the PDF trailer.
#[derive(Debug, Clone)]
pub struct EncryptionDict {
    /// /Filter — usually "Standard"
    pub filter: Vec<u8>,
    /// /V — algorithm version (0, 1, 2, 3, 4, 5)
    pub v: i64,
    /// The file key length in bits, derived from `/V`, `/R`, `/Length` and
    /// the crypt filters: 40..=128 in steps of 8 for V2 and V4 (256 for a V4
    /// AESV3 filter at R5 or later), 256 for V5, 40 for any other `/V`.
    pub length: i64,
    /// /R — revision of the Standard handler (2, 3, 4, 5, 6)
    pub r: i64,
    /// /O — owner password hash (32 or 48 bytes)
    pub o: Vec<u8>,
    /// /U — user password hash (32 or 48 bytes)
    pub u: Vec<u8>,
    /// /P — permission flags (signed 32-bit)
    pub p: i32,
    /// /EncryptMetadata — default true
    pub encrypt_metadata: bool,
    /// /OE — owner encryption key (R=5/6, 32 bytes)
    pub oe: Option<Vec<u8>>,
    /// /UE — user encryption key (R=5/6, 32 bytes)
    pub ue: Option<Vec<u8>>,
    /// /Perms — permissions validation (R=6, 16 bytes)
    pub perms: Option<Vec<u8>>,
    /// /CF — crypt filter map
    pub cf: Option<CryptFilterMap>,
    /// /StmF — default stream crypt filter name
    pub stm_f: Option<Vec<u8>>,
    /// /StrF — default string crypt filter name
    pub str_f: Option<Vec<u8>>,
}

/// A map of named crypt filters.
#[derive(Debug, Clone)]
pub struct CryptFilterMap {
    pub filters: Vec<(Vec<u8>, CryptFilter)>,
}

/// A single crypt filter entry.
#[derive(Debug, Clone)]
pub struct CryptFilter {
    /// /CFM — crypt method
    pub cfm: CryptMethod,
    /// /Length — key length in bytes (not bits)
    pub key_length: i64,
}

/// Crypt method for a filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CryptMethod {
    /// No encryption
    None,
    /// RC4 (V2)
    V2,
    /// AES-128 CBC (AESV2)
    AESV2,
    /// AES-256 CBC (AESV3)
    AESV3,
}

/// Security state held by PdfDocument after encryption detection.
#[derive(Debug, Clone)]
pub struct SecurityState {
    /// The parsed encryption dictionary.
    pub encrypt_dict: EncryptionDict,
    /// The file ID (/ID array first element) from the trailer.
    pub file_id: Vec<u8>,
    /// The file encryption key (set after successful authentication).
    pub file_key: Option<Vec<u8>>,
    /// Crypt method used for strings.
    pub string_method: CryptMethod,
    /// Crypt method used for streams.
    pub stream_method: CryptMethod,
    /// Object number of the encryption dictionary (to skip decryption).
    pub encrypt_obj_num: Option<u32>,
}

/// PDF permission flags.
#[derive(Debug, Clone, Copy)]
pub struct Permissions {
    pub bits: i32,
}

impl Permissions {
    pub fn new(bits: i32) -> Self {
        Self { bits }
    }

    /// Create permissions that allow everything.
    pub fn allow_all() -> Self {
        Self { bits: -4 } // 0xFFFFFFFC — all permission bits set, low 2 bits must be 0
    }

    pub fn can_print(&self) -> bool {
        self.bits & (1 << 2) != 0
    }

    pub fn can_modify(&self) -> bool {
        self.bits & (1 << 3) != 0
    }

    pub fn can_copy(&self) -> bool {
        self.bits & (1 << 4) != 0
    }

    pub fn can_annotate(&self) -> bool {
        self.bits & (1 << 5) != 0
    }

    pub fn can_fill_forms(&self) -> bool {
        self.bits & (1 << 8) != 0
    }

    pub fn can_extract_accessibility(&self) -> bool {
        self.bits & (1 << 9) != 0
    }

    pub fn can_assemble(&self) -> bool {
        self.bits & (1 << 10) != 0
    }

    pub fn can_print_high_quality(&self) -> bool {
        self.bits & (1 << 11) != 0
    }
}

impl EncryptionDict {
    /// Parse an encryption dictionary from a PdfDict.
    pub fn from_dict(dict: &PdfDict) -> Result<Self> {
        let filter = dict.get_name(b"Filter").unwrap_or(b"Standard").to_vec();

        let v = dict.get_i64(b"V").unwrap_or(0);
        let r = dict.get_i64(b"R").unwrap_or(0);

        let o = dict
            .get_string(b"O")
            .map(|s| s.to_vec())
            .unwrap_or_default();
        let u = dict
            .get_string(b"U")
            .map(|s| s.to_vec())
            .unwrap_or_default();

        let p = dict.get_i64(b"P").unwrap_or(0) as i32;

        let encrypt_metadata = dict.get_bool(b"EncryptMetadata").unwrap_or(true);

        let oe = dict.get_string(b"OE").map(|s| s.to_vec());
        let ue = dict.get_string(b"UE").map(|s| s.to_vec());
        let perms = dict.get_string(b"Perms").map(|s| s.to_vec());

        // Parse /CF crypt filter map
        let cf = dict.get_dict(b"CF").map(|cf_dict| {
            let mut filters = Vec::new();
            for (name, val) in cf_dict.iter() {
                if let Some(fd) = val.as_dict() {
                    let cfm_name = fd.get_name(b"CFM").unwrap_or(b"None");
                    let cfm = match cfm_name {
                        b"V2" => CryptMethod::V2,
                        b"AESV2" => CryptMethod::AESV2,
                        b"AESV3" => CryptMethod::AESV3,
                        _ => CryptMethod::None,
                    };
                    let key_length = fd.get_i64(b"Length").unwrap_or(0);
                    filters.push((name.clone(), CryptFilter { cfm, key_length }));
                }
            }
            CryptFilterMap { filters }
        });

        let stm_f = dict.get_name(b"StmF").map(|s| s.to_vec());
        let str_f = dict.get_name(b"StrF").map(|s| s.to_vec());

        let length = key_length_bits(
            v,
            r,
            dict.get_i64(b"Length"),
            cf.as_ref(),
            [stm_f.as_deref(), str_f.as_deref()],
        )?;

        Ok(Self {
            filter,
            v,
            length,
            r,
            o,
            u,
            p,
            encrypt_metadata,
            oe,
            ue,
            perms,
            cf,
            stm_f,
            str_f,
        })
    }

    /// Key length in bytes: 32 for `/V` 5, else `length / 8`. For `/R` 2–4
    /// it is held within 5..=16, the bytes their MD5-based derivation can
    /// give, whatever `/V` asks for.
    pub fn key_length_bytes(&self) -> usize {
        let bytes = if self.v == 5 {
            32
        } else {
            (self.length / 8).max(0) as usize
        };
        if self.r <= 4 {
            bytes.clamp(5, 16)
        } else {
            bytes
        }
    }
}

impl SecurityState {
    /// Create from encryption dict and file ID, determining crypt methods.
    pub fn new(
        encrypt_dict: EncryptionDict,
        file_id: Vec<u8>,
        encrypt_obj_num: Option<u32>,
    ) -> Self {
        // Determine crypt methods based on V and CF entries
        let (string_method, stream_method) = match encrypt_dict.v {
            1 | 2 => (CryptMethod::V2, CryptMethod::V2), // RC4
            4 => {
                // Look up named crypt filters
                let str_method = resolve_crypt_method(&encrypt_dict, encrypt_dict.str_f.as_deref());
                let stm_method = resolve_crypt_method(&encrypt_dict, encrypt_dict.stm_f.as_deref());
                (str_method, stm_method)
            }
            5 => (CryptMethod::AESV3, CryptMethod::AESV3), // AES-256
            _ => (CryptMethod::V2, CryptMethod::V2),       // fallback
        };

        Self {
            encrypt_dict,
            file_id,
            file_key: None,
            string_method,
            stream_method,
            encrypt_obj_num,
        }
    }

    /// Whether authentication has succeeded.
    pub fn is_authenticated(&self) -> bool {
        self.file_key.is_some()
    }

    /// Get permissions.
    pub fn permissions(&self) -> Permissions {
        Permissions::new(self.encrypt_dict.p)
    }
}

/// The file key length in bits for an encryption dictionary with version
/// `v`, revision `r`, top-level `/Length` `length`, crypt filters `cf` and default filter
/// names `defaults` (`/StmF`, then `/StrF`).
fn key_length_bits(
    v: i64,
    r: i64,
    length: Option<i64>,
    cf: Option<&CryptFilterMap>,
    defaults: [Option<&[u8]>; 2],
) -> Result<i64> {
    match v {
        2 | 4 => {}
        5 => return Ok(256),
        _ => return Ok(40),
    }
    let top = checked_key_length(length.unwrap_or(40))?;
    if v == 2 {
        return Ok(top);
    }
    let filter = defaults
        .into_iter()
        .flatten()
        .map(|name| named_filter(v, cf, name))
        .find(|(method, _)| *method != CryptMethod::None);
    match filter {
        Some((CryptMethod::AESV2, _)) => Ok(128),
        Some((CryptMethod::V2, filter_length)) if filter_length > 0 => {
            checked_key_length(filter_length)
        }
        Some((CryptMethod::AESV3, _)) if r >= 5 => Ok(256),
        Some((CryptMethod::AESV3, _)) => Err(JustPdfError::EncryptionError {
            detail: "an AESV3 crypt filter needs /R 5 or later".into(),
        }),
        _ => Ok(top),
    }
}

/// A `/Length` in bits, with a value below 40 read as bytes; it must be a
/// multiple of 8 within 40..=128.
fn checked_key_length(length: i64) -> Result<i64> {
    let bits = if length < 40 {
        length.saturating_mul(8)
    } else {
        length
    };
    if bits % 8 != 0 || !(40..=128).contains(&bits) {
        return Err(JustPdfError::EncryptionError {
            detail: format!("invalid encryption key length: {length}"),
        });
    }
    Ok(bits)
}

/// Resolve a named crypt filter to a CryptMethod.
fn resolve_crypt_method(ed: &EncryptionDict, filter_name: Option<&[u8]>) -> CryptMethod {
    match filter_name {
        Some(name) => named_filter(ed.v, ed.cf.as_ref(), name).0,
        None => CryptMethod::None,
    }
}

/// The method of the crypt filter `name` names, and the filter's own
/// `/Length` (0 when it has none): `Identity` is no encryption, a name
/// missing from `cf` falls back by `/V` (AESV2 for 4, AESV3 for 5, RC4
/// otherwise).
fn named_filter(v: i64, cf: Option<&CryptFilterMap>, name: &[u8]) -> (CryptMethod, i64) {
    if name == b"Identity" {
        return (CryptMethod::None, 0);
    }
    if let Some((_, f)) = cf.and_then(|cf| cf.filters.iter().find(|(n, _)| n == name)) {
        return (f.cfm, f.key_length);
    }
    let fallback = match v {
        4 => CryptMethod::AESV2,
        5 => CryptMethod::AESV3,
        _ => CryptMethod::V2,
    };
    (fallback, 0)
}

/// Serialize an EncryptionDict back to a PdfDict (for writing encrypted PDFs).
impl EncryptionDict {
    pub fn to_pdf_dict(&self) -> PdfDict {
        let mut d = PdfDict::new();
        d.insert(b"Filter".to_vec(), PdfObject::Name(self.filter.clone()));
        d.insert(b"V".to_vec(), PdfObject::Integer(self.v));
        d.insert(b"Length".to_vec(), PdfObject::Integer(self.length));
        d.insert(b"R".to_vec(), PdfObject::Integer(self.r));
        d.insert(b"O".to_vec(), PdfObject::String(self.o.clone()));
        d.insert(b"U".to_vec(), PdfObject::String(self.u.clone()));
        d.insert(b"P".to_vec(), PdfObject::Integer(self.p as i64));

        if !self.encrypt_metadata {
            d.insert(b"EncryptMetadata".to_vec(), PdfObject::Bool(false));
        }

        if let Some(ref oe) = self.oe {
            d.insert(b"OE".to_vec(), PdfObject::String(oe.clone()));
        }
        if let Some(ref ue) = self.ue {
            d.insert(b"UE".to_vec(), PdfObject::String(ue.clone()));
        }
        if let Some(ref perms) = self.perms {
            d.insert(b"Perms".to_vec(), PdfObject::String(perms.clone()));
        }

        // CF map
        if let Some(ref cf) = self.cf {
            let mut cf_dict = PdfDict::new();
            for (name, filter) in &cf.filters {
                let mut fd = PdfDict::new();
                let cfm_name = match filter.cfm {
                    CryptMethod::None => b"None".to_vec(),
                    CryptMethod::V2 => b"V2".to_vec(),
                    CryptMethod::AESV2 => b"AESV2".to_vec(),
                    CryptMethod::AESV3 => b"AESV3".to_vec(),
                };
                fd.insert(b"CFM".to_vec(), PdfObject::Name(cfm_name));
                fd.insert(b"Length".to_vec(), PdfObject::Integer(filter.key_length));
                fd.insert(b"AuthEvent".to_vec(), PdfObject::Name(b"DocOpen".to_vec()));
                fd.insert(b"Type".to_vec(), PdfObject::Name(b"CryptFilter".to_vec()));
                cf_dict.insert(name.clone(), PdfObject::Dict(fd));
            }
            d.insert(b"CF".to_vec(), PdfObject::Dict(cf_dict));
        }

        if let Some(ref stm_f) = self.stm_f {
            d.insert(b"StmF".to_vec(), PdfObject::Name(stm_f.clone()));
        }
        if let Some(ref str_f) = self.str_f {
            d.insert(b"StrF".to_vec(), PdfObject::Name(str_f.clone()));
        }

        d
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permissions_all() {
        let p = Permissions::allow_all();
        assert!(p.can_print());
        assert!(p.can_modify());
        assert!(p.can_copy());
        assert!(p.can_annotate());
        assert!(p.can_fill_forms());
        assert!(p.can_assemble());
        assert!(p.can_print_high_quality());
    }

    #[test]
    fn test_permissions_restricted() {
        // Only print allowed (bit 2)
        let p = Permissions::new(0xFFFFF0C0u32 as i32 | (1 << 2));
        assert!(p.can_print());
        assert!(!p.can_modify());
        assert!(!p.can_copy());
    }

    #[test]
    fn test_parse_encryption_dict_basic() {
        let mut d = PdfDict::new();
        d.insert(b"Filter".to_vec(), PdfObject::Name(b"Standard".to_vec()));
        d.insert(b"V".to_vec(), PdfObject::Integer(2));
        d.insert(b"R".to_vec(), PdfObject::Integer(3));
        d.insert(b"Length".to_vec(), PdfObject::Integer(128));
        d.insert(b"O".to_vec(), PdfObject::String(vec![0u8; 32]));
        d.insert(b"U".to_vec(), PdfObject::String(vec![0u8; 32]));
        d.insert(b"P".to_vec(), PdfObject::Integer(-4));

        let ed = EncryptionDict::from_dict(&d).unwrap();
        assert_eq!(ed.v, 2);
        assert_eq!(ed.r, 3);
        assert_eq!(ed.length, 128);
        assert_eq!(ed.key_length_bytes(), 16);
        assert!(ed.encrypt_metadata);
    }

    fn standard_dict(v: i64, r: i64, length: Option<i64>) -> PdfDict {
        let mut d = PdfDict::new();
        d.insert(b"Filter".to_vec(), PdfObject::Name(b"Standard".to_vec()));
        d.insert(b"V".to_vec(), PdfObject::Integer(v));
        d.insert(b"R".to_vec(), PdfObject::Integer(r));
        if let Some(length) = length {
            d.insert(b"Length".to_vec(), PdfObject::Integer(length));
        }
        d.insert(b"O".to_vec(), PdfObject::String(vec![0u8; 32]));
        d.insert(b"U".to_vec(), PdfObject::String(vec![0u8; 32]));
        d.insert(b"P".to_vec(), PdfObject::Integer(-4));
        d
    }

    fn with_std_cf(mut d: PdfDict, cfm: &[u8], length: Option<i64>) -> PdfDict {
        let mut filter = PdfDict::new();
        filter.insert(b"CFM".to_vec(), PdfObject::Name(cfm.to_vec()));
        if let Some(length) = length {
            filter.insert(b"Length".to_vec(), PdfObject::Integer(length));
        }
        let mut cf = PdfDict::new();
        cf.insert(b"StdCF".to_vec(), PdfObject::Dict(filter));
        d.insert(b"CF".to_vec(), PdfObject::Dict(cf));
        d.insert(b"StmF".to_vec(), PdfObject::Name(b"StdCF".to_vec()));
        d.insert(b"StrF".to_vec(), PdfObject::Name(b"StdCF".to_vec()));
        d
    }

    #[test]
    fn an_invalid_key_length_is_an_encryption_error() {
        for length in [0, -8, 256, 44, 136] {
            let err = EncryptionDict::from_dict(&standard_dict(2, 3, Some(length))).unwrap_err();
            assert!(
                matches!(err, crate::error::JustPdfError::EncryptionError { .. }),
                "/Length {length}: {err:?}"
            );
        }
    }

    #[test]
    fn a_key_length_below_40_is_read_as_bytes() {
        let ed = EncryptionDict::from_dict(&standard_dict(2, 3, Some(5))).unwrap();
        assert_eq!((ed.length, ed.key_length_bytes()), (40, 5));
        let ed = EncryptionDict::from_dict(&standard_dict(2, 3, Some(16))).unwrap();
        assert_eq!((ed.length, ed.key_length_bytes()), (128, 16));
    }

    #[test]
    fn v1_always_uses_a_40_bit_key() {
        let ed = EncryptionDict::from_dict(&standard_dict(1, 2, Some(128))).unwrap();
        assert_eq!((ed.length, ed.key_length_bytes()), (40, 5));
    }

    #[test]
    fn a_v4_aesv2_filter_sets_a_128_bit_key_without_a_top_level_length() {
        let d = with_std_cf(standard_dict(4, 4, None), b"AESV2", None);
        let ed = EncryptionDict::from_dict(&d).unwrap();
        assert_eq!((ed.length, ed.key_length_bytes()), (128, 16));
    }

    #[test]
    fn a_v4_filter_name_missing_from_cf_gets_the_aesv2_key_length() {
        let mut d = standard_dict(4, 4, None);
        d.insert(b"StmF".to_vec(), PdfObject::Name(b"StdCF".to_vec()));
        d.insert(b"StrF".to_vec(), PdfObject::Name(b"StdCF".to_vec()));
        let ed = EncryptionDict::from_dict(&d).unwrap();
        let state = SecurityState::new(ed.clone(), vec![], None);
        assert_eq!(state.stream_method, CryptMethod::AESV2);
        assert_eq!((ed.length, ed.key_length_bytes()), (128, 16));
    }

    #[test]
    fn r2_to_r4_key_bytes_stay_within_what_md5_gives() {
        let v5_r3 = EncryptionDict::from_dict(&standard_dict(5, 3, None)).unwrap();
        assert_eq!(v5_r3.key_length_bytes(), 16);
        let v5_r6 = EncryptionDict::from_dict(&standard_dict(5, 6, None)).unwrap();
        assert_eq!(v5_r6.key_length_bytes(), 32);
    }

    #[test]
    fn a_v4_aesv3_filter_needs_r5_or_later() {
        let d = with_std_cf(standard_dict(4, 6, None), b"AESV3", None);
        assert_eq!(EncryptionDict::from_dict(&d).unwrap().length, 256);
        let d = with_std_cf(standard_dict(4, 4, None), b"AESV3", None);
        assert!(EncryptionDict::from_dict(&d).is_err());
    }

    #[test]
    fn a_v4_rc4_filter_takes_its_own_length_in_bytes() {
        let d = with_std_cf(standard_dict(4, 4, Some(40)), b"V2", Some(16));
        let ed = EncryptionDict::from_dict(&d).unwrap();
        assert_eq!((ed.length, ed.key_length_bytes()), (128, 16));
        let d = with_std_cf(standard_dict(4, 4, Some(96)), b"V2", None);
        assert_eq!(EncryptionDict::from_dict(&d).unwrap().length, 96);
        let d = with_std_cf(standard_dict(4, 4, None), b"V2", Some(44));
        assert!(EncryptionDict::from_dict(&d).is_err());
    }

    #[test]
    fn test_encryption_dict_roundtrip() {
        let mut d = PdfDict::new();
        d.insert(b"Filter".to_vec(), PdfObject::Name(b"Standard".to_vec()));
        d.insert(b"V".to_vec(), PdfObject::Integer(4));
        d.insert(b"R".to_vec(), PdfObject::Integer(4));
        d.insert(b"Length".to_vec(), PdfObject::Integer(128));
        d.insert(b"O".to_vec(), PdfObject::String(vec![0xAB; 32]));
        d.insert(b"U".to_vec(), PdfObject::String(vec![0xCD; 32]));
        d.insert(b"P".to_vec(), PdfObject::Integer(-3904));

        let ed = EncryptionDict::from_dict(&d).unwrap();
        let back = ed.to_pdf_dict();

        assert_eq!(back.get_i64(b"V"), Some(4));
        assert_eq!(back.get_i64(b"R"), Some(4));
    }

    #[test]
    fn test_crypt_method_identity() {
        let ed = EncryptionDict {
            filter: b"Standard".to_vec(),
            v: 4,
            length: 128,
            r: 4,
            o: vec![],
            u: vec![],
            p: -4,
            encrypt_metadata: true,
            oe: None,
            ue: None,
            perms: None,
            cf: None,
            stm_f: Some(b"Identity".to_vec()),
            str_f: Some(b"Identity".to_vec()),
        };
        let state = SecurityState::new(ed, vec![], None);
        assert_eq!(state.string_method, CryptMethod::None);
        assert_eq!(state.stream_method, CryptMethod::None);
    }
}
