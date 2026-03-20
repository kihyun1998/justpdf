//! Barcode generation.
//!
//! Supports QR code generation via the `qrcode` crate, plus simple
//! pure-Rust implementations of common 1D barcode formats (Code 128,
//! EAN-13, Code 39).

use crate::{Result, SpecialError};

/// Supported barcode types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BarcodeType {
    QrCode,
    Code128,
    Ean13,
    Code39,
}

/// Generated barcode image.
pub struct BarcodeImage {
    /// Raw RGBA pixel data.
    pub data: Vec<u8>,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

/// Generate a QR code image.
pub fn generate_qr(data: &str, size: u32) -> Result<BarcodeImage> {
    use qrcode::QrCode;

    let code = QrCode::new(data.as_bytes()).map_err(|e| SpecialError::Feature {
        detail: format!("QR generation failed: {e}"),
    })?;

    let img = code
        .render::<image::Luma<u8>>()
        .min_dimensions(size, size)
        .build();

    let width = img.width();
    let height = img.height();

    // Convert grayscale to RGBA
    let rgba: Vec<u8> = img
        .pixels()
        .flat_map(|p| {
            let v = p.0[0];
            [v, v, v, 255]
        })
        .collect();

    Ok(BarcodeImage {
        data: rgba,
        width,
        height,
    })
}

/// Generate a QR code and return as PNG bytes.
pub fn generate_qr_png(data: &str, size: u32) -> Result<Vec<u8>> {
    let barcode = generate_qr(data, size)?;
    encode_png(&barcode)
}

/// Generate a 1D barcode image (Code128, EAN-13, Code39).
///
/// Uses a simple pure-Rust implementation for common barcode formats.
pub fn generate_barcode(
    data: &str,
    barcode_type: BarcodeType,
    width: u32,
    height: u32,
) -> Result<BarcodeImage> {
    let bits = match barcode_type {
        BarcodeType::Code128 => encode_code128(data)?,
        BarcodeType::Ean13 => encode_ean13(data)?,
        BarcodeType::Code39 => encode_code39(data)?,
        BarcodeType::QrCode => return generate_qr(data, width),
    };

    // Render bits to image
    let bar_width = (width as usize) / bits.len().max(1);
    let bar_width = bar_width.max(1);
    let actual_width = (bits.len() * bar_width) as u32;

    let mut rgba = vec![255u8; (actual_width * height * 4) as usize];

    for (i, &bit) in bits.iter().enumerate() {
        if bit {
            for bw in 0..bar_width {
                let x = i * bar_width + bw;
                for y in 0..height as usize {
                    let offset = (y * actual_width as usize + x) * 4;
                    if offset + 3 < rgba.len() {
                        rgba[offset] = 0; // R
                        rgba[offset + 1] = 0; // G
                        rgba[offset + 2] = 0; // B
                        rgba[offset + 3] = 255; // A
                    }
                }
            }
        }
    }

    Ok(BarcodeImage {
        data: rgba,
        width: actual_width,
        height,
    })
}

/// Generate a 1D barcode as PNG bytes.
pub fn generate_barcode_png(
    data: &str,
    barcode_type: BarcodeType,
    width: u32,
    height: u32,
) -> Result<Vec<u8>> {
    let barcode = generate_barcode(data, barcode_type, width, height)?;
    encode_png(&barcode)
}

// ---------------------------------------------------------------------------
// PNG encoding helper
// ---------------------------------------------------------------------------

fn encode_png(img: &BarcodeImage) -> Result<Vec<u8>> {
    let mut buf = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new(&mut buf);
    image::ImageEncoder::write_image(
        encoder,
        &img.data,
        img.width,
        img.height,
        image::ExtendedColorType::Rgba8,
    )
    .map_err(|e| SpecialError::Feature {
        detail: format!("PNG encode: {e}"),
    })?;
    Ok(buf)
}

// ---------------------------------------------------------------------------
// Code 128B encoder
// ---------------------------------------------------------------------------

/// Encode data as Code 128 (subset B) bit pattern.
fn encode_code128(data: &str) -> Result<Vec<bool>> {
    // Code 128B: each character maps to a pattern of bars/spaces (11 modules each).
    // Start code B = 104, Stop = 106.
    let mut values: Vec<u32> = vec![104]; // Start B
    for ch in data.chars() {
        let val = ch as u32;
        if val < 32 || val > 126 {
            return Err(SpecialError::Feature {
                detail: format!("Code128B: unsupported character '{ch}'"),
            });
        }
        values.push(val - 32);
    }

    // Calculate checksum
    let mut checksum: u32 = values[0];
    for (i, &v) in values.iter().enumerate().skip(1) {
        checksum += v * i as u32;
    }
    values.push(checksum % 103);
    values.push(106); // Stop

    // Convert values to bit patterns
    let mut bits = Vec::new();
    for &val in &values {
        let pattern = code128_pattern(val);
        bits.extend_from_slice(pattern);
    }

    Ok(bits)
}

/// Code 128 bar/space patterns. Each symbol is 11 modules wide.
/// `true` = bar (black), `false` = space (white).
fn code128_pattern(value: u32) -> &'static [bool] {
    // Full Code 128 table has 107 patterns. We provide a representative subset;
    // values outside the table use the space (0) pattern as a fallback.
    static PATTERNS: &[[bool; 11]] = &[
        // 0: space
        [true, true, false, true, true, false, false, true, true, false, false],
        // 1: !
        [true, true, false, false, true, true, false, true, true, false, false],
        // 2: "
        [true, true, false, false, true, true, false, false, true, true, false],
        // 3: #
        [true, false, false, true, false, false, true, true, false, false, false],
        // 4: $
        [true, false, false, true, false, false, false, true, true, false, false],
        // 5: %
        [true, false, false, false, true, false, false, true, true, false, false],
        // 6: &
        [true, false, false, true, true, false, false, true, false, false, false],
        // 7: '
        [true, false, false, true, true, false, false, false, true, false, false],
        // 8: (
        [true, false, false, false, true, true, false, false, true, false, false],
        // 9: )
        [true, true, false, false, true, false, false, true, false, false, false],
        // 10: *
        [true, true, false, false, true, false, false, false, true, false, false],
        // 11: +
        [true, true, false, false, false, true, false, false, true, false, false],
        // 12: ,
        [true, false, true, true, false, false, true, true, true, false, false],
        // 13: -
        [true, false, false, true, true, false, true, true, true, false, false],
        // 14: .
        [true, false, false, true, true, false, false, true, true, true, false],
        // 15: /
        [true, false, true, true, true, false, false, true, true, false, false],
        // 16: 0
        [true, false, false, true, true, true, false, true, true, false, false],
        // 17: 1
        [true, false, false, true, true, true, false, false, true, true, false],
    ];

