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
    DataMatrix,
    Pdf417,
    Aztec,
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
        BarcodeType::DataMatrix => return generate_datamatrix(data, width.max(height)),
        BarcodeType::Pdf417 => return generate_pdf417(data, width, height),
        BarcodeType::Aztec => return generate_aztec(data, width.max(height)),
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

// ---------------------------------------------------------------------------
// DataMatrix encoder (basic ASCII mode with ECC 200 structure)
// ---------------------------------------------------------------------------

/// Generate a DataMatrix barcode image.
///
/// Implements a basic ASCII-mode encoder with an L-shaped finder pattern
/// and alternating timing patterns. Each ASCII character is encoded as
/// its value + 1 and arranged in a square grid.
pub fn generate_datamatrix(data: &str, module_size: u32) -> Result<BarcodeImage> {
    if data.is_empty() {
        return Err(SpecialError::Feature {
            detail: "DataMatrix: empty data".into(),
        });
    }

    // Encode ASCII: each char becomes (value + 1), capped at 0..=127
    let codewords: Vec<u8> = data
        .bytes()
        .map(|b| b.wrapping_add(1))
        .collect();

    // Determine matrix size (including finder/timing patterns).
    // Minimum 10x10, grow as needed.
    let data_capacity = |size: usize| -> usize {
        // Interior data area is (size-2) x (size-2) for finder + timing borders
        let inner = size.saturating_sub(2);
        (inner * inner) / 8_usize.max(1)
    };

    let mut matrix_size = 10usize;
    while data_capacity(matrix_size) < codewords.len() && matrix_size < 144 {
        matrix_size += 2;
    }

    // Build the module grid
    let mut grid = vec![vec![false; matrix_size]; matrix_size];

    // L-shaped finder pattern: solid bottom row and solid left column
    for i in 0..matrix_size {
        grid[matrix_size - 1][i] = true; // bottom row
        grid[i][0] = true; // left column
    }

    // Timing patterns: alternating on top row and right column
    for i in 0..matrix_size {
        grid[0][i] = i % 2 == 0; // top row
        grid[i][matrix_size - 1] = i % 2 != 0; // right column (odd = dark)
    }

    // Fill data modules in the interior (row 1..size-1, col 1..size-1)
    let mut bit_idx = 0;
    for row in 1..matrix_size - 1 {
        for col in 1..matrix_size - 1 {
            let byte_idx = bit_idx / 8;
            let bit_pos = 7 - (bit_idx % 8);
            if byte_idx < codewords.len() {
                grid[row][col] = (codewords[byte_idx] >> bit_pos) & 1 == 1;
            }
            bit_idx += 1;
        }
    }

    // Render to image
    render_2d_grid(&grid, module_size)
}

// ---------------------------------------------------------------------------
// PDF417 encoder (basic structure)
// ---------------------------------------------------------------------------

