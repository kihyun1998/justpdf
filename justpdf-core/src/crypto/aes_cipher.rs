//! AES-CBC encryption/decryption for PDF.
//!
//! AES-128 CBC is used in V=4 (R=4), AES-256 CBC in V=5 (R=5/6).
//! Per PDF spec: first 16 bytes of encrypted data are the IV,
//! followed by CBC-encrypted data with PKCS#7 padding.

use aes::Aes128;
use aes::Aes256;
use cbc::cipher::{BlockDecryptMut, BlockEncryptMut, KeyIvInit};

use crate::error::{JustPdfError, Result};

type Aes128CbcDec = cbc::Decryptor<Aes128>;
type Aes128CbcEnc = cbc::Encryptor<Aes128>;
type Aes256CbcDec = cbc::Decryptor<Aes256>;
type Aes256CbcEnc = cbc::Encryptor<Aes256>;

/// Decrypt AES-CBC data. The first 16 bytes are the IV.
/// Removes PKCS#7 padding after decryption.
pub fn decrypt_aes_cbc(key: &[u8], data: &[u8]) -> Result<Vec<u8>> {
    if data.len() < 16 {
        return Err(JustPdfError::EncryptionError {
            detail: "AES data too short (no IV)".into(),
        });
    }

    let iv = &data[..16];
    let ciphertext = &data[16..];

    if ciphertext.is_empty() {
        return Ok(Vec::new());
    }

    if ciphertext.len() % 16 != 0 {
        return Err(JustPdfError::EncryptionError {
            detail: format!(
                "AES ciphertext length {} not multiple of 16",
                ciphertext.len()
            ),
        });
    }

    let mut buf = ciphertext.to_vec();

    match key.len() {
        16 => {
            let decryptor = Aes128CbcDec::new_from_slices(key, iv).map_err(|e| {
                JustPdfError::EncryptionError {
                    detail: format!("AES-128 init error: {}", e),
                }
            })?;
            decryptor
                .decrypt_padded_mut::<cbc::cipher::block_padding::NoPadding>(&mut buf)
                .map_err(|e| JustPdfError::EncryptionError {
                    detail: format!("AES-128 decrypt error: {}", e),
                })?;
        }
        32 => {
            let decryptor = Aes256CbcDec::new_from_slices(key, iv).map_err(|e| {
                JustPdfError::EncryptionError {
                    detail: format!("AES-256 init error: {}", e),
                }
            })?;
            decryptor
                .decrypt_padded_mut::<cbc::cipher::block_padding::NoPadding>(&mut buf)
                .map_err(|e| JustPdfError::EncryptionError {
                    detail: format!("AES-256 decrypt error: {}", e),
                })?;
        }
        _ => {
            return Err(JustPdfError::EncryptionError {
                detail: format!("unsupported AES key length: {}", key.len()),
            });
        }
    }

    // Remove PKCS#7 padding
    remove_pkcs7_padding(&mut buf)?;
    Ok(buf)
}

/// Encrypt data using AES-CBC. Prepends a random 16-byte IV.
/// Adds PKCS#7 padding.
pub fn encrypt_aes_cbc(key: &[u8], data: &[u8], iv: &[u8; 16]) -> Result<Vec<u8>> {
    // Add PKCS#7 padding
    let pad_len = 16 - (data.len() % 16);
    let mut padded = data.to_vec();
    padded.extend(std::iter::repeat(pad_len as u8).take(pad_len));

    let mut buf = padded;

    match key.len() {
        16 => {
            let encryptor = Aes128CbcEnc::new_from_slices(key, iv).map_err(|e| {
                JustPdfError::EncryptionError {
                    detail: format!("AES-128 init error: {}", e),
                }
            })?;
            let buf_len = buf.len();
            let ct = encryptor
                .encrypt_padded_mut::<cbc::cipher::block_padding::NoPadding>(&mut buf, buf_len)
                .map_err(|e| JustPdfError::EncryptionError {
                    detail: format!("AES-128 encrypt error: {}", e),
                })?;
            let mut result = Vec::with_capacity(16 + ct.len());
            result.extend_from_slice(iv);
            result.extend_from_slice(ct);
            Ok(result)
        }
        32 => {
            let encryptor = Aes256CbcEnc::new_from_slices(key, iv).map_err(|e| {
                JustPdfError::EncryptionError {
                    detail: format!("AES-256 init error: {}", e),
                }
            })?;
            let buf_len = buf.len();
            let ct = encryptor
                .encrypt_padded_mut::<cbc::cipher::block_padding::NoPadding>(&mut buf, buf_len)
                .map_err(|e| JustPdfError::EncryptionError {
                    detail: format!("AES-256 encrypt error: {}", e),
                })?;
            let mut result = Vec::with_capacity(16 + ct.len());
            result.extend_from_slice(iv);
            result.extend_from_slice(ct);
            Ok(result)
        }
        _ => Err(JustPdfError::EncryptionError {
            detail: format!("unsupported AES key length: {}", key.len()),
        }),
    }
}

