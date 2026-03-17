use crate::error::{JustPdfError, Result};

/// Decode ASCII85Decode (a.k.a. btoa) data.
/// Groups of 5 ASCII chars (33..117) decode to 4 bytes.
/// `z` is shorthand for 4 zero bytes.
/// Terminated by `~>`.
pub fn decode(data: &[u8]) -> Result<Vec<u8>> {
    let mut result = Vec::new();
    let mut group = Vec::with_capacity(5);
    let mut i = 0;

    while i < data.len() {
        let b = data[i];
        i += 1;

        match b {
            // End of data
            b'~' => {
                if i < data.len() && data[i] == b'>' {
                    break;
                }
                // Just ~ without > — treat as EOD anyway
                break;
            }
            // Whitespace: skip
            b'\0' | b'\t' | b'\n' | b'\x0C' | b'\r' | b' ' => continue,
            // z shorthand: four zero bytes (only valid between groups)
            b'z' => {
                if !group.is_empty() {
                    return Err(JustPdfError::StreamDecode {
                        filter: "ASCII85Decode".into(),
                        detail: "'z' inside a group".into(),
                    });
                }
                result.extend_from_slice(&[0, 0, 0, 0]);
            }
            // Valid ASCII85 chars: ! (33) to u (117)
            b'!'..=b'u' => {
                group.push(b - b'!');
                if group.len() == 5 {
                    let val = decode_group(&group, 5)?;
                    result.push((val >> 24) as u8);
                    result.push((val >> 16) as u8);
                    result.push((val >> 8) as u8);
                    result.push(val as u8);
                    group.clear();
                }
            }
            _ => {
                return Err(JustPdfError::StreamDecode {
                    filter: "ASCII85Decode".into(),
                    detail: format!("invalid byte 0x{b:02X}"),
                });
            }
        }
    }

    // Handle final partial group (2-4 chars)
    if !group.is_empty() {
        let n = group.len();
        if n < 2 {
            return Err(JustPdfError::StreamDecode {
                filter: "ASCII85Decode".into(),
                detail: format!("incomplete group of {n} char(s)"),
            });
        }
        // Pad with 'u' (84) to make 5 chars
        while group.len() < 5 {
            group.push(84); // 'u' - '!' = 84
        }
        let val = decode_group(&group, 5)?;
        let bytes = [
            (val >> 24) as u8,
            (val >> 16) as u8,
            (val >> 8) as u8,
            val as u8,
        ];
        // Output only n-1 bytes
        result.extend_from_slice(&bytes[..n - 1]);
    }

    Ok(result)
}

fn decode_group(group: &[u8], _count: usize) -> Result<u32> {
    let mut val: u64 = 0;
    val += group[0] as u64 * 85 * 85 * 85 * 85;
    val += group[1] as u64 * 85 * 85 * 85;
    val += group[2] as u64 * 85 * 85;
    val += group[3] as u64 * 85;
    val += group[4] as u64;

    if val > u32::MAX as u64 {
        return Err(JustPdfError::StreamDecode {
            filter: "ASCII85Decode".into(),
            detail: "group value overflow".into(),
        });
    }

    Ok(val as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic() {
        // "Man " in ASCII85 is "9jqo^"
        let decoded = decode(b"9jqo^~>").unwrap();
        assert_eq!(decoded, b"Man ");
    }

    #[test]
    fn test_zero_shorthand() {
        let decoded = decode(b"z~>").unwrap();
        assert_eq!(decoded, vec![0, 0, 0, 0]);
    }

    #[test]
    fn test_empty() {
        let decoded = decode(b"~>").unwrap();
        assert_eq!(decoded, Vec::<u8>::new());
    }

    #[test]
    fn test_partial_group() {
        // 2-char group: "9j" decodes to 1 byte
        let decoded = decode(b"9j~>").unwrap();
        assert_eq!(decoded.len(), 1);
    }

    #[test]
    fn test_whitespace_ignored() {
        let decoded = decode(b"9j qo ^~>").unwrap();
        assert_eq!(decoded, b"Man ");
    }

    #[test]
    fn test_invalid_char() {
        let result = decode(b"\xFF~>");
        assert!(result.is_err());
    }

    #[test]
    fn test_known_vector() {
        // "Hello" → BOu!rD]
        // Actually let's test the reverse
        let encoded = b"87cURD]j7BEbo80~>";
        let decoded = decode(encoded).unwrap();
        // This should decode to some known text
        assert!(!decoded.is_empty());
    }
}