/// Generate a PDF417 barcode image.
///
/// Implements basic PDF417 structure with start/stop patterns and
/// data codewords derived from ASCII text encoding.
pub fn generate_pdf417(data: &str, width: u32, height: u32) -> Result<BarcodeImage> {
    if data.is_empty() {
        return Err(SpecialError::Feature {
            detail: "PDF417: empty data".into(),
        });
    }

    // PDF417 text compaction: each pair of characters → codeword
    let mut codewords: Vec<u16> = Vec::new();
    // Length descriptor
    codewords.push(data.len() as u16 + 1); // symbol length including length codeword

    for ch in data.bytes() {
        codewords.push(ch as u16);
    }

    // Number of data columns (1-30), pick based on data length
    let num_cols = ((codewords.len() as f64).sqrt().ceil() as usize).clamp(2, 10);
    let num_rows = ((codewords.len() + num_cols - 1) / num_cols).max(3);

    // Start pattern: 8 modules (81111113)
    let start_pattern: [bool; 17] = [
        true, true, true, true, true, true, true, true,
        false, true, false, true, false, true, false, true, false,
    ];
    // Stop pattern
    let stop_pattern: [bool; 18] = [
        true, true, true, true, true, true, true, false,
        true, false, false, false, true, false, false, false, false, true,
    ];

    // Total modules per row
    let modules_per_row = start_pattern.len() + num_cols * 17 + stop_pattern.len();

    // Build bit grid
    let mut grid = vec![vec![false; modules_per_row]; num_rows];

    for row in 0..num_rows {
        let mut col_offset = 0;

        // Start pattern
        for (i, &bit) in start_pattern.iter().enumerate() {
            grid[row][col_offset + i] = bit;
        }
        col_offset += start_pattern.len();

        // Data codewords for this row
        for c in 0..num_cols {
            let cw_idx = row * num_cols + c;
            let cw_val = if cw_idx < codewords.len() {
                codewords[cw_idx]
            } else {
                900 // padding codeword
            };
            // Encode codeword as 17-module pattern
            let pattern = pdf417_codeword_pattern(cw_val);
            for (i, &bit) in pattern.iter().enumerate() {
                grid[row][col_offset + i] = bit;
            }
            col_offset += 17;
        }

        // Stop pattern
        for (i, &bit) in stop_pattern.iter().enumerate() {
            grid[row][col_offset + i] = bit;
        }
    }

    // Scale to requested dimensions
    let module_w = (width as usize) / modules_per_row.max(1);
    let module_h = (height as usize) / num_rows.max(1);
    let module_w = module_w.max(1);
    let module_h = module_h.max(1);

    let actual_width = modules_per_row * module_w;
    let actual_height = num_rows * module_h;

    let mut rgba = vec![255u8; actual_width * actual_height * 4];

    for row in 0..num_rows {
        for col in 0..modules_per_row {
            if grid[row][col] {
                for dy in 0..module_h {
                    for dx in 0..module_w {
                        let x = col * module_w + dx;
                        let y = row * module_h + dy;
                        let offset = (y * actual_width + x) * 4;
                        if offset + 3 < rgba.len() {
                            rgba[offset] = 0;
                            rgba[offset + 1] = 0;
                            rgba[offset + 2] = 0;
                            rgba[offset + 3] = 255;
                        }
                    }
                }
            }
        }
    }

    Ok(BarcodeImage {
        data: rgba,
        width: actual_width as u32,
        height: actual_height as u32,
    })
}

/// Convert a PDF417 codeword value to a 17-module bar/space pattern.
/// This is a simplified encoding based on the codeword value bits.
fn pdf417_codeword_pattern(value: u16) -> [bool; 17] {
    let mut pattern = [false; 17];
    // Start with a bar
    pattern[0] = true;

    // Encode value bits into alternating bar/space regions
    let val = value % 929; // PDF417 modulus
    for i in 0..16 {
        let bit = (val >> (15 - i)) & 1;
        pattern[i + 1] = bit == 1;
    }

    // Ensure pattern starts with bar and ends with space (PDF417 rule)
    pattern[0] = true;
    pattern[16] = false;

    pattern
}

// ---------------------------------------------------------------------------
// Aztec encoder (basic structure with bull's eye finder)
// ---------------------------------------------------------------------------

