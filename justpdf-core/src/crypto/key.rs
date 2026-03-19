//! PDF encryption key derivation algorithms.
//!
//! Implements key derivation for the Standard security handler:
//! - Algorithm 2 (R=2/3/4): MD5-based file encryption key
//! - Algorithm 2.A (R=5): Unwrap file key from /UE or /OE using SHA-256
//! - Algorithm 2.B (R=6): Extended hash with SHA-256/384/512 rotation

use md5::{Digest, Md5};
use sha2::{Sha256, Sha384, Sha512};

use super::types::EncryptionDict;

/// The PDF "password padding" constant (32 bytes) from Table 3.18 / ISO 32000.
pub const PADDING: [u8; 32] = [
    0x28, 0xBF, 0x4E, 0x5E, 0x4E, 0x75, 0x8A, 0x41, 0x64, 0x00, 0x4E, 0x56, 0xFF, 0xFA, 0x01,
    0x08, 0x2E, 0x2E, 0x00, 0xB6, 0xD0, 0x68, 0x3E, 0x80, 0x2F, 0x0C, 0xA9, 0xFE, 0x64, 0x53,
    0x69, 0x7A,
];

/// Pad a password to 32 bytes using the PDF padding constant.
pub fn pad_password(password: &[u8]) -> [u8; 32] {
    let mut padded = [0u8; 32];
    let len = password.len().min(32);
    padded[..len].copy_from_slice(&password[..len]);
    padded[len..].copy_from_slice(&PADDING[..32 - len]);
    padded
}

/// Algorithm 2: Compute the file encryption key (R=2/3/4).
///
/// PDF spec ISO 32000-1:2008, 7.6.3.3 (Algorithm 2).
pub fn compute_file_encryption_key_r234(
    password: &[u8],
    ed: &EncryptionDict,
    file_id: &[u8],
) -> Vec<u8> {
    let padded = pad_password(password);
    let key_len = ed.key_length_bytes();

    let mut hasher = Md5::new();
    hasher.update(padded);
    hasher.update(&ed.o);
    hasher.update(&(ed.p as u32).to_le_bytes());
    hasher.update(file_id);

    if !ed.encrypt_metadata && ed.r >= 4 {
        hasher.update([0xFF, 0xFF, 0xFF, 0xFF]);
    }

    let mut hash = hasher.finalize().to_vec();

    // For R>=3, iterate MD5 50 times
    if ed.r >= 3 {
        for _ in 0..50 {
            let mut h = Md5::new();
            h.update(&hash[..key_len]);
            hash = h.finalize().to_vec();
        }
    }

    hash.truncate(key_len);
    hash
}

/// Algorithm 3: Compute the owner password value /O (R=2/3/4).
///
/// This creates the /O entry that gets stored in the encryption dict.
pub fn compute_o_value_r234(
    owner_password: &[u8],
    user_password: &[u8],
    ed: &EncryptionDict,
) -> Vec<u8> {
    let owner_padded = pad_password(owner_password);
    let key_len = ed.key_length_bytes();

    let mut hasher = Md5::new();
    hasher.update(owner_padded);
    let mut hash = hasher.finalize().to_vec();

    if ed.r >= 3 {
        for _ in 0..50 {
            let mut h = Md5::new();
            h.update(&hash[..key_len]);
            hash = h.finalize().to_vec();
        }
    }

    let rc4_key = &hash[..key_len];
    let user_padded = pad_password(user_password);
    let mut result = super::rc4::rc4(rc4_key, &user_padded);

    if ed.r >= 3 {
        for i in 1..=19u8 {
            let mut xor_key = vec![0u8; key_len];
            for (j, byte) in rc4_key.iter().enumerate() {
                xor_key[j] = byte ^ i;
            }
            result = super::rc4::rc4(&xor_key, &result);
        }
    }

    result
}

/// Algorithm 4/5: Compute the user password value /U (R=2/3/4).
pub fn compute_u_value_r234(
    file_key: &[u8],
    ed: &EncryptionDict,
    file_id: &[u8],
) -> Vec<u8> {
    if ed.r == 2 {
        // Algorithm 4: Simple RC4 encryption of the padding
        super::rc4::rc4(file_key, &PADDING)
    } else {
        // Algorithm 5: MD5(padding + file_id), then RC4 x 20
        let mut hasher = Md5::new();
        hasher.update(PADDING);
        hasher.update(file_id);
        let mut result = hasher.finalize().to_vec();

        result = super::rc4::rc4(file_key, &result);

        for i in 1..=19u8 {
            let mut xor_key = vec![0u8; file_key.len()];
            for (j, byte) in file_key.iter().enumerate() {
                xor_key[j] = byte ^ i;
            }
            result = super::rc4::rc4(&xor_key, &result);
        }

        // Pad to 32 bytes with arbitrary data
        result.resize(32, 0);
        result
    }
}