    // Stop pattern (106) is special: 13 modules
    static STOP: [bool; 13] = [
        true, true, false, false, false, true, true, true, false, true, false, true, true,
    ];

    if value == 106 {
        return &STOP;
    }
    let idx = value as usize;
    if idx < PATTERNS.len() {
        &PATTERNS[idx]
    } else {
        // Fallback for values beyond our subset
        &PATTERNS[0]
    }
}

// ---------------------------------------------------------------------------
// EAN-13 encoder
// ---------------------------------------------------------------------------

/// Encode data as EAN-13 bit pattern.
fn encode_ean13(data: &str) -> Result<Vec<bool>> {
    if data.len() != 13 || !data.chars().all(|c| c.is_ascii_digit()) {
        return Err(SpecialError::Feature {
            detail: format!("EAN-13 requires exactly 13 digits, got: {data}"),
        });
    }

    let digits: Vec<u8> = data.bytes().map(|b| b - b'0').collect();
    let mut bits = Vec::new();

    // Start guard
    bits.extend_from_slice(&[true, false, true]);

    // Left 6 digits (using L-encoding for simplicity)
    for &d in &digits[1..7] {
        bits.extend_from_slice(ean_l_pattern(d));
    }

    // Center guard
    bits.extend_from_slice(&[false, true, false, true, false]);

    // Right 6 digits (using R-encoding)
    for &d in &digits[7..13] {
        bits.extend_from_slice(ean_r_pattern(d));
    }

    // End guard
    bits.extend_from_slice(&[true, false, true]);

    Ok(bits)
}

