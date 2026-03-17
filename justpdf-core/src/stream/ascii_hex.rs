use crate::error::{JustPdfError, Result};

/// Decode ASCIIHexDecode data.
/// Input is hex digits with optional whitespace, terminated by `>`.
pub fn decode(data: &[u8]) -> Result<Vec<u8>> {
    let mut result = Vec::new();
    let mut hex_chars = Vec::new();

    for &b in data {
        match b {
            b'>' => break, // End of data marker
            _ if b.is_ascii_hexdigit() => hex_chars.push(b),
            _ if b.is_ascii_whitespace() => continue,
            _ => {
                return Err(JustPdfError::StreamDecode {
                    filter: "ASCIIHexDecode".into(),
                    detail: format!("invalid byte 0x{b:02X}"),
                });
            }
        }
    }

    // Pad with trailing 0 if odd
    if hex_chars.len() % 2 != 0 {
        hex_chars.push(b'0');
    }

    for pair in hex_chars.chunks(2) {
        let hi = hex_val(pair[0]);
        let lo = hex_val(pair[1]);
        result.push((hi << 4) | lo);
    }

    Ok(result)
}

#[inline]
fn hex_val(b: u8) -> u8 {
    match b {
        b'0'..=b'9' => b - b'0',
        b'a'..=b'f' => b - b'a' + 10,
        b'A'..=b'F' => b - b'A' + 10,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic() {
        assert_eq!(decode(b"48656C6C6F>").unwrap(), b"Hello");
    }

    #[test]
    fn test_with_whitespace() {
        assert_eq!(decode(b"48 65 6C 6C 6F>").unwrap(), b"Hello");
    }

    #[test]
    fn test_odd_digits() {
        // "ABC>" → AB C0
        assert_eq!(decode(b"ABC>").unwrap(), vec![0xAB, 0xC0]);
    }

    #[test]
    fn test_empty() {
        assert_eq!(decode(b">").unwrap(), Vec::<u8>::new());
    }

    #[test]
    fn test_no_eod_marker() {
        // Should still work, treating entire input as hex
        assert_eq!(decode(b"4142").unwrap(), b"AB");
    }

    #[test]
    fn test_lowercase() {
        assert_eq!(decode(b"4a4b>").unwrap(), b"JK");
    }

    #[test]
    fn test_invalid_char() {
        assert!(decode(b"48GG>").is_err());
    }
}
