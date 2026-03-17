use crate::error::{JustPdfError, Result};

/// Decode DCTDecode (JPEG) data.
/// Returns raw pixel data (RGB or Grayscale).
pub fn decode(data: &[u8]) -> Result<DecodedImage> {
    let mut decoder = jpeg_decoder::Decoder::new(data);
    let pixels = decoder.decode().map_err(|e| JustPdfError::StreamDecode {
        filter: "DCTDecode".into(),
        detail: format!("JPEG decode error: {e}"),
    })?;

    let info = decoder.info().ok_or_else(|| JustPdfError::StreamDecode {
        filter: "DCTDecode".into(),
        detail: "no JPEG image info".into(),
    })?;

    let color_type = match info.pixel_format {
        jpeg_decoder::PixelFormat::L8 => ColorType::Gray,
        jpeg_decoder::PixelFormat::RGB24 => ColorType::Rgb,
        jpeg_decoder::PixelFormat::CMYK32 => ColorType::Cmyk,
        jpeg_decoder::PixelFormat::L16 => ColorType::Gray,
    };

    Ok(DecodedImage {
        width: info.width as u32,
        height: info.height as u32,
        color_type,
        data: pixels,
    })
}

/// Get JPEG image dimensions without fully decoding.
pub fn jpeg_dimensions(data: &[u8]) -> Result<(u32, u32)> {
    let mut decoder = jpeg_decoder::Decoder::new(data);
    decoder
        .read_info()
        .map_err(|e| JustPdfError::StreamDecode {
            filter: "DCTDecode".into(),
            detail: format!("JPEG header error: {e}"),
        })?;
    let info = decoder.info().ok_or_else(|| JustPdfError::StreamDecode {
        filter: "DCTDecode".into(),
        detail: "no JPEG image info".into(),
    })?;
    Ok((info.width as u32, info.height as u32))
}

/// Color type of a decoded image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorType {
    Gray,
    Rgb,
    Cmyk,
}

impl ColorType {
    pub fn components(&self) -> usize {
        match self {
            Self::Gray => 1,
            Self::Rgb => 3,
            Self::Cmyk => 4,
        }
    }
}

/// A decoded image with pixel data.
#[derive(Debug, Clone)]
pub struct DecodedImage {
    pub width: u32,
    pub height: u32,
    pub color_type: ColorType,
    pub data: Vec<u8>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_corrupted_jpeg() {
        let result = decode(b"\x00\x01\x02\x03");
        assert!(result.is_err());
    }

    // Note: a real JPEG decode test would require a JPEG fixture file.
    // The minimal JPEG is ~600 bytes so we don't embed one here.
    // Integration tests with real PDFs cover this.
}