/// Remove PKCS#7 padding from decrypted data.
fn remove_pkcs7_padding(data: &mut Vec<u8>) -> Result<()> {
    if data.is_empty() {
        return Ok(());
    }

    let pad_byte = *data.last().unwrap();
    let pad_len = pad_byte as usize;

    if pad_len == 0 || pad_len > 16 || pad_len > data.len() {
        return Err(JustPdfError::EncryptionError {
            detail: format!("invalid PKCS#7 padding byte: {}", pad_byte),
        });
    }

    // Verify all padding bytes
    for &b in &data[data.len() - pad_len..] {
        if b != pad_byte {
            return Err(JustPdfError::EncryptionError {
                detail: "invalid PKCS#7 padding".into(),
            });
        }
    }

    data.truncate(data.len() - pad_len);
    Ok(())
}

/// Decrypt a single AES-256 ECB block (used in R=6 key derivation).
pub fn decrypt_aes256_ecb_block(key: &[u8; 32], block: &[u8; 16]) -> [u8; 16] {
    use aes::cipher::{BlockDecrypt, KeyInit};
    let cipher = Aes256::new(key.into());
    let mut b = aes::Block::clone_from_slice(block);
    cipher.decrypt_block(&mut b);
    let mut out = [0u8; 16];
    out.copy_from_slice(&b);
    out
}

/// Encrypt a single AES-256 ECB block (used in R=6 /Perms generation).
pub fn encrypt_aes256_ecb_block(key: &[u8; 32], block: &[u8; 16]) -> [u8; 16] {
    use aes::cipher::{BlockEncrypt, KeyInit};
    let cipher = Aes256::new(key.into());
    let mut b = aes::Block::clone_from_slice(block);
    cipher.encrypt_block(&mut b);
    let mut out = [0u8; 16];
    out.copy_from_slice(&b);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aes128_roundtrip() {
        let key = [0x42u8; 16];
        let iv = [0x00u8; 16];
        let plaintext = b"Hello AES-128!!";

        let encrypted = encrypt_aes_cbc(&key, plaintext, &iv).unwrap();
        assert_ne!(&encrypted[16..], plaintext.as_slice());

        let decrypted = decrypt_aes_cbc(&key, &encrypted).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_aes256_roundtrip() {
        let key = [0x42u8; 32];
        let iv = [0x00u8; 16];
        let plaintext = b"Hello AES-256 encryption test data!";

        let encrypted = encrypt_aes_cbc(&key, plaintext, &iv).unwrap();
        let decrypted = decrypt_aes_cbc(&key, &encrypted).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_aes_empty_data() {
        let key = [0x42u8; 16];
        let iv = [0x00u8; 16];
        let encrypted = encrypt_aes_cbc(&key, b"", &iv).unwrap();
        let decrypted = decrypt_aes_cbc(&key, &encrypted).unwrap();
        assert!(decrypted.is_empty());
    }

    #[test]
    fn test_aes_short_data_error() {
        let key = [0x42u8; 16];
        let result = decrypt_aes_cbc(&key, &[0u8; 10]);
        assert!(result.is_err());
    }

    #[test]
    fn test_aes256_ecb_roundtrip() {
        let key = [0x42u8; 32];
        let block = [0x01u8; 16];
        let encrypted = encrypt_aes256_ecb_block(&key, &block);
        let decrypted = decrypt_aes256_ecb_block(&key, &encrypted);
        assert_eq!(decrypted, block);
    }
}