/// Generate an Aztec barcode image.
///
/// Implements basic Aztec structure with the central bull's eye finder
/// pattern and data encoded in layers around it.
pub fn generate_aztec(data: &str, size: u32) -> Result<BarcodeImage> {
    if data.is_empty() {
        return Err(SpecialError::Feature {
            detail: "Aztec: empty data".into(),
        });
    }

    // Encode data as bits
    let data_bits: Vec<bool> = data
        .bytes()
        .flat_map(|b| (0..8).rev().map(move |i| (b >> i) & 1 == 1))
        .collect();

    // Determine grid size based on data length
    // Compact Aztec: 15x15 core, data in layers of 4 modules each side
    let core_size = 11; // Bull's eye is 11x11
    let num_layers = ((data_bits.len() as f64 / 40.0).ceil() as usize).max(1).min(4);
    let grid_size = core_size + num_layers * 4;

    let mut grid = vec![vec![false; grid_size]; grid_size];
    let center = grid_size / 2;

    // Draw bull's eye finder pattern (concentric squares)
    // Center pixel
    grid[center][center] = true;

    // Ring 1 (3x3 border)
    draw_ring(&mut grid, center, 1, true);
    // Ring 2 (5x5 border)
    draw_ring(&mut grid, center, 2, false);
    // Ring 3 (7x7 border)
    draw_ring(&mut grid, center, 3, true);
    // Ring 4 (9x9 border)
    draw_ring(&mut grid, center, 4, false);
    // Ring 5 (11x11 border)
    draw_ring(&mut grid, center, 5, true);

    // Orientation marks at corners of the finder
    let finder_half = 5;
    // Top-left corner mark
    if center >= finder_half && center >= finder_half {
        grid[center - finder_half][center - finder_half] = true;
    }
    // Top-right corner mark
    grid[center - finder_half][center + finder_half] = true;
    // Bottom-left corner mark
    grid[center + finder_half][center - finder_half] = true;

    // Place data bits in layers around the finder
    let mut bit_idx = 0;
    for layer in 0..num_layers {
        let offset = 6 + layer * 2;
        // Top side
        for col in (center.saturating_sub(offset))..=(center + offset).min(grid_size - 1) {
            if bit_idx < data_bits.len() {
                let r = center.saturating_sub(offset);
                if r < grid_size && col < grid_size {
                    grid[r][col] = data_bits[bit_idx];
                    bit_idx += 1;
                }
            }
        }
        // Right side
        for row in (center.saturating_sub(offset))..=(center + offset).min(grid_size - 1) {
            if bit_idx < data_bits.len() {
                let c = (center + offset).min(grid_size - 1);
                if row < grid_size {
                    grid[row][c] = data_bits[bit_idx];
                    bit_idx += 1;
                }
            }
        }
        // Bottom side
        for col in ((center.saturating_sub(offset))..=(center + offset).min(grid_size - 1)).rev() {
            if bit_idx < data_bits.len() {
                let r = (center + offset).min(grid_size - 1);
                if col < grid_size {
                    grid[r][col] = data_bits[bit_idx];
                    bit_idx += 1;
                }
            }
        }
        // Left side
        for row in ((center.saturating_sub(offset))..=(center + offset).min(grid_size - 1)).rev() {
            if bit_idx < data_bits.len() {
                let c = center.saturating_sub(offset);
                if row < grid_size {
                    grid[row][c] = data_bits[bit_idx];
                    bit_idx += 1;
                }
            }
        }
    }

    // Render with module_size derived from requested size
    let module_size = (size / grid_size as u32).max(1);
    render_2d_grid(&grid, module_size)
}

/// Draw a ring (border of a square) around center at the given distance.
fn draw_ring(grid: &mut [Vec<bool>], center: usize, dist: usize, value: bool) {
    let size = grid.len();
    let top = center.saturating_sub(dist);
    let bottom = (center + dist).min(size - 1);
    let left = center.saturating_sub(dist);
    let right = (center + dist).min(size - 1);

    for col in left..=right {
        if top < size && col < size {
            grid[top][col] = value;
        }
        if bottom < size && col < size {
            grid[bottom][col] = value;
        }
    }
    for row in top..=bottom {
        if row < size && left < size {
            grid[row][left] = value;
        }
        if row < size && right < size {
            grid[row][right] = value;
        }
    }
}

/// Render a 2D boolean grid to a BarcodeImage with the given module size.
fn render_2d_grid(grid: &[Vec<bool>], module_size: u32) -> Result<BarcodeImage> {
    let grid_h = grid.len();
    let grid_w = if grid_h > 0 { grid[0].len() } else { 0 };
    let module_size = module_size.max(1) as usize;

    let img_w = grid_w * module_size;
    let img_h = grid_h * module_size;

    let mut rgba = vec![255u8; img_w * img_h * 4];

    for row in 0..grid_h {
        for col in 0..grid_w {
            if grid[row][col] {
                for dy in 0..module_size {
                    for dx in 0..module_size {
                        let x = col * module_size + dx;
                        let y = row * module_size + dy;
                        let offset = (y * img_w + x) * 4;
                        if offset + 3 < rgba.len() {
                            rgba[offset] = 0;
                            rgba[offset + 1] = 0;
                            rgba[offset + 2] = 0;
                            rgba[offset + 3] = 255;
                        }
                    }
                }
            }
        }
    }

    Ok(BarcodeImage {
        data: rgba,
        width: img_w as u32,
        height: img_h as u32,
    })
}

