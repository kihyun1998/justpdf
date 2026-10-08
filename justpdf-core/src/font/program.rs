//! Decoded font programs, as [`crate::PdfDocument::font_program`] shares them.

use std::sync::Arc;

/// A decoded font program and its [`font_hash`].
#[derive(Debug)]
pub struct FontProgramData {
    /// The decoded program bytes.
    pub data: Arc<[u8]>,
    /// [`font_hash`] of `data`.
    pub hash: u64,
}

impl FontProgramData {
    /// `data` with its [`font_hash`].
    pub fn new(data: impl Into<Arc<[u8]>>) -> Self {
        let data = data.into();
        let hash = font_hash(&data);
        Self { data, hash }
    }
}

/// FNV-1a 64-bit hash of a font program's length and all of its bytes.
pub fn font_hash(data: &[u8]) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x00000100000001B3;

    let mut hash = FNV_OFFSET;
    for &b in (data.len() as u64).to_le_bytes().iter().chain(data) {
        hash ^= b as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}