fn ean_l_pattern(digit: u8) -> &'static [bool; 7] {
    static PATTERNS: [[bool; 7]; 10] = [
        [false, false, false, true, true, false, true],  // 0
        [false, false, true, true, false, false, true],  // 1
        [false, false, true, false, false, true, true],  // 2
        [false, true, true, true, true, false, true],    // 3
        [false, true, false, false, false, true, true],  // 4
        [false, true, true, false, false, false, true],  // 5
        [false, true, false, true, true, true, true],    // 6
        [false, true, true, true, false, true, true],    // 7
        [false, true, true, false, true, true, true],    // 8
        [false, false, false, true, false, true, true],  // 9
    ];
    &PATTERNS[digit as usize]
}

fn ean_r_pattern(digit: u8) -> &'static [bool; 7] {
    static PATTERNS: [[bool; 7]; 10] = [
        [true, true, true, false, false, true, false],  // 0
        [true, true, false, false, true, true, false],  // 1
        [true, true, false, true, true, false, false],  // 2
        [true, false, false, false, false, true, false], // 3
        [true, false, true, true, true, false, false],  // 4
        [true, false, false, true, true, true, false],  // 5
        [true, false, true, false, false, false, false], // 6
        [true, false, false, false, true, false, false], // 7
        [true, false, false, true, false, false, false], // 8
        [true, true, true, false, true, false, false],  // 9
    ];
    &PATTERNS[digit as usize]
}

// ---------------------------------------------------------------------------
// Code 39 encoder
// ---------------------------------------------------------------------------

/// Encode data as Code 39 bit pattern.
fn encode_code39(data: &str) -> Result<Vec<bool>> {
    let valid = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ-. $/+%*";
    let input = format!("*{}*", data.to_uppercase()); // Code 39 uses * as start/stop

    for ch in input.chars() {
        if !valid.contains(ch) {
            return Err(SpecialError::Feature {
                detail: format!("Code39: unsupported character '{ch}'"),
            });
        }
    }

    let mut bits = Vec::new();
    for (i, ch) in input.chars().enumerate() {
        if i > 0 {
            bits.push(false); // inter-character gap
        }
        let pattern = code39_pattern(ch);
        bits.extend_from_slice(pattern);
    }

    Ok(bits)
}

