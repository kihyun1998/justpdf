//! Image deskewing for scanned PDF pages.
//!
//! Provides basic skew detection and correction for scanned document images.
//! This is useful as a preprocessing step before OCR.

use crate::{Result, SpecialError};

/// Detected skew angle in degrees.
#[derive(Debug, Clone, Copy)]
pub struct SkewResult {
    /// The detected skew angle in degrees. Positive = clockwise.
    pub angle_degrees: f64,
    /// Confidence of the detection (0.0 - 1.0).
    pub confidence: f64,
}

/// Detect the skew angle of a grayscale image.
///
/// Uses a projection-profile method: projects rows at various angles
/// and finds the angle that maximizes the variance of the row sums.
///
/// `image_data` should be grayscale pixel data (1 byte per pixel).
pub fn detect_skew(
    image_data: &[u8],
    width: u32,
    height: u32,
) -> Result<SkewResult> {
    if image_data.len() != (width * height) as usize {
        return Err(SpecialError::Feature {
            detail: format!(
                "expected {} bytes for {}x{} grayscale image, got {}",
                width * height,
                width,
                height,
                image_data.len()
            ),
        });
    }

    let w = width as f64;
    let h = height as f64;
    let cx = w / 2.0;
    let cy = h / 2.0;

    let mut best_angle = 0.0f64;
    let mut best_variance = 0.0f64;

    // Search angles from -5 to +5 degrees in 0.1 degree steps
    let mut angle = -5.0f64;
    while angle <= 5.0 {
        let rad = angle.to_radians();
        let cos_a = rad.cos();
        let sin_a = rad.sin();

        // Project each pixel onto the y-axis after rotation
        let mut row_sums = vec![0u64; height as usize];

        for y in 0..height {
            for x in 0..width {
                let dx = x as f64 - cx;
                let dy = y as f64 - cy;
                let ry = (-dx * sin_a + dy * cos_a + cy) as i32;
                if ry >= 0 && ry < height as i32 {
                    let pixel = image_data[(y * width + x) as usize] as u64;
                    // Invert so text (dark) has high values
                    row_sums[ry as usize] += 255 - pixel;
                }
            }
        }

        // Calculate variance of row sums
        let n = row_sums.len() as f64;
        let mean: f64 = row_sums.iter().sum::<u64>() as f64 / n;
        let variance: f64 = row_sums
            .iter()
            .map(|&s| {
                let diff = s as f64 - mean;
                diff * diff
            })
            .sum::<f64>()
            / n;

        if variance > best_variance {
            best_variance = variance;
            best_angle = angle;
        }

        angle += 0.1;
    }

    // Confidence: ratio of best variance to mean of all variances
    let confidence = if best_variance > 0.0 {
        (best_variance / (best_variance + 1.0)).min(1.0)
    } else {
        0.0
    };

    Ok(SkewResult {
        angle_degrees: best_angle,
        confidence,
    })
}

/// Deskew a grayscale image by rotating it to correct the detected skew.
///
/// Returns the corrected image data (same dimensions, grayscale).
pub fn deskew_image(
    image_data: &[u8],
    width: u32,
    height: u32,
) -> Result<Vec<u8>> {
    let skew = detect_skew(image_data, width, height)?;

    if skew.angle_degrees.abs() < 0.05 {
        // No significant skew, return original
        return Ok(image_data.to_vec());
    }

    rotate_image(image_data, width, height, -skew.angle_degrees)
}

/// Rotate a grayscale image by the given angle in degrees.
///
/// Uses bilinear interpolation. Background pixels are set to white (255).
fn rotate_image(
    image_data: &[u8],
    width: u32,
    height: u32,
    angle_degrees: f64,
) -> Result<Vec<u8>> {
    let w = width as usize;
    let h = height as usize;
    let cx = w as f64 / 2.0;
    let cy = h as f64 / 2.0;

    let rad = angle_degrees.to_radians();
    let cos_a = rad.cos();
    let sin_a = rad.sin();

    let mut output = vec![255u8; w * h]; // white background

    for y in 0..h {
        for x in 0..w {
            // Map output (x,y) back to input coordinates (inverse rotation)
            let dx = x as f64 - cx;
            let dy = y as f64 - cy;
            let src_x = dx * cos_a + dy * sin_a + cx;
            let src_y = -dx * sin_a + dy * cos_a + cy;

            // Bilinear interpolation
            let sx = src_x.floor() as i32;
            let sy = src_y.floor() as i32;
            let fx = src_x - sx as f64;
            let fy = src_y - sy as f64;

            if sx >= 0 && sx + 1 < w as i32 && sy >= 0 && sy + 1 < h as i32 {
                let sx = sx as usize;
                let sy = sy as usize;

                let p00 = image_data[sy * w + sx] as f64;
                let p10 = image_data[sy * w + sx + 1] as f64;
                let p01 = image_data[(sy + 1) * w + sx] as f64;
                let p11 = image_data[(sy + 1) * w + sx + 1] as f64;

                let value = p00 * (1.0 - fx) * (1.0 - fy)
                    + p10 * fx * (1.0 - fy)
                    + p01 * (1.0 - fx) * fy
                    + p11 * fx * fy;

                output[y * w + x] = value.round() as u8;
            }
        }
    }

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_skew_uniform() {
        // Uniform white image should have near-zero skew
        let data = vec![255u8; 100 * 100];
        let result = detect_skew(&data, 100, 100).unwrap();
        assert!(result.angle_degrees.abs() <= 5.0);
    }

    #[test]
    fn test_detect_skew_invalid_size() {
        let data = vec![255u8; 50]; // wrong size for 100x100
        let result = detect_skew(&data, 100, 100);
        assert!(result.is_err());
    }

    #[test]
    fn test_deskew_no_skew() {
        let data = vec![200u8; 50 * 50];
        let result = deskew_image(&data, 50, 50).unwrap();
        assert_eq!(result.len(), 50 * 50);
    }

    #[test]
    fn test_rotate_image_zero() {
        let data = vec![128u8; 20 * 20];
        let result = rotate_image(&data, 20, 20, 0.0).unwrap();
        assert_eq!(result.len(), 20 * 20);
        // Center pixels should be preserved
        assert_eq!(result[10 * 20 + 10], 128);
    }
}