/// Compute the owner key for R=2 (Algorithm 7 step for owner password auth).
///
/// Recover the user password from /O using the owner password, then derive the file key.
pub fn recover_user_password_from_owner_r234(
    owner_password: &[u8],
    ed: &EncryptionDict,
) -> Vec<u8> {
    let owner_padded = pad_password(owner_password);
    let key_len = ed.key_length_bytes();

    let mut hasher = Md5::new();
    hasher.update(owner_padded);
    let mut hash = hasher.finalize().to_vec();

    if ed.r >= 3 {
        for _ in 0..50 {
            let mut h = Md5::new();
            h.update(&hash[..key_len]);
            hash = h.finalize().to_vec();
        }
    }

    let rc4_key = &hash[..key_len];

    if ed.r == 2 {
        super::rc4::rc4(rc4_key, &ed.o)
    } else {
        // Reverse the 20 RC4 rounds
        let mut result = ed.o.clone();
        for i in (0..=19u8).rev() {
            let mut xor_key = vec![0u8; key_len];
            for (j, byte) in rc4_key.iter().enumerate() {
                xor_key[j] = byte ^ i;
            }
            result = super::rc4::rc4(&xor_key, &result);
        }
        result
    }
}

// --- AES-256 (R=5/6) key derivation ---

/// Algorithm 2.A (R=5): Compute file encryption key from /UE or /OE.
///
/// For R=5 (deprecated): SHA-256(password + validation_salt + U_or_O) → key for AES-256-CBC decrypt of /UE or /OE.
pub fn compute_file_key_r5(
    password: &[u8],
    key_salt: &[u8],
    u_or_o: &[u8],
    encrypted_key: &[u8],
) -> Option<Vec<u8>> {
    if encrypted_key.len() < 32 {
        return None;
    }

    let mut hasher = Sha256::new();
    let pw = if password.len() > 127 {
        &password[..127]
    } else {
        password
    };
    hasher.update(pw);
    hasher.update(key_salt);
    hasher.update(u_or_o);
    let hash = hasher.finalize();

    // Decrypt /UE or /OE using AES-256-CBC with zero IV
    let iv = [0u8; 16];
    let key: [u8; 32] = hash.into();

    // The encrypted_key is 32 bytes, no IV prefix for this specific operation
    if encrypted_key.len() < 32 {
        return None;
    }

    // AES-256-CBC decrypt with zero IV, no padding
    use aes::Aes256;
    use cbc::cipher::{BlockDecryptMut, KeyIvInit};
    type Aes256CbcDec = cbc::Decryptor<Aes256>;

    let mut buf = encrypted_key[..32].to_vec();
    let decryptor = Aes256CbcDec::new_from_slices(&key, &iv).ok()?;
    decryptor
        .decrypt_padded_mut::<cbc::cipher::block_padding::NoPadding>(&mut buf)
        .ok()?;

    Some(buf)
}