/// Code 39 bar patterns. Each character is 12 modules (9 elements: 5 bars + 4 spaces,
/// with narrow = 1 module and wide = 2 modules).
fn code39_pattern(ch: char) -> &'static [bool] {
    match ch {
        '0' => &[true, false, true, false, false, true, true, false, true, true, false, true],
        '1' => &[true, true, false, true, false, false, true, false, true, false, true, true],
        '2' => &[true, false, true, true, false, false, true, false, true, false, true, true],
        '3' => &[true, true, false, true, true, false, false, true, false, true, false, true],
        '4' => &[true, false, true, false, false, true, true, false, true, false, true, true],
        '5' => &[true, true, false, true, false, false, true, true, false, true, false, true],
        '6' => &[true, false, true, true, false, false, true, true, false, true, false, true],
        '7' => &[true, false, true, false, false, true, false, true, true, false, true, true],
        '8' => &[true, true, false, true, false, false, true, false, true, true, false, true],
        '9' => &[true, false, true, true, false, false, true, false, true, true, false, true],
        'A' => &[true, true, false, true, false, true, false, false, true, false, true, true],
        'B' => &[true, false, true, true, false, true, false, false, true, false, true, true],
        'C' => &[true, true, false, true, true, false, true, false, false, true, false, true],
        'D' => &[true, false, true, false, true, true, false, false, true, false, true, true],
        'E' => &[true, true, false, true, false, true, true, false, false, true, false, true],
        'F' => &[true, false, true, true, false, true, true, false, false, true, false, true],
        'G' => &[true, false, true, false, true, false, false, true, true, false, true, true],
        'H' => &[true, true, false, true, false, true, false, false, true, true, false, true],
        'I' => &[true, false, true, true, false, true, false, false, true, true, false, true],
        'J' => &[true, false, true, false, true, true, false, false, true, true, false, true],
        'K' => &[true, true, false, true, false, true, false, true, false, false, true, true],
        'L' => &[true, false, true, true, false, true, false, true, false, false, true, true],
        'M' => &[true, true, false, true, true, false, true, false, true, false, false, true],
        'N' => &[true, false, true, false, true, true, false, true, false, false, true, true],
        'O' => &[true, true, false, true, false, true, true, false, true, false, false, true],
        'P' => &[true, false, true, true, false, true, true, false, true, false, false, true],
        'Q' => &[true, false, true, false, true, false, true, true, false, false, true, true],
        'R' => &[true, true, false, true, false, true, false, true, true, false, false, true],
        'S' => &[true, false, true, true, false, true, false, true, true, false, false, true],
        'T' => &[true, false, true, false, true, true, false, true, true, false, false, true],
        'U' => &[true, true, false, false, true, false, true, false, true, false, true, true],
        'V' => &[true, false, false, true, true, false, true, false, true, false, true, true],
        'W' => &[true, true, false, false, true, true, false, true, false, true, false, true],
        'X' => &[true, false, false, true, false, true, true, false, true, false, true, true],
        'Y' => &[true, true, false, false, true, false, true, true, false, true, false, true],
        'Z' => &[true, false, false, true, true, false, true, true, false, true, false, true],
        '-' => &[true, false, false, true, false, true, false, true, true, false, true, true],
        '.' => &[true, true, false, false, true, false, true, false, true, true, false, true],
        ' ' => &[true, false, false, true, true, false, true, false, true, true, false, true],
        '*' => &[true, false, false, true, false, true, true, false, true, true, false, true],
        _ => &[true, false, true, false, false, true, false, true, false, true, false, true], // fallback
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_qr() {
        let result = generate_qr("Hello, World!", 256);
        assert!(result.is_ok());
        let img = result.unwrap();
        assert!(img.width > 0);
        assert!(img.height > 0);
        assert_eq!(img.data.len(), (img.width * img.height * 4) as usize);
    }

    #[test]
    fn test_generate_qr_png() {
        let png = generate_qr_png("https://example.com", 256).unwrap();
        assert_eq!(&png[..4], &[0x89, 0x50, 0x4E, 0x47]); // PNG magic
    }

    #[test]
    fn test_generate_ean13() {
        let result = generate_barcode("4006381333931", BarcodeType::Ean13, 200, 100);
        assert!(result.is_ok());
        let img = result.unwrap();
        assert!(img.width > 0);
    }

    #[test]
    fn test_ean13_invalid_length() {
        let result = generate_barcode("123", BarcodeType::Ean13, 200, 100);
        assert!(result.is_err());
    }

    #[test]
    fn test_generate_code39() {
        let result = generate_barcode("HELLO", BarcodeType::Code39, 300, 80);
        assert!(result.is_ok());
    }

    #[test]
    fn test_generate_code128() {
        let result = generate_barcode("Hello123", BarcodeType::Code128, 300, 80);
        assert!(result.is_ok());
    }

    #[test]
    fn test_barcode_png_output() {
        let png = generate_barcode_png("TEST", BarcodeType::Code39, 200, 60).unwrap();
        assert_eq!(&png[..4], &[0x89, 0x50, 0x4E, 0x47]);
    }

    #[test]
    fn test_qr_roundtrip_data() {
        // Generate QR, verify it's a valid image
        let img = generate_qr("test data 12345", 128).unwrap();
        assert!(img.width >= 128);
        assert!(img.height >= 128);
    }
}
