use crate::error::{JustPdfError, Result};
use crate::object::PdfDict;

/// Apply predictor defiltering to decoded data.
pub fn apply(data: Vec<u8>, params: &PdfDict) -> Result<Vec<u8>> {
    let predictor = params.get_i64(b"Predictor").unwrap_or(1) as u32;

    match predictor {
        1 => Ok(data), // No prediction
        2 => apply_tiff_predictor(data, params),
        10..=15 => apply_png_predictor(data, params),
        _ => Err(JustPdfError::StreamDecode {
            filter: "Predictor".into(),
            detail: format!("unsupported predictor value: {predictor}"),
        }),
    }
}

/// The row layout `/DecodeParms` describes, checked: `/Columns` and
/// `/Colors` below 1 read as 1 and `/BitsPerComponent` below 1 as 8; bits per
/// component must be 1, 2, 4, 8 or 16, colours at most 32, and a row's bits
/// below `i32::MAX`.
struct Layout {
    bits_per_component: usize,
    /// Bytes from one pixel to the next, at least 1.
    bytes_per_pixel: usize,
    /// Bytes of one row, without a PNG filter byte.
    row_bytes: usize,
}

impl Layout {
    fn read(params: &PdfDict) -> Result<Self> {
        let at_least_one = |key: &[u8], default: i64| match params.get_i64(key) {
            Some(v) if v >= 1 => v,
            _ => default,
        };
        let columns = at_least_one(b"Columns", 1);
        let colors = at_least_one(b"Colors", 1);
        let bits = at_least_one(b"BitsPerComponent", 8);
        let invalid = |detail: String| JustPdfError::StreamDecode {
            filter: "Predictor".into(),
            detail,
        };
        if ![1, 2, 4, 8, 16].contains(&bits) {
            return Err(invalid(format!("invalid /BitsPerComponent: {bits}")));
        }
        if colors > 32 {
            return Err(invalid(format!("too many /Colors: {colors}")));
        }
        if columns >= i64::from(i32::MAX) / (bits * colors) {
            return Err(invalid(format!("too many /Columns: {columns}")));
        }
        Ok(Self {
            bits_per_component: bits as usize,
            bytes_per_pixel: ((colors * bits) as usize).div_ceil(8),
            row_bytes: ((columns * colors * bits) as usize).div_ceil(8),
        })
    }
}

/// TIFF Predictor 2: horizontal differencing.
fn apply_tiff_predictor(data: Vec<u8>, params: &PdfDict) -> Result<Vec<u8>> {
    let Layout {
        bits_per_component,
        bytes_per_pixel,
        row_bytes,
    } = Layout::read(params)?;

    if bits_per_component != 8 {
        return Err(JustPdfError::StreamDecode {
            filter: "Predictor".into(),
            detail: format!("TIFF predictor only supports 8 BPC, got {bits_per_component}"),
        });
    }

    let mut result = data.clone();

    for row_start in (0..result.len()).step_by(row_bytes) {
        let row_end = (row_start + row_bytes).min(result.len());
        for i in (row_start + bytes_per_pixel)..row_end {
            result[i] = result[i].wrapping_add(result[i - bytes_per_pixel]);
        }
    }

    Ok(result)
}

/// PNG predictor (10-15): each row starts with a filter byte.
fn apply_png_predictor(data: Vec<u8>, params: &PdfDict) -> Result<Vec<u8>> {
    let Layout {
        bytes_per_pixel,
        row_bytes,
        ..
    } = Layout::read(params)?;
    // Each row has 1 filter byte + row_bytes of data
    let stride = row_bytes + 1;

    let mut result = Vec::with_capacity(data.len());
    // Sized only when the data holds a whole row.
    let mut prev_row: Vec<u8> = if data.len() >= stride {
        vec![0; row_bytes]
    } else {
        Vec::new()
    };

    let mut offset = 0;
    while offset + stride <= data.len() {
        let filter_byte = data[offset];
        let row_data = &data[offset + 1..offset + stride];

        let mut decoded_row = vec![0u8; row_bytes];

        match filter_byte {
            0 => {
                // None
                decoded_row.copy_from_slice(row_data);
            }
            1 => {
                // Sub
                for i in 0..row_bytes {
                    let left = if i >= bytes_per_pixel {
                        decoded_row[i - bytes_per_pixel]
                    } else {
                        0
                    };
                    decoded_row[i] = row_data[i].wrapping_add(left);
                }
            }
            2 => {
                // Up
                for i in 0..row_bytes {
                    decoded_row[i] = row_data[i].wrapping_add(prev_row[i]);
                }
            }
            3 => {
                // Average
                for i in 0..row_bytes {
                    let left = if i >= bytes_per_pixel {
                        decoded_row[i - bytes_per_pixel] as u16
                    } else {
                        0
                    };
                    let up = prev_row[i] as u16;
                    let avg = ((left + up) / 2) as u8;
                    decoded_row[i] = row_data[i].wrapping_add(avg);
                }
            }
            4 => {
                // Paeth
                for i in 0..row_bytes {
                    let left = if i >= bytes_per_pixel {
                        decoded_row[i - bytes_per_pixel]
                    } else {
                        0
                    };
                    let up = prev_row[i];
                    let up_left = if i >= bytes_per_pixel {
                        prev_row[i - bytes_per_pixel]
                    } else {
                        0
                    };
                    let paeth = paeth_predictor(left, up, up_left);
                    decoded_row[i] = row_data[i].wrapping_add(paeth);
                }
            }
            _ => {
                return Err(JustPdfError::StreamDecode {
                    filter: "Predictor".into(),
                    detail: format!("unknown PNG filter type: {filter_byte}"),
                });
            }
        }

        result.extend_from_slice(&decoded_row);
        prev_row = decoded_row;
        offset += stride;
    }

    // Handle any remaining bytes (incomplete last row)
    if offset < data.len() {
        result.extend_from_slice(&data[offset..]);
    }

    Ok(result)
}

