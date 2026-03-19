//! Object-level decryption for PDF.
//!
//! Decrypts strings and stream data using the appropriate cipher
//! based on the encryption configuration.

use crate::error::Result;
use crate::object::{PdfDict, PdfObject};

use super::aes_cipher;
use super::key;
use super::rc4;
use super::types::{CryptMethod, SecurityState};

/// Decrypt a PdfObject in place, given the security state and the object's
/// indirect reference numbers (needed for per-object key derivation in R<=4).
///
/// Returns a new PdfObject with decrypted strings and stream data.
/// Skips the encryption dictionary object itself.
pub fn decrypt_object(
    obj: PdfObject,
    state: &SecurityState,
    obj_num: u32,
    gen_num: u16,
) -> Result<PdfObject> {
    let file_key = match &state.file_key {
        Some(k) => k,
        None => return Ok(obj), // Not authenticated, return as-is
    };

    // Don't decrypt the encryption dictionary itself
    if let Some(enc_num) = state.encrypt_obj_num {
        if obj_num == enc_num {
            return Ok(obj);
        }
    }

    match obj {
        PdfObject::String(data) => {
            let decrypted = decrypt_bytes(
                file_key,
                &data,
                obj_num,
                gen_num,
                state.string_method,
                &state.encrypt_dict,
            )?;
            Ok(PdfObject::String(decrypted))
        }
        PdfObject::Stream { dict, data } => {
            // Check if stream has its own /Crypt filter
            let method = stream_crypt_method(&dict, state);
            if method == CryptMethod::None {
                // Identity — no decryption needed
                return Ok(PdfObject::Stream { dict, data });
            }

            let decrypted = decrypt_bytes(
                file_key,
                &data,
                obj_num,
                gen_num,
                method,
                &state.encrypt_dict,
            )?;

            // Remove /Crypt from the filter chain if present
            let dict = remove_crypt_filter(dict);
            Ok(PdfObject::Stream {
                dict,
                data: decrypted,
            })
        }
        PdfObject::Dict(d) => {
            let mut new_dict = PdfDict::new();
            for (k, v) in d.iter() {
                let decrypted_val = decrypt_object(v.clone(), state, obj_num, gen_num)?;
                new_dict.insert(k.clone(), decrypted_val);
            }
            Ok(PdfObject::Dict(new_dict))
        }
        PdfObject::Array(arr) => {
            let mut new_arr = Vec::with_capacity(arr.len());
            for item in arr {
                new_arr.push(decrypt_object(item, state, obj_num, gen_num)?);
            }
            Ok(PdfObject::Array(new_arr))
        }
        // Other types don't need decryption
        other => Ok(other),
    }
}

/// Decrypt raw bytes using the appropriate method.
fn decrypt_bytes(
    file_key: &[u8],
    data: &[u8],
    obj_num: u32,
    gen_num: u16,
    method: CryptMethod,
    _ed: &super::types::EncryptionDict,
) -> Result<Vec<u8>> {
    if data.is_empty() {
        return Ok(Vec::new());
    }

    match method {
        CryptMethod::None => Ok(data.to_vec()),
        CryptMethod::V2 => {
            // RC4 with per-object key
            let obj_key = key::compute_object_key(file_key, obj_num, gen_num, false);
            Ok(rc4::rc4(&obj_key, data))
        }
        CryptMethod::AESV2 => {
            // AES-128-CBC with per-object key
            let obj_key = key::compute_object_key(file_key, obj_num, gen_num, true);
            aes_cipher::decrypt_aes_cbc(&obj_key, data)
        }
        CryptMethod::AESV3 => {
            // AES-256-CBC with file key directly (no per-object key)
            aes_cipher::decrypt_aes_cbc(file_key, data)
        }
    }
}

