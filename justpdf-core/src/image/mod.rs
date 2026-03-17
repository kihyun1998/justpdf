use crate::error::{JustPdfError, Result};
use crate::object::{PdfDict, PdfObject};
use crate::stream;
use crate::stream::dct;

/// Information about a PDF image XObject.
#[derive(Debug, Clone)]
pub struct ImageInfo {
    /// Image width in pixels.
    pub width: u32,
    /// Image height in pixels.
    pub height: u32,
    /// Bits per component (1, 2, 4, 8, 16).
    pub bits_per_component: u32,
    /// Color space name.
    pub color_space: Vec<u8>,
    /// Number of color components.
    pub num_components: u32,
    /// Filter used to encode the image data.
    pub filter: Option<Vec<u8>>,
    /// Whether this is an image mask.
    pub is_mask: bool,
    /// Whether there is a soft mask.
    pub has_smask: bool,
}

/// Extract image info from an image XObject dictionary.
pub fn image_info(dict: &PdfDict) -> Option<ImageInfo> {
    let width = dict.get_i64(b"Width")? as u32;
    let height = dict.get_i64(b"Height")? as u32;

    let is_mask = dict
        .get(b"ImageMask")
        .and_then(|o| o.as_bool())
        .unwrap_or(false);

    let bits_per_component = if is_mask {
        1
    } else {
        dict.get_i64(b"BitsPerComponent").unwrap_or(8) as u32
    };

    let color_space = dict
        .get(b"ColorSpace")
        .and_then(|o| o.as_name())
        .unwrap_or(if is_mask { b"DeviceGray" } else { b"DeviceRGB" })
        .to_vec();

    let num_components = match color_space.as_slice() {
        b"DeviceGray" | b"CalGray" | b"G" => 1,
        b"DeviceRGB" | b"CalRGB" | b"RGB" => 3,
        b"DeviceCMYK" | b"CMYK" => 4,
        _ => 3, // default assumption
    };

    let filter = match dict.get(b"Filter") {
        Some(PdfObject::Name(n)) => Some(n.clone()),
        Some(PdfObject::Array(arr)) => arr.last().and_then(|o| o.as_name()).map(|n| n.to_vec()),
        _ => None,
    };

    let has_smask = dict.get(b"SMask").is_some();

    Some(ImageInfo {
        width,
        height,
        bits_per_component,
        color_space,
        num_components,
        filter,
        is_mask,
        has_smask,
    })
}

/// Decoded image data with metadata.
#[derive(Debug, Clone)]
pub struct DecodedImage {
    pub width: u32,
    pub height: u32,
    /// Number of color components.
    pub components: u32,
    /// Bits per component.
    pub bpc: u32,
    /// Raw pixel data (decoded).
    pub data: Vec<u8>,
    /// The image format the data came from.
    pub source_format: ImageFormat,
}

/// The original encoding format of the image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    /// Raw/uncompressed or FlateDecode.
    Raw,
    /// JPEG (DCTDecode).
    Jpeg,
    /// JPEG2000 (JPXDecode).
    Jpeg2000,
    /// JBIG2.
    Jbig2,
    /// CCITT Fax.
    CcittFax,
}

/// Decode an image XObject's stream data.
pub fn decode_image(raw_data: &[u8], dict: &PdfDict) -> Result<DecodedImage> {
    let info = image_info(dict).ok_or_else(|| JustPdfError::StreamDecode {
        filter: "image".into(),
        detail: "missing Width or Height in image dict".into(),
    })?;

    let filter = info.filter.as_deref();

    match filter {
        Some(b"DCTDecode") | Some(b"DCT") => {
            let decoded = dct::decode(raw_data)?;
            Ok(DecodedImage {
                width: decoded.width,
                height: decoded.height,
                components: decoded.color_type.components() as u32,
                bpc: 8,
                data: decoded.data,
                source_format: ImageFormat::Jpeg,
            })
        }
        Some(b"JPXDecode") => {
            // JPEG2000: not implemented, return raw
            Ok(DecodedImage {
                width: info.width,
                height: info.height,
                components: info.num_components,
                bpc: info.bits_per_component,
                data: raw_data.to_vec(),
                source_format: ImageFormat::Jpeg2000,
            })
        }
        Some(b"JBIG2Decode") => Ok(DecodedImage {
            width: info.width,
            height: info.height,
            components: 1,
            bpc: 1,
            data: raw_data.to_vec(),
            source_format: ImageFormat::Jbig2,
        }),
        Some(b"CCITTFaxDecode") | Some(b"CCF") => Ok(DecodedImage {
            width: info.width,
            height: info.height,
            components: 1,
            bpc: 1,
            data: raw_data.to_vec(),
            source_format: ImageFormat::CcittFax,
        }),
        _ => {
            // Raw or FlateDecode (already decoded by stream decoder)
            let decoded = stream::decode_stream(raw_data, dict)?;
            Ok(DecodedImage {
                width: info.width,
                height: info.height,
                components: info.num_components,
                bpc: info.bits_per_component,
                data: decoded,
                source_format: ImageFormat::Raw,
            })
        }
    }
}

