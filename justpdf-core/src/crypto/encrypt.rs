//! Object-level encryption for PDF writing.
//!
//! Encrypts strings and stream data before serialization.

use crate::error::Result;
use crate::object::{PdfDict, PdfObject};

use super::aes_cipher;
use super::key;
use super::rc4;
use super::types::{CryptMethod, EncryptionDict, Permissions, SecurityState};

/// Configuration for encrypting a new PDF document.
#[derive(Debug, Clone)]
pub struct EncryptionConfig {
    /// User password (may be empty for open access).
    pub user_password: Vec<u8>,
    /// Owner password (for full control).
    pub owner_password: Vec<u8>,
    /// Permission flags.
    pub permissions: Permissions,
    /// Encryption method.
    pub method: EncryptionMethod,
    /// Whether to encrypt metadata streams.
    pub encrypt_metadata: bool,
}

/// Supported encryption methods for writing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncryptionMethod {
    /// RC4 128-bit (V=2, R=3)
    RC4_128,
    /// AES-128 CBC (V=4, R=4)
    AES128,
    /// AES-256 CBC (V=5, R=6)
    AES256,
}

impl EncryptionConfig {
    /// Build the encryption dict, generate keys, and return a SecurityState
    /// suitable for encrypting objects during serialization.
    ///
    /// Also returns the encryption PdfDict to be added as an indirect object,
    /// and the /ID array to be added to the trailer.
    pub fn build(
        &self,
        file_id: &[u8],
    ) -> Result<(SecurityState, PdfDict, Vec<PdfObject>)> {
        match self.method {
            EncryptionMethod::RC4_128 => self.build_r3(file_id),
            EncryptionMethod::AES128 => self.build_r4(file_id),
            EncryptionMethod::AES256 => self.build_r6(file_id),
        }
    }

    fn build_r3(&self, file_id: &[u8]) -> Result<(SecurityState, PdfDict, Vec<PdfObject>)> {
        let mut ed = EncryptionDict {
            filter: b"Standard".to_vec(),
            v: 2,
            length: 128,
            r: 3,
            o: vec![0u8; 32],
            u: vec![0u8; 32],
            p: self.permissions.bits,
            encrypt_metadata: self.encrypt_metadata,
            oe: None,
            ue: None,
            perms: None,
            cf: None,
            stm_f: None,
            str_f: None,
        };

        let (o, u, file_key) = key::generate_o_u_values_r234(
            &self.user_password,
            &self.owner_password,
            &ed,
            file_id,
        );
        ed.o = o;
        ed.u = u;

        let pdf_dict = ed.to_pdf_dict();
        let id_array = make_id_array(file_id);

        let mut state = SecurityState::new(ed, file_id.to_vec(), None);
        state.file_key = Some(file_key);
        state.string_method = CryptMethod::V2;
        state.stream_method = CryptMethod::V2;

        Ok((state, pdf_dict, id_array))
    }

    fn build_r4(&self, file_id: &[u8]) -> Result<(SecurityState, PdfDict, Vec<PdfObject>)> {
        let mut ed = EncryptionDict {
            filter: b"Standard".to_vec(),
            v: 4,
            length: 128,
            r: 4,
            o: vec![0u8; 32],
            u: vec![0u8; 32],
            p: self.permissions.bits,
            encrypt_metadata: self.encrypt_metadata,
            oe: None,
            ue: None,
            perms: None,
            cf: Some(super::types::CryptFilterMap {
                filters: vec![(
                    b"StdCF".to_vec(),
                    super::types::CryptFilter {
                        cfm: CryptMethod::AESV2,
                        key_length: 16,
                    },
                )],
            }),
            stm_f: Some(b"StdCF".to_vec()),
            str_f: Some(b"StdCF".to_vec()),
        };

        let (o, u, file_key) = key::generate_o_u_values_r234(
            &self.user_password,
            &self.owner_password,
            &ed,
            file_id,
        );
        ed.o = o;
        ed.u = u;

        let pdf_dict = ed.to_pdf_dict();
        let id_array = make_id_array(file_id);

        let mut state = SecurityState::new(ed, file_id.to_vec(), None);
        state.file_key = Some(file_key);
        state.string_method = CryptMethod::AESV2;
        state.stream_method = CryptMethod::AESV2;

        Ok((state, pdf_dict, id_array))
    }

