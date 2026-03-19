//! RC4 stream cipher implementation for PDF encryption.
//!
//! RC4 is used in PDF encryption V=1/2 (R=2/3/4).
//! It is symmetric: encrypt and decrypt are the same operation.

/// RC4 state.
struct Rc4State {
    s: [u8; 256],
    i: u8,
    j: u8,
}

impl Rc4State {
    fn new(key: &[u8]) -> Self {
        let mut s = [0u8; 256];
        for i in 0..256 {
            s[i] = i as u8;
        }
        let mut j: u8 = 0;
        for i in 0..256 {
            j = j
                .wrapping_add(s[i])
                .wrapping_add(key[i % key.len()]);
            s.swap(i, j as usize);
        }
        Self { s, i: 0, j: 0 }
    }

    fn process(&mut self, data: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(data.len());
        for &byte in data {
            self.i = self.i.wrapping_add(1);
            self.j = self.j.wrapping_add(self.s[self.i as usize]);
            self.s.swap(self.i as usize, self.j as usize);
            let k = self.s[(self.s[self.i as usize].wrapping_add(self.s[self.j as usize])) as usize];
            out.push(byte ^ k);
        }
        out
    }
}

/// Encrypt/decrypt data using RC4. RC4 is symmetric.
pub fn rc4(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut state = Rc4State::new(key);
    state.process(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rc4_roundtrip() {
        let key = b"Secret";
        let plaintext = b"Hello, PDF encryption!";
        let ciphertext = rc4(key, plaintext);
        assert_ne!(&ciphertext, plaintext);
        let decrypted = rc4(key, &ciphertext);
        assert_eq!(&decrypted, plaintext);
    }

    #[test]
    fn test_rc4_empty() {
        let result = rc4(b"key", b"");
        assert!(result.is_empty());
    }

    // RFC 6229 test vector: Key = 0102030405
    #[test]
    fn test_rc4_known_vector() {
        let key = [0x01, 0x02, 0x03, 0x04, 0x05];
        let plaintext = [0u8; 16];
        let ct = rc4(&key, &plaintext);
        // Known first bytes of RC4(01020304050) keystream
        assert_eq!(ct[0], 0xb2);
        assert_eq!(ct[1], 0x39);
        assert_eq!(ct[2], 0x63);
        assert_eq!(ct[3], 0x05);
    }
}