/// Determine the crypt method for a stream, considering explicit /Crypt filter.
fn stream_crypt_method(dict: &PdfDict, state: &SecurityState) -> CryptMethod {
    // Check if stream has a /Filter array containing /Crypt
    if let Some(filters) = dict.get(b"Filter") {
        let filter_names: Vec<&[u8]> = match filters {
            PdfObject::Name(n) => vec![n.as_slice()],
            PdfObject::Array(arr) => arr
                .iter()
                .filter_map(|o| o.as_name())
                .collect(),
            _ => vec![],
        };

        // If /Crypt filter is present, check DecodeParms for the filter name
        if filter_names.contains(&b"Crypt".as_slice()) {
            // Look for the crypt filter name in DecodeParms
            if let Some(params) = dict.get(b"DecodeParms") {
                let filter_name = extract_crypt_filter_name(params, &filter_names);
                if let Some(name) = filter_name {
                    if name == b"Identity" {
                        return CryptMethod::None;
                    }
                    // Look up in CF map
                    if let Some(ref cf) = state.encrypt_dict.cf {
                        for (n, f) in &cf.filters {
                            if n == name {
                                return f.cfm;
                            }
                        }
                    }
                }
            }
        }
    }

    // Use default stream crypt method
    state.stream_method
}

/// Extract the /Name from DecodeParms corresponding to /Crypt filter.
fn extract_crypt_filter_name<'a>(
    params: &'a PdfObject,
    filter_names: &[&[u8]],
) -> Option<&'a [u8]> {
    match params {
        PdfObject::Dict(d) => {
            // Single filter case
            d.get_name(b"Name")
        }
        PdfObject::Array(arr) => {
            // Find the DecodeParms entry corresponding to the /Crypt filter
            for (i, name) in filter_names.iter().enumerate() {
                if *name == b"Crypt" {
                    if let Some(PdfObject::Dict(d)) = arr.get(i) {
                        return d.get_name(b"Name");
                    }
                }
            }
            None
        }
        _ => None,
    }
}