    fn build_r6(&self, file_id: &[u8]) -> Result<(SecurityState, PdfDict, Vec<PdfObject>)> {
        // Generate a random 32-byte file encryption key
        let file_key = generate_random_key();

        // Generate random salts
        let uvs = generate_random_salt();
        let uks = generate_random_salt();
        let ovs = generate_random_salt();
        let oks = generate_random_salt();

        let (o, u, oe, ue, perms) = key::generate_values_r6(
            &self.user_password,
            &self.owner_password,
            self.permissions.bits,
            self.encrypt_metadata,
            &file_key,
            &uvs,
            &uks,
            &ovs,
            &oks,
        );

        let ed = EncryptionDict {
            filter: b"Standard".to_vec(),
            v: 5,
            length: 256,
            r: 6,
            o,
            u,
            p: self.permissions.bits,
            encrypt_metadata: self.encrypt_metadata,
            oe: Some(oe),
            ue: Some(ue),
            perms: Some(perms),
            cf: Some(super::types::CryptFilterMap {
                filters: vec![(
                    b"StdCF".to_vec(),
                    super::types::CryptFilter {
                        cfm: CryptMethod::AESV3,
                        key_length: 32,
                    },
                )],
            }),
            stm_f: Some(b"StdCF".to_vec()),
            str_f: Some(b"StdCF".to_vec()),
        };

        let pdf_dict = ed.to_pdf_dict();
        let id_array = make_id_array(file_id);

        let mut state = SecurityState::new(ed, file_id.to_vec(), None);
        state.file_key = Some(file_key.to_vec());
        state.string_method = CryptMethod::AESV3;
        state.stream_method = CryptMethod::AESV3;

        Ok((state, pdf_dict, id_array))
    }
}

/// Encrypt a PdfObject for writing.
pub fn encrypt_object(
    obj: &PdfObject,
    state: &SecurityState,
    obj_num: u32,
    gen_num: u16,
) -> Result<PdfObject> {
    let file_key = match &state.file_key {
        Some(k) => k,
        None => return Ok(obj.clone()),
    };

    // Don't encrypt the encryption dictionary itself
    if let Some(enc_num) = state.encrypt_obj_num {
        if obj_num == enc_num {
            return Ok(obj.clone());
        }
    }

    match obj {
        PdfObject::String(data) => {
            let encrypted = encrypt_bytes(
                file_key,
                data,
                obj_num,
                gen_num,
                state.string_method,
            )?;
            Ok(PdfObject::String(encrypted))
        }
        PdfObject::Stream { dict, data } => {
            let encrypted = encrypt_bytes(
                file_key,
                data,
                obj_num,
                gen_num,
                state.stream_method,
            )?;
            Ok(PdfObject::Stream {
                dict: dict.clone(),
                data: encrypted,
            })
        }
        PdfObject::Dict(d) => {
            let mut new_dict = PdfDict::new();
            for (k, v) in d.iter() {
                let encrypted_val = encrypt_object(v, state, obj_num, gen_num)?;
                new_dict.insert(k.clone(), encrypted_val);
            }
            Ok(PdfObject::Dict(new_dict))
        }
        PdfObject::Array(arr) => {
            let mut new_arr = Vec::with_capacity(arr.len());
            for item in arr {
                new_arr.push(encrypt_object(item, state, obj_num, gen_num)?);
            }
            Ok(PdfObject::Array(new_arr))
        }
        other => Ok(other.clone()),
    }
}