/// Algorithm 2.B (R=6): Extended hash computation.
///
/// Uses SHA-256/384/512 rotation based on the last byte of each round.
/// 64 rounds minimum, continues while round number < last_byte + 32.
pub fn compute_hash_r6(
    password: &[u8],
    salt: &[u8],
    u_bytes: &[u8],
) -> [u8; 32] {
    let pw = if password.len() > 127 {
        &password[..127]
    } else {
        password
    };

    // Initial hash: SHA-256(password + salt + U)
    let mut hasher = Sha256::new();
    hasher.update(pw);
    hasher.update(salt);
    hasher.update(u_bytes);
    let mut k = hasher.finalize().to_vec();

    let mut round = 0u32;
    loop {
        // Build K1 = password + K + U, repeated 64 times
        let mut k1 = Vec::with_capacity((pw.len() + k.len() + u_bytes.len()) * 64);
        for _ in 0..64 {
            k1.extend_from_slice(pw);
            k1.extend_from_slice(&k);
            k1.extend_from_slice(u_bytes);
        }

        // AES-128-CBC encrypt K1 with key=K[0..16], IV=K[16..32]
        let aes_key = &k[..16];
        let iv = &k[16..32];
        let encrypted = aes128_cbc_encrypt_no_padding(aes_key, iv, &k1);

        // Sum first 16 bytes mod 3 to choose hash
        let sum: u64 = encrypted[..16].iter().map(|&b| b as u64).sum();
        let hash_choice = sum % 3;

        k = match hash_choice {
            0 => {
                let mut h = Sha256::new();
                h.update(&encrypted);
                h.finalize().to_vec()
            }
            1 => {
                let mut h = Sha384::new();
                h.update(&encrypted);
                h.finalize().to_vec()
            }
            _ => {
                let mut h = Sha512::new();
                h.update(&encrypted);
                h.finalize().to_vec()
            }
        };

        let last_byte = *encrypted.last().unwrap_or(&0);
        round += 1;

        if round >= 64 && last_byte <= (round - 32) as u8 {
            break;
        }
    }

    let mut result = [0u8; 32];
    result.copy_from_slice(&k[..32]);
    result
}

/// Compute file encryption key for R=6 using /UE (user key entry).
pub fn compute_file_key_r6_user(
    password: &[u8],
    u_value: &[u8],
    ue_value: &[u8],
) -> Option<Vec<u8>> {
    if u_value.len() < 48 || ue_value.len() < 32 {
        return None;
    }

    let key_salt = &u_value[40..48];
    let hash = compute_hash_r6(password, key_salt, &[]);

    // Decrypt /UE with this hash as AES-256-CBC key, zero IV
    let iv = [0u8; 16];
    use aes::Aes256;
    use cbc::cipher::{BlockDecryptMut, KeyIvInit};
    type Aes256CbcDec = cbc::Decryptor<Aes256>;

    let key: [u8; 32] = hash;
    let mut buf = ue_value[..32].to_vec();
    let decryptor = Aes256CbcDec::new_from_slices(&key, &iv).ok()?;
    decryptor
        .decrypt_padded_mut::<cbc::cipher::block_padding::NoPadding>(&mut buf)
        .ok()?;

    Some(buf)
}

/// Compute file encryption key for R=6 using /OE (owner key entry).
pub fn compute_file_key_r6_owner(
    password: &[u8],
    o_value: &[u8],
    oe_value: &[u8],
    u_value: &[u8],
) -> Option<Vec<u8>> {
    if o_value.len() < 48 || oe_value.len() < 32 {
        return None;
    }

    let key_salt = &o_value[40..48];
    // For owner, U is used as additional input
    let u_trunc = if u_value.len() >= 48 {
        &u_value[..48]
    } else {
        u_value
    };
    let hash = compute_hash_r6(password, key_salt, u_trunc);

    let iv = [0u8; 16];
    use aes::Aes256;
    use cbc::cipher::{BlockDecryptMut, KeyIvInit};
    type Aes256CbcDec = cbc::Decryptor<Aes256>;

    let key: [u8; 32] = hash;
    let mut buf = oe_value[..32].to_vec();
    let decryptor = Aes256CbcDec::new_from_slices(&key, &iv).ok()?;
    decryptor
        .decrypt_padded_mut::<cbc::cipher::block_padding::NoPadding>(&mut buf)
        .ok()?;

    Some(buf)
}

/// Per-object key derivation for R=2/3/4.
///
/// PDF spec Algorithm 1: key = MD5(file_key + obj_num_le + gen_num_le [+ "sAlT" for AES]).
pub fn compute_object_key(
    file_key: &[u8],
    obj_num: u32,
    gen_num: u16,
    is_aes: bool,
) -> Vec<u8> {
    let mut hasher = Md5::new();
    hasher.update(file_key);
    hasher.update(&obj_num.to_le_bytes()[..3]);
    hasher.update(&gen_num.to_le_bytes()[..2]);

    if is_aes {
        hasher.update(b"sAlT");
    }

    let hash = hasher.finalize();
    let key_len = (file_key.len() + 5).min(16);
    hash[..key_len].to_vec()
}

// --- Helper for R=6 hash computation ---