/// Remove /Crypt from the filter chain in a stream dict.
fn remove_crypt_filter(mut dict: PdfDict) -> PdfDict {
    if let Some(filter) = dict.get(b"Filter").cloned() {
        match filter {
            PdfObject::Name(ref n) if n == b"Crypt" => {
                dict.remove(b"Filter");
                dict.remove(b"DecodeParms");
            }
            PdfObject::Array(ref arr) => {
                let crypt_indices: Vec<usize> = arr
                    .iter()
                    .enumerate()
                    .filter_map(|(i, o)| {
                        if o.as_name() == Some(b"Crypt") {
                            Some(i)
                        } else {
                            None
                        }
                    })
                    .collect();

                if !crypt_indices.is_empty() {
                    let mut new_filters: Vec<PdfObject> = arr
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| !crypt_indices.contains(i))
                        .map(|(_, o)| o.clone())
                        .collect();

                    // Also remove corresponding DecodeParms entries
                    if let Some(PdfObject::Array(params)) = dict.get(b"DecodeParms").cloned() {
                        let new_params: Vec<PdfObject> = params
                            .iter()
                            .enumerate()
                            .filter(|(i, _)| !crypt_indices.contains(i))
                            .map(|(_, o)| o.clone())
                            .collect();
                        if new_params.is_empty() {
                            dict.remove(b"DecodeParms");
                        } else if new_params.len() == 1 {
                            dict.insert(b"DecodeParms".to_vec(), new_params.into_iter().next().unwrap());
                        } else {
                            dict.insert(b"DecodeParms".to_vec(), PdfObject::Array(new_params));
                        }
                    }

                    if new_filters.is_empty() {
                        dict.remove(b"Filter");
                    } else if new_filters.len() == 1 {
                        dict.insert(b"Filter".to_vec(), new_filters.remove(0));
                    } else {
                        dict.insert(b"Filter".to_vec(), PdfObject::Array(new_filters));
                    }
                }
            }
            _ => {}
        }
    }
    dict
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::types::{CryptMethod, EncryptionDict, SecurityState};

    fn make_state(method: CryptMethod) -> SecurityState {
        let ed = EncryptionDict {
            filter: b"Standard".to_vec(),
            v: 2,
            length: 128,
            r: 3,
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
        let mut state = SecurityState::new(ed, b"file-id".to_vec(), None);
        state.file_key = Some(vec![0x42u8; 16]);
        state.string_method = method;
        state.stream_method = method;
        state
    }

    #[test]
    fn test_decrypt_rc4_string_roundtrip() {
        let state = make_state(CryptMethod::V2);
        let file_key = state.file_key.as_ref().unwrap();
        let obj_key = key::compute_object_key(file_key, 1, 0, false);

        let plaintext = b"Hello, World!";
        let encrypted = rc4::rc4(&obj_key, plaintext);

        let obj = PdfObject::String(encrypted);
        let decrypted = decrypt_object(obj, &state, 1, 0).unwrap();

        if let PdfObject::String(data) = decrypted {
            assert_eq!(data, plaintext);
        } else {
            panic!("expected String");
        }
    }

    #[test]
    fn test_decrypt_aes128_string_roundtrip() {
        let state = make_state(CryptMethod::AESV2);
        let file_key = state.file_key.as_ref().unwrap();
        let obj_key = key::compute_object_key(file_key, 5, 0, true);

        let plaintext = b"AES encrypted string";
        let iv = [0u8; 16];
        let encrypted = aes_cipher::encrypt_aes_cbc(&obj_key, plaintext, &iv).unwrap();

        let obj = PdfObject::String(encrypted);
        let decrypted = decrypt_object(obj, &state, 5, 0).unwrap();

        if let PdfObject::String(data) = decrypted {
            assert_eq!(data, plaintext);
        } else {
            panic!("expected String");
        }
    }

    #[test]
    fn test_decrypt_skips_encrypt_dict() {
        let ed = EncryptionDict {
            filter: b"Standard".to_vec(),
            v: 2,
            length: 128,
            r: 3,
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
        let mut state = SecurityState::new(ed, b"id".to_vec(), Some(99));
        state.file_key = Some(vec![0x42u8; 16]);

        let original = PdfObject::String(b"should not decrypt".to_vec());
        let result = decrypt_object(original.clone(), &state, 99, 0).unwrap();
        assert_eq!(result, original);
    }

    #[test]
    fn test_decrypt_no_key() {
        let ed = EncryptionDict {
            filter: b"Standard".to_vec(),
            v: 2,
            length: 128,
            r: 3,
            o: vec![],
            u: vec![],
            p: 0,
            encrypt_metadata: true,
            oe: None,
            ue: None,
            perms: None,
            cf: None,
            stm_f: None,
            str_f: None,
        };
        let state = SecurityState::new(ed, vec![], None);

        let original = PdfObject::String(b"data".to_vec());
        let result = decrypt_object(original.clone(), &state, 1, 0).unwrap();
        assert_eq!(result, original);
    }

    #[test]
    fn test_decrypt_recursive_dict() {
        let state = make_state(CryptMethod::V2);
        let file_key = state.file_key.as_ref().unwrap();
        let obj_key = key::compute_object_key(file_key, 3, 0, false);

        let encrypted = rc4::rc4(&obj_key, b"test");

        let mut inner = PdfDict::new();
        inner.insert(b"Key".to_vec(), PdfObject::String(encrypted));

        let obj = PdfObject::Dict(inner);
        let decrypted = decrypt_object(obj, &state, 3, 0).unwrap();

        if let PdfObject::Dict(d) = decrypted {
            assert_eq!(d.get_string(b"Key"), Some(b"test".as_slice()));
        } else {
            panic!("expected Dict");
        }
    }

    #[test]
    fn test_remove_crypt_filter_single() {
        let mut dict = PdfDict::new();
        dict.insert(b"Filter".to_vec(), PdfObject::Name(b"Crypt".to_vec()));
        let cleaned = remove_crypt_filter(dict);
        assert!(cleaned.get(b"Filter").is_none());
    }

    #[test]
    fn test_remove_crypt_filter_array() {
        let mut dict = PdfDict::new();
        dict.insert(
            b"Filter".to_vec(),
            PdfObject::Array(vec![
                PdfObject::Name(b"Crypt".to_vec()),
                PdfObject::Name(b"FlateDecode".to_vec()),
            ]),
        );
        let cleaned = remove_crypt_filter(dict);
        // Should have single FlateDecode remaining
        assert_eq!(
            cleaned.get(b"Filter"),
            Some(&PdfObject::Name(b"FlateDecode".to_vec()))
        );
    }
}