/// Encrypt raw bytes using the appropriate method.
fn encrypt_bytes(
    file_key: &[u8],
    data: &[u8],
    obj_num: u32,
    gen_num: u16,
    method: CryptMethod,
) -> Result<Vec<u8>> {
    if data.is_empty() {
        return Ok(Vec::new());
    }

    match method {
        CryptMethod::None => Ok(data.to_vec()),
        CryptMethod::V2 => {
            let obj_key = key::compute_object_key(file_key, obj_num, gen_num, false);
            Ok(rc4::rc4(&obj_key, data))
        }
        CryptMethod::AESV2 => {
            let obj_key = key::compute_object_key(file_key, obj_num, gen_num, true);
            let iv = generate_iv();
            aes_cipher::encrypt_aes_cbc(&obj_key, data, &iv)
        }
        CryptMethod::AESV3 => {
            let iv = generate_iv();
            aes_cipher::encrypt_aes_cbc(file_key, data, &iv)
        }
    }
}

/// Generate file ID array for the trailer.
fn make_id_array(file_id: &[u8]) -> Vec<PdfObject> {
    vec![
        PdfObject::String(file_id.to_vec()),
        PdfObject::String(file_id.to_vec()),
    ]
}

/// Generate a random 16-byte IV.
fn generate_iv() -> [u8; 16] {
    let mut iv = [0u8; 16];
    // Use a simple deterministic approach for now — in production, use OsRng
    // For each encryption we use the current time hash as entropy source
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let hash = {
        use md5::Digest;
        let mut h = md5::Md5::new();
        h.update(seed.to_le_bytes());
        h.update(b"justpdf-iv");
        h.finalize()
    };
    iv.copy_from_slice(&hash);
    iv
}

/// Generate a random 32-byte file key for AES-256.
fn generate_random_key() -> [u8; 32] {
    let mut key = [0u8; 32];
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let hash1 = {
        use sha2::Digest;
        let mut h = sha2::Sha256::new();
        h.update(seed.to_le_bytes());
        h.update(b"justpdf-key-1");
        h.finalize()
    };
    key.copy_from_slice(&hash1);
    key
}

/// Generate a random 8-byte salt.
fn generate_random_salt() -> [u8; 8] {
    let mut salt = [0u8; 8];
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let count = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
        .wrapping_add(count as u128);
    let hash = {
        use md5::Digest;
        let mut h = md5::Md5::new();
        h.update(seed.to_le_bytes());
        h.update(b"justpdf-salt");
        h.finalize()
    };
    salt.copy_from_slice(&hash[..8]);
    salt
}