/// AES-128-CBC encrypt without padding (data must be aligned to 16 bytes).
fn aes128_cbc_encrypt_no_padding(key: &[u8], iv: &[u8], data: &[u8]) -> Vec<u8> {
    use aes::Aes128;
    use cbc::cipher::{BlockEncryptMut, KeyIvInit};
    type Aes128CbcEnc = cbc::Encryptor<Aes128>;

    // Data may not be aligned; pad to 16 bytes if needed
    let aligned_len = ((data.len() + 15) / 16) * 16;
    let mut buf = vec![0u8; aligned_len];
    buf[..data.len()].copy_from_slice(data);

    let encryptor = Aes128CbcEnc::new_from_slices(key, iv).unwrap();
    let ct_len = buf.len();
    encryptor
        .encrypt_padded_mut::<cbc::cipher::block_padding::NoPadding>(&mut buf, ct_len)
        .unwrap();

    buf
}

// --- Key generation for writing encrypted PDFs ---

/// Generate /O and /U values for a new encryption dict (R=2/3/4).
pub fn generate_o_u_values_r234(
    user_password: &[u8],
    owner_password: &[u8],
    ed: &EncryptionDict,
    file_id: &[u8],
) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    // 1. Compute /O using owner password
    let o_value = compute_o_value_r234(owner_password, user_password, ed);

    // 2. Create a temporary dict with /O set to compute the file key
    let mut temp_ed = ed.clone();
    temp_ed.o = o_value.clone();

    // 3. Compute file encryption key using user password
    let file_key = compute_file_encryption_key_r234(user_password, &temp_ed, file_id);

    // 4. Compute /U using file key
    let u_value = compute_u_value_r234(&file_key, &temp_ed, file_id);

    (o_value, u_value, file_key)
}

/// Generate encryption entries for R=6 (AES-256).
///
/// Returns (O, U, OE, UE, Perms, file_key).
pub fn generate_values_r6(
    user_password: &[u8],
    owner_password: &[u8],
    permissions: i32,
    encrypt_metadata: bool,
    file_key: &[u8; 32],
    user_validation_salt: &[u8; 8],
    user_key_salt: &[u8; 8],
    owner_validation_salt: &[u8; 8],
    owner_key_salt: &[u8; 8],
) -> (Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>) {
    let pw_u = if user_password.len() > 127 {
        &user_password[..127]
    } else {
        user_password
    };
    let pw_o = if owner_password.len() > 127 {
        &owner_password[..127]
    } else {
        owner_password
    };

    // Compute U value: SHA-256(password + validation_salt) ∥ validation_salt ∥ key_salt
    let u_hash = compute_hash_r6(pw_u, user_validation_salt, &[]);
    let mut u_value = Vec::with_capacity(48);
    u_value.extend_from_slice(&u_hash);
    u_value.extend_from_slice(user_validation_salt);
    u_value.extend_from_slice(user_key_salt);

    // Compute UE: AES-256-CBC(key=SHA-256(password + key_salt), iv=0) of file_key
    let ue_hash = compute_hash_r6(pw_u, user_key_salt, &[]);
    let iv = [0u8; 16];
    let ue_value = aes256_cbc_encrypt_no_padding(&ue_hash, &iv, file_key);

    // Compute O value: SHA-256(password + validation_salt + U) ∥ validation_salt ∥ key_salt
    let o_hash = compute_hash_r6(pw_o, owner_validation_salt, &u_value[..48]);
    let mut o_value = Vec::with_capacity(48);
    o_value.extend_from_slice(&o_hash);
    o_value.extend_from_slice(owner_validation_salt);
    o_value.extend_from_slice(owner_key_salt);

    // Compute OE: AES-256-CBC(key=SHA-256(password + key_salt + U), iv=0) of file_key
    let oe_hash = compute_hash_r6(pw_o, owner_key_salt, &u_value[..48]);
    let oe_value = aes256_cbc_encrypt_no_padding(&oe_hash, &iv, file_key);

    // Compute Perms: encrypt permissions block with AES-256-ECB
    let mut perms_block = [0u8; 16];
    let p_bytes = (permissions as u32).to_le_bytes();
    perms_block[..4].copy_from_slice(&p_bytes);
    perms_block[4..8].fill(0xFF); // upper 32 bits of permissions
    perms_block[8] = if encrypt_metadata { b'T' } else { b'F' };
    perms_block[9] = b'a';
    perms_block[10] = b'd';
    perms_block[11] = b'b';
    // bytes 12-15 are random, leave as zeros for determinism

    let perms_value = super::aes_cipher::encrypt_aes256_ecb_block(file_key, &perms_block);

    (o_value, u_value, oe_value.to_vec(), ue_value.to_vec(), perms_value.to_vec())
}

