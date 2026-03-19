//! PDF encryption and decryption (Phase 6).
//!
//! Supports the Standard security handler with:
//! - RC4 40-bit and 128-bit (V=1/2, R=2/3)
//! - AES-128 CBC (V=4, R=4)
//! - AES-256 CBC (V=5, R=5/6)

mod aes_cipher;
pub(crate) mod auth;
mod decrypt;
mod encrypt;
mod key;
mod rc4;
mod types;

pub use decrypt::decrypt_object;
pub use encrypt::{encrypt_object, generate_file_id, EncryptionConfig, EncryptionMethod};
pub use types::{CryptFilter, CryptMethod, EncryptionDict, Permissions, SecurityState};