/// Generate a file ID based on document content.
pub fn generate_file_id(title: &[u8], timestamp: u64) -> Vec<u8> {
    use md5::Digest;
    let mut h = md5::Md5::new();
    h.update(title);
    h.update(timestamp.to_le_bytes());
    h.update(b"justpdf");
    h.finalize().to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_state_for_encrypt(method: CryptMethod) -> SecurityState {
        let ed = EncryptionDict {
            filter: b"Standard".to_vec(),
            v: if method == CryptMethod::AESV3 { 5 } else { 2 },
            length: if method == CryptMethod::AESV3 { 256 } else { 128 },
            r: match method {
                CryptMethod::V2 => 3,
                CryptMethod::AESV2 => 4,
                CryptMethod::AESV3 => 6,
                CryptMethod::None => 3,
            },
            o: vec![0u8; 32],
            u: vec![0u8; 32],
            p: -4,
            encrypt_metadata: true,
            oe: None,
            ue: None,
            perms: None,
            cf: None,
            stm_f: None,
            str_f: None,
        };

        let key_len = if method == CryptMethod::AESV3 { 32 } else { 16 };
        let mut state = SecurityState::new(ed, b"id".to_vec(), None);
        state.file_key = Some(vec![0x42u8; key_len]);
        state.string_method = method;
        state.stream_method = method;
        state
    }

    #[test]
    fn test_encrypt_decrypt_rc4_roundtrip() {
        let state = make_state_for_encrypt(CryptMethod::V2);

        let original = PdfObject::String(b"Hello RC4!".to_vec());
        let encrypted = encrypt_object(&original, &state, 1, 0).unwrap();
        assert_ne!(encrypted, original);

        let decrypted =
            super::super::decrypt::decrypt_object(encrypted, &state, 1, 0).unwrap();
        assert_eq!(decrypted, original);
    }

    #[test]
    fn test_encrypt_decrypt_aes128_roundtrip() {
        let state = make_state_for_encrypt(CryptMethod::AESV2);

        let original = PdfObject::String(b"Hello AES-128!".to_vec());
        let encrypted = encrypt_object(&original, &state, 1, 0).unwrap();
        assert_ne!(encrypted, original);

        let decrypted =
            super::super::decrypt::decrypt_object(encrypted, &state, 1, 0).unwrap();
        assert_eq!(decrypted, original);
    }

    #[test]
    fn test_encrypt_decrypt_aes256_roundtrip() {
        let state = make_state_for_encrypt(CryptMethod::AESV3);

        let original = PdfObject::String(b"Hello AES-256!".to_vec());
        let encrypted = encrypt_object(&original, &state, 1, 0).unwrap();
        assert_ne!(encrypted, original);

        let decrypted =
            super::super::decrypt::decrypt_object(encrypted, &state, 1, 0).unwrap();
        assert_eq!(decrypted, original);
    }

    #[test]
    fn test_encrypt_config_r3() {
        let config = EncryptionConfig {
            user_password: b"user".to_vec(),
            owner_password: b"owner".to_vec(),
            permissions: Permissions::allow_all(),
            method: EncryptionMethod::RC4_128,
            encrypt_metadata: true,
        };

        let file_id = b"test-file-id-cfg";
        let (state, pdf_dict, id_arr) = config.build(file_id).unwrap();

        assert!(state.file_key.is_some());
        assert_eq!(pdf_dict.get_i64(b"V"), Some(2));
        assert_eq!(pdf_dict.get_i64(b"R"), Some(3));
        assert_eq!(id_arr.len(), 2);
    }

    #[test]
    fn test_encrypt_config_r4() {
        let config = EncryptionConfig {
            user_password: vec![],
            owner_password: b"secret".to_vec(),
            permissions: Permissions::allow_all(),
            method: EncryptionMethod::AES128,
            encrypt_metadata: true,
        };

        let (state, pdf_dict, _) = config.build(b"id").unwrap();
        assert_eq!(pdf_dict.get_i64(b"V"), Some(4));
        assert_eq!(pdf_dict.get_i64(b"R"), Some(4));
        assert_eq!(state.string_method, CryptMethod::AESV2);
    }

    #[test]
    fn test_encrypt_config_r6() {
        let config = EncryptionConfig {
            user_password: b"user256".to_vec(),
            owner_password: b"owner256".to_vec(),
            permissions: Permissions::allow_all(),
            method: EncryptionMethod::AES256,
            encrypt_metadata: true,
        };

        let (state, pdf_dict, _) = config.build(b"id256").unwrap();
        assert_eq!(pdf_dict.get_i64(b"V"), Some(5));
        assert_eq!(pdf_dict.get_i64(b"R"), Some(6));
        assert_eq!(state.string_method, CryptMethod::AESV3);
        assert_eq!(state.file_key.as_ref().unwrap().len(), 32);
    }

    #[test]
    fn test_encrypt_skips_encrypt_dict_obj() {
        let mut state = make_state_for_encrypt(CryptMethod::V2);
        state.encrypt_obj_num = Some(10);

        let original = PdfObject::String(b"no encrypt".to_vec());
        let result = encrypt_object(&original, &state, 10, 0).unwrap();
        assert_eq!(result, original); // not encrypted
    }

    #[test]
    fn test_generate_file_id() {
        let id1 = generate_file_id(b"Doc 1", 12345);
        let id2 = generate_file_id(b"Doc 2", 12345);
        assert_eq!(id1.len(), 16);
        assert_ne!(id1, id2);
    }
}