/// Extract raw JPEG bytes from a DCTDecode image stream (passthrough, no decoding).
pub fn extract_jpeg_bytes(raw_data: &[u8], dict: &PdfDict) -> Result<Vec<u8>> {
    let filter = match dict.get(b"Filter") {
        Some(PdfObject::Name(n)) => n.clone(),
        Some(PdfObject::Array(arr)) => arr.last().and_then(|o| o.as_name()).unwrap_or(b"").to_vec(),
        _ => Vec::new(),
    };

    if filter == b"DCTDecode" || filter == b"DCT" {
        Ok(raw_data.to_vec())
    } else {
        Err(JustPdfError::StreamDecode {
            filter: "image".into(),
            detail: "not a JPEG image".into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_image_info_basic() {
        let mut dict = PdfDict::new();
        dict.insert(b"Type".to_vec(), PdfObject::Name(b"XObject".to_vec()));
        dict.insert(b"Subtype".to_vec(), PdfObject::Name(b"Image".to_vec()));
        dict.insert(b"Width".to_vec(), PdfObject::Integer(100));
        dict.insert(b"Height".to_vec(), PdfObject::Integer(200));
        dict.insert(b"BitsPerComponent".to_vec(), PdfObject::Integer(8));
        dict.insert(
            b"ColorSpace".to_vec(),
            PdfObject::Name(b"DeviceRGB".to_vec()),
        );

        let info = image_info(&dict).unwrap();
        assert_eq!(info.width, 100);
        assert_eq!(info.height, 200);
        assert_eq!(info.bits_per_component, 8);
        assert_eq!(info.num_components, 3);
        assert!(!info.is_mask);
    }

    #[test]
    fn test_image_info_mask() {
        let mut dict = PdfDict::new();
        dict.insert(b"Width".to_vec(), PdfObject::Integer(50));
        dict.insert(b"Height".to_vec(), PdfObject::Integer(50));
        dict.insert(b"ImageMask".to_vec(), PdfObject::Bool(true));

        let info = image_info(&dict).unwrap();
        assert!(info.is_mask);
        assert_eq!(info.bits_per_component, 1);
    }

    #[test]
    fn test_image_info_jpeg() {
        let mut dict = PdfDict::new();
        dict.insert(b"Width".to_vec(), PdfObject::Integer(640));
        dict.insert(b"Height".to_vec(), PdfObject::Integer(480));
        dict.insert(b"BitsPerComponent".to_vec(), PdfObject::Integer(8));
        dict.insert(
            b"ColorSpace".to_vec(),
            PdfObject::Name(b"DeviceRGB".to_vec()),
        );
        dict.insert(b"Filter".to_vec(), PdfObject::Name(b"DCTDecode".to_vec()));

        let info = image_info(&dict).unwrap();
        assert_eq!(info.filter, Some(b"DCTDecode".to_vec()));
    }

    #[test]
    fn test_image_info_missing_dims() {
        let dict = PdfDict::new();
        assert!(image_info(&dict).is_none());
    }

    #[test]
    fn test_image_info_cmyk() {
        let mut dict = PdfDict::new();
        dict.insert(b"Width".to_vec(), PdfObject::Integer(100));
        dict.insert(b"Height".to_vec(), PdfObject::Integer(100));
        dict.insert(
            b"ColorSpace".to_vec(),
            PdfObject::Name(b"DeviceCMYK".to_vec()),
        );

        let info = image_info(&dict).unwrap();
        assert_eq!(info.num_components, 4);
    }
}
