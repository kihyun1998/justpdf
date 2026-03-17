use crate::error::{JustPdfError, Result};

/// Decode RunLengthDecode data.
/// Format: length byte followed by data.
/// - 0..=127: copy next (length+1) bytes literally
/// - 129..=255: repeat next byte (257-length) times
/// - 128: end of data
pub fn decode(data: &[u8]) -> Result<Vec<u8>> {
    let mut result = Vec::new();
    let mut i = 0;

    while i < data.len() {
        let length = data[i];
        i += 1;

        match length {
            0..=127 => {
                // Copy next (length+1) bytes
                let count = length as usize + 1;
                if i + count > data.len() {
                    return Err(JustPdfError::StreamDecode {
                        filter: "RunLengthDecode".into(),
                        detail: "unexpected end of data in literal run".into(),
                    });
                }
                result.extend_from_slice(&data[i..i + count]);
                i += count;
            }
            128 => {
                // EOD
                break;
            }
            129..=255 => {
                // Repeat next byte (257-length) times
                let count = 257 - length as usize;
                if i >= data.len() {
                    return Err(JustPdfError::StreamDecode {
                        filter: "RunLengthDecode".into(),
                        detail: "unexpected end of data in repeat run".into(),
                    });
                }
                let byte = data[i];
                i += 1;
                result.extend(std::iter::repeat_n(byte, count));
            }
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_literal_run() {
        // 2 = copy next 3 bytes, then EOD
        let data = [2, b'A', b'B', b'C', 128];
        let result = decode(&data).unwrap();
        assert_eq!(result, b"ABC");
    }

    #[test]
    fn test_repeat_run() {
        // 253 = repeat next byte (257-253)=4 times, then EOD
        let data = [253, b'X', 128];
        let result = decode(&data).unwrap();
        assert_eq!(result, b"XXXX");
    }

    #[test]
    fn test_mixed() {
        // Literal "AB" (1, A, B) + Repeat 'C' 3 times (254, C) + EOD
        let data = [1, b'A', b'B', 254, b'C', 128];
        let result = decode(&data).unwrap();
        assert_eq!(result, b"ABCCC");
    }

    #[test]
    fn test_empty() {
        let data = [128]; // Just EOD
        let result = decode(&data).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_no_eod() {
        // Data without EOD marker — should still work
        let data = [0, b'A'];
        let result = decode(&data).unwrap();
        assert_eq!(result, b"A");
    }
}