/// AES-256-CBC encrypt without padding (for R=6 key generation).
fn aes256_cbc_encrypt_no_padding(key: &[u8; 32], iv: &[u8; 16], data: &[u8]) -> Vec<u8> {
    use aes::Aes256;
    use cbc::cipher::{BlockEncryptMut, KeyIvInit};
    type Aes256CbcEnc = cbc::Encryptor<Aes256>;

    let aligned_len = ((data.len() + 15) / 16) * 16;
    let mut buf = vec![0u8; aligned_len];
    buf[..data.len()].copy_from_slice(data);

    let encryptor = Aes256CbcEnc::new_from_slices(key, iv).unwrap();
    let ct_len = buf.len();
    encryptor
        .encrypt_padded_mut::<cbc::cipher::block_padding::NoPadding>(&mut buf, ct_len)
        .unwrap();

    buf
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pad_password_empty() {
        let padded = pad_password(b"");
        assert_eq!(padded, PADDING);
    }

    #[test]
    fn test_pad_password_short() {
        let padded = pad_password(b"test");
        assert_eq!(&padded[..4], b"test");
        assert_eq!(&padded[4..], &PADDING[..28]);
    }

    #[test]
    fn test_pad_password_long() {
        let long_pw = [b'A'; 64];
        let padded = pad_password(&long_pw);
        assert_eq!(padded, [b'A'; 32]);
    }

    #[test]
    fn test_compute_object_key() {
        let file_key = vec![0x01, 0x02, 0x03, 0x04, 0x05];
        let key = compute_object_key(&file_key, 10, 0, false);
        assert_eq!(key.len(), 10); // min(5+5, 16) = 10
    }

    #[test]
    fn test_compute_object_key_aes() {
        let file_key = vec![0x01u8; 16];
        let key = compute_object_key(&file_key, 1, 0, true);
        // With "sAlT" suffix, key derivation includes extra bytes
        assert_eq!(key.len(), 16); // min(16+5, 16) = 16
    }

    #[test]
    fn test_r234_key_generation_roundtrip() {
        let user_pw = b"user123";
        let owner_pw = b"owner456";

        let ed = EncryptionDict {
            filter: b"Standard".to_vec(),
            v: 2,
            length: 128,
            r: 3,
            o: vec![0u8; 32], // placeholder
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

        let file_id = b"test-file-id-123";

        let (o, u, file_key) = generate_o_u_values_r234(user_pw, owner_pw, &ed, file_id);
        assert_eq!(o.len(), 32);
        assert_eq!(u.len(), 32);
        assert!(!file_key.is_empty());

        // Verify: using user password should derive the same file key
        let mut verify_ed = ed.clone();
        verify_ed.o = o;
        verify_ed.u = u;
        let verify_key = compute_file_encryption_key_r234(user_pw, &verify_ed, file_id);
        assert_eq!(verify_key, file_key);
    }

    #[test]
    fn test_r6_hash_deterministic() {
        let hash1 = compute_hash_r6(b"password", b"12345678", &[]);
        let hash2 = compute_hash_r6(b"password", b"12345678", &[]);
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_r6_hash_different_passwords() {
        let hash1 = compute_hash_r6(b"pass1", b"12345678", &[]);
        let hash2 = compute_hash_r6(b"pass2", b"12345678", &[]);
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_r6_generation_roundtrip() {
        let user_pw = b"user";
        let owner_pw = b"owner";
        let file_key = [0x42u8; 32];
        let uvs = [1u8; 8];
        let uks = [2u8; 8];
        let ovs = [3u8; 8];
        let oks = [4u8; 8];

        let (o, u, oe, ue, perms) = generate_values_r6(
            user_pw, owner_pw, -4, true, &file_key, &uvs, &uks, &ovs, &oks,
        );

        assert_eq!(u.len(), 48);
        assert_eq!(o.len(), 48);
        assert_eq!(ue.len(), 32);
        assert_eq!(oe.len(), 32);
        assert_eq!(perms.len(), 16);

        // Verify user password can recover file key
        let recovered = compute_file_key_r6_user(user_pw, &u, &ue);
        assert_eq!(recovered.unwrap(), file_key.to_vec());

        // Verify owner password can recover file key
        let recovered_o = compute_file_key_r6_owner(owner_pw, &o, &oe, &u);
        assert_eq!(recovered_o.unwrap(), file_key.to_vec());
    }
}
