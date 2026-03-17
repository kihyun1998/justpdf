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

/// TIFF Predictor 2: horizontal differencing.
fn apply_tiff_predictor(data: Vec<u8>, params: &PdfDict) -> Result<Vec<u8>> {
    let columns = params.get_i64(b"Columns").unwrap_or(1) as usize;
    let colors = params.get_i64(b"Colors").unwrap_or(1) as usize;
    let bits_per_component = params.get_i64(b"BitsPerComponent").unwrap_or(8) as usize;

    if bits_per_component != 8 {
        return Err(JustPdfError::StreamDecode {
            filter: "Predictor".into(),
            detail: format!("TIFF predictor only supports 8 BPC, got {bits_per_component}"),
        });
    }

    let bytes_per_pixel = colors;
    let row_bytes = columns * bytes_per_pixel;
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
    let columns = params.get_i64(b"Columns").unwrap_or(1) as usize;
    let colors = params.get_i64(b"Colors").unwrap_or(1) as usize;
    let bits_per_component = params.get_i64(b"BitsPerComponent").unwrap_or(8) as usize;

    let bytes_per_pixel = (colors * bits_per_component).div_ceil(8);
    let row_bytes = (columns * colors * bits_per_component).div_ceil(8);
    // Each row has 1 filter byte + row_bytes of data
    let stride = row_bytes + 1;

    let mut result = Vec::with_capacity(data.len());
    let mut prev_row: Vec<u8> = vec![0; row_bytes];

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

    #[test]
    fn test_unsupported_predictor() {
        let mut params = PdfDict::new();
        params.insert(b"Predictor".to_vec(), crate::object::PdfObject::Integer(99));
        let result = apply(vec![1, 2, 3], &params);
        assert!(result.is_err());
    }
}