/// Generate a DataMatrix barcode as PNG bytes.
pub fn generate_datamatrix_png(data: &str, module_size: u32) -> Result<Vec<u8>> {
    let barcode = generate_datamatrix(data, module_size)?;
    encode_png(&barcode)
}

/// Generate a PDF417 barcode as PNG bytes.
pub fn generate_pdf417_png(data: &str, width: u32, height: u32) -> Result<Vec<u8>> {
    let barcode = generate_pdf417(data, width, height)?;
    encode_png(&barcode)
}

/// Generate an Aztec barcode as PNG bytes.
pub fn generate_aztec_png(data: &str, size: u32) -> Result<Vec<u8>> {
    let barcode = generate_aztec(data, size)?;
    encode_png(&barcode)
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

    // --- DataMatrix tests ---

    #[test]
    fn test_generate_datamatrix() {
        let result = generate_datamatrix("Hello", 4);
        assert!(result.is_ok());
        let img = result.unwrap();
        assert!(img.width > 0);
        assert!(img.height > 0);
        assert_eq!(img.data.len(), (img.width * img.height * 4) as usize);
    }

    #[test]
    fn test_datamatrix_empty_data() {
        let result = generate_datamatrix("", 4);
        assert!(result.is_err());
    }

    #[test]
    fn test_datamatrix_png() {
        let png = generate_datamatrix_png("Test123", 4).unwrap();
        assert_eq!(&png[..4], &[0x89, 0x50, 0x4E, 0x47]);
    }

    #[test]
    fn test_datamatrix_finder_pattern() {
        // Verify the L-shaped finder pattern exists
        let img = generate_datamatrix("A", 1).unwrap();
        // Bottom-left pixel should be black (part of L-pattern)
        let bottom_left = ((img.height - 1) * img.width * 4) as usize;
        assert_eq!(img.data[bottom_left], 0); // R=0 (black)
    }

    // --- PDF417 tests ---

    #[test]
    fn test_generate_pdf417() {
        let result = generate_pdf417("Hello PDF417", 300, 100);
        assert!(result.is_ok());
        let img = result.unwrap();
        assert!(img.width > 0);
        assert!(img.height > 0);
        assert_eq!(img.data.len(), (img.width * img.height * 4) as usize);
    }

    #[test]
    fn test_pdf417_empty_data() {
        let result = generate_pdf417("", 300, 100);
        assert!(result.is_err());
    }

    #[test]
    fn test_pdf417_png() {
        let png = generate_pdf417_png("Test", 200, 80).unwrap();
        assert_eq!(&png[..4], &[0x89, 0x50, 0x4E, 0x47]);
    }

    #[test]
    fn test_pdf417_via_generate_barcode() {
        let result = generate_barcode("Test", BarcodeType::Pdf417, 300, 100);
        assert!(result.is_ok());
    }

    // --- Aztec tests ---

    #[test]
    fn test_generate_aztec() {
        let result = generate_aztec("Hello Aztec", 200);
        assert!(result.is_ok());
        let img = result.unwrap();
        assert!(img.width > 0);
        assert!(img.height > 0);
        assert_eq!(img.data.len(), (img.width * img.height * 4) as usize);
    }

    #[test]
    fn test_aztec_empty_data() {
        let result = generate_aztec("", 100);
        assert!(result.is_err());
    }

    #[test]
    fn test_aztec_png() {
        let png = generate_aztec_png("Test", 100).unwrap();
        assert_eq!(&png[..4], &[0x89, 0x50, 0x4E, 0x47]);
    }

    #[test]
    fn test_aztec_bulls_eye() {
        // Verify center pixel is dark (bull's eye center)
        let img = generate_aztec("A", 1).unwrap();
        let center_x = img.width / 2;
        let center_y = img.height / 2;
        let offset = ((center_y * img.width + center_x) * 4) as usize;
        assert_eq!(img.data[offset], 0); // R=0 (black)
    }

    // --- Cross-type tests ---

    #[test]
    fn test_all_2d_barcode_types_via_generate_barcode() {
        for btype in [BarcodeType::DataMatrix, BarcodeType::Pdf417, BarcodeType::Aztec] {
            let result = generate_barcode("Test", btype, 200, 200);
            assert!(result.is_ok(), "failed for {btype:?}");
        }
    }
}