fn paeth_predictor(a: u8, b: u8, c: u8) -> u8 {
    let a = a as i32;
    let b = b as i32;
    let c = c as i32;
    let p = a + b - c;
    let pa = (p - a).abs();
    let pb = (p - b).abs();
    let pc = (p - c).abs();
    if pa <= pb && pa <= pc {
        a as u8
    } else if pb <= pc {
        b as u8
    } else {
        c as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_predictor() {
        let dict = PdfDict::new();
        let data = vec![1, 2, 3, 4];
        let result = apply(data.clone(), &dict).unwrap();
        assert_eq!(result, data);
    }

    #[test]
    fn test_png_none_filter() {
        let mut params = PdfDict::new();
        params.insert(b"Predictor".to_vec(), crate::object::PdfObject::Integer(10));
        params.insert(b"Columns".to_vec(), crate::object::PdfObject::Integer(3));
        params.insert(b"Colors".to_vec(), crate::object::PdfObject::Integer(1));

        // Filter byte 0 (None) + 3 bytes of data
        let data = vec![0, 10, 20, 30];
        let result = apply(data, &params).unwrap();
        assert_eq!(result, vec![10, 20, 30]);
    }

    #[test]
    fn test_png_sub_filter() {
        let mut params = PdfDict::new();
        params.insert(b"Predictor".to_vec(), crate::object::PdfObject::Integer(11));
        params.insert(b"Columns".to_vec(), crate::object::PdfObject::Integer(3));
        params.insert(b"Colors".to_vec(), crate::object::PdfObject::Integer(1));

        // Filter byte 1 (Sub), bytes: 10, 5, 3
        // Result: 10, 10+5=15, 15+3=18
        let data = vec![1, 10, 5, 3];
        let result = apply(data, &params).unwrap();
        assert_eq!(result, vec![10, 15, 18]);
    }

    #[test]
    fn test_png_up_filter() {
        let mut params = PdfDict::new();
        params.insert(b"Predictor".to_vec(), crate::object::PdfObject::Integer(12));
        params.insert(b"Columns".to_vec(), crate::object::PdfObject::Integer(3));
        params.insert(b"Colors".to_vec(), crate::object::PdfObject::Integer(1));

        // Row 0: filter=0 (None), data=10,20,30
        // Row 1: filter=2 (Up), data=1,2,3 → 10+1=11, 20+2=22, 30+3=33
        let data = vec![0, 10, 20, 30, 2, 1, 2, 3];
        let result = apply(data, &params).unwrap();
        assert_eq!(result, vec![10, 20, 30, 11, 22, 33]);
    }

    fn params(entries: &[(&[u8], i64)]) -> PdfDict {
        let mut d = PdfDict::new();
        for (key, value) in entries {
            d.insert(key.to_vec(), crate::object::PdfObject::Integer(*value));
        }
        d
    }

    #[test]
    fn parameters_below_one_take_their_defaults() {
        for key in [&b"Columns"[..], b"Colors", b"BitsPerComponent"] {
            let p = params(&[(b"Predictor", 10), (key, -1)]);
            assert_eq!(
                apply(vec![0, 10, 0, 20], &p).unwrap(),
                vec![10, 20],
                "{key:?}"
            );
        }
        let p = params(&[(b"Predictor", 10), (b"Columns", 0), (b"Colors", 0)]);
        assert_eq!(apply(vec![0, 10], &p).unwrap(), vec![10]);
    }

    #[test]
    fn a_row_longer_than_the_data_is_no_error_and_adds_no_bytes() {
        let p = params(&[(b"Predictor", 12), (b"Columns", 1000)]);
        assert!(apply(vec![0, 1, 2, 3], &p).unwrap().len() <= 4);
    }

    #[test]
    fn invalid_predictor_parameters_are_errors() {
        for p in [
            params(&[(b"Predictor", 10), (b"BitsPerComponent", 3)]),
            params(&[(b"Predictor", 10), (b"Colors", 33)]),
            params(&[(b"Predictor", 10), (b"Columns", i64::MAX), (b"Colors", 32)]),
            params(&[(b"Predictor", 12), (b"Columns", 1_000_000_000_000)]),
            params(&[(b"Predictor", 2), (b"Columns", 1 << 28)]),
            params(&[(b"Predictor", 2), (b"Colors", 33)]),
        ] {
            let err = apply(vec![0, 1, 2, 3], &p).unwrap_err();
            assert!(
                matches!(err, JustPdfError::StreamDecode { .. }),
                "{p:?}: {err:?}"
            );
        }
    }

    #[test]
    fn the_tiff_predictor_adds_each_byte_to_the_one_a_pixel_before() {
        let p = params(&[(b"Predictor", 2), (b"Columns", 3)]);
        assert_eq!(
            apply(vec![10, 5, 3, 1, 1, 1], &p).unwrap(),
            vec![10, 15, 18, 1, 2, 3]
        );
        let p = params(&[(b"Predictor", 2), (b"Columns", 0)]);
        assert_eq!(apply(vec![1, 2, 3], &p).unwrap(), vec![1, 2, 3]);
    }

    #[test]
    fn test_unsupported_predictor() {
        let mut params = PdfDict::new();
        params.insert(b"Predictor".to_vec(), crate::object::PdfObject::Integer(99));
        let result = apply(vec![1, 2, 3], &params);
        assert!(result.is_err());
    }
}
