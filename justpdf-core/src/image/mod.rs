use crate::color::{Color, ColorSpace};
use crate::error::{JustPdfError, Result};
use crate::object::{PdfDict, PdfObject};
use crate::parser::PdfDocument;
use crate::stream;
use crate::stream::dct;

/// Information about a PDF image XObject.
#[derive(Debug, Clone)]
pub struct ImageInfo {
    /// Image width in pixels.
    pub width: u32,
    /// Image height in pixels.
    pub height: u32,
    /// Bits per component: 1 for an image mask, 8 when `/BitsPerComponent`
    /// is absent or 0, else its value (`u32::MAX` for a negative one).
    /// `decode_image` refuses a value outside 1..=16, except for JPX.
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
        match dict.get_i64(b"BitsPerComponent") {
            None | Some(0) => 8,
            Some(bits) => u32::try_from(bits).unwrap_or(u32::MAX),
        }
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
    /// The image's colour space, resolved; the samples are in it.
    pub color_space: ColorSpace,
    /// The image's `/Decode` array, when given.
    pub decode: Option<Vec<f64>>,
}

impl DecodedImage {
    /// The image as 8-bit RGB, `width × height × 3` bytes. Samples are
    /// unpacked by `bpc` (1, 2, 4, 8 or 16, each row starting on a byte
    /// boundary), mapped through `/Decode` (ISO 32000-2 §8.9.5.2), and
    /// converted from the colour space. A colour space whose component count
    /// differs from the buffer's is replaced by the device space with that
    /// many components. Missing samples read as 0.
    pub fn to_rgb8(&self) -> Vec<u8> {
        let (w, h) = (self.width as usize, self.height as usize);
        let n = self.components.max(1) as usize;
        let mut out = vec![0u8; w * h * 3];
        let space = self.sample_space(n);

        if self.bpc == 8 && self.decode.is_none() {
            let pixels = (w * h).min(self.data.len() / n);
            match (&space, n) {
                (ColorSpace::DeviceGray, 1) => {
                    for (px, &g) in out.chunks_exact_mut(3).zip(&self.data[..pixels]) {
                        px.copy_from_slice(&[g, g, g]);
                    }
                    return out;
                }
                (ColorSpace::DeviceRGB, 3) => {
                    out[..pixels * 3].copy_from_slice(&self.data[..pixels * 3]);
                    return out;
                }
                (ColorSpace::DeviceCMYK, 4) => {
                    for (px, cmyk) in out.chunks_exact_mut(3).zip(self.data.chunks_exact(4)) {
                        let [c, m, y, k] =
                            [cmyk[0], cmyk[1], cmyk[2], cmyk[3]].map(|v| v as f64 / 255.0);
                        px.copy_from_slice(&[
                            ((1.0 - c) * (1.0 - k) * 255.0) as u8,
                            ((1.0 - m) * (1.0 - k) * 255.0) as u8,
                            ((1.0 - y) * (1.0 - k) * 255.0) as u8,
                        ]);
                    }
                    return out;
                }
                _ => {}
            }
        }

        let bpc = self.bpc.clamp(1, 16);
        let max = ((1u32 << bpc) - 1) as f64;
        let decode = self.decode_ranges(&space, n, max);
        let decoded =
            |c: usize, raw: f64| decode[2 * c] + raw * (decode[2 * c + 1] - decode[2 * c]) / max;
        let to_rgb = |components: &[f64]| -> [u8; 3] {
            let rgb = match (&space, components) {
                (ColorSpace::DeviceGray, &[g]) => [g, g, g],
                (ColorSpace::DeviceRGB, &[r, g, b]) => [r, g, b],
                (ColorSpace::DeviceCMYK, &[c, m, y, k]) => crate::color::cmyk_to_rgb(c, m, y, k),
                _ => Color {
                    components: components.to_vec(),
                }
                .to_rgb(&space),
            };
            rgb.map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u8)
        };
        let row_bytes = (w * n * bpc as usize).div_ceil(8);

        // One component of at most 8 bits: one conversion per sample value.
        if n == 1 && bpc <= 8 {
            let table: Vec<[u8; 3]> = (0..=max as u32)
                .map(|v| to_rgb(&[decoded(0, v as f64)]))
                .collect();
            for y in 0..h {
                let row = y * row_bytes;
                for x in 0..w {
                    let v = if bpc == 8 {
                        self.data.get(row + x).copied().unwrap_or(0) as u32
                    } else {
                        read_sample(&self.data, row * 8 + x * bpc as usize, bpc)
                    };
                    out[(y * w + x) * 3..][..3].copy_from_slice(&table[v as usize]);
                }
            }
            return out;
        }

        // Each component's decoded value per raw sample, for at most 8 bits.
        let tables: Option<Vec<Vec<f64>>> = (bpc <= 8).then(|| {
            (0..n)
                .map(|c| (0..=max as u32).map(|v| decoded(c, v as f64)).collect())
                .collect()
        });
        let mut components = vec![0.0; n];
        for y in 0..h {
            let row = y * row_bytes;
            for x in 0..w {
                for (c, value) in components.iter_mut().enumerate() {
                    let raw = if bpc == 8 {
                        self.data.get(row + x * n + c).copied().unwrap_or(0) as u32
                    } else {
                        read_sample(&self.data, row * 8 + (x * n + c) * bpc as usize, bpc)
                    };
                    *value = match &tables {
                        Some(t) => t[c][raw as usize],
                        None => decoded(c, raw as f64),
                    };
                }
                out[(y * w + x) * 3..][..3].copy_from_slice(&to_rgb(&components));
            }
        }
        out
    }

    /// Whether the samples are gray already: one component in a gray space
    /// (DeviceGray, CalGray, or ICCBased standing in for one).
    pub fn is_gray(&self) -> bool {
        let n = self.components.max(1) as usize;
        matches!(
            self.sample_space(n),
            ColorSpace::DeviceGray | ColorSpace::CalGray { .. }
        )
    }

    /// The space the samples convert from: the colour space (through its
    /// substitute for ICCBased), or the device space with `n` components
    /// when the counts differ.
    fn sample_space(&self, n: usize) -> ColorSpace {
        let space = self
            .color_space
            .substitute()
            .unwrap_or_else(|| self.color_space.clone());
        if space.num_components() == n {
            return space;
        }
        match n {
            1 => ColorSpace::DeviceGray,
            4 => ColorSpace::DeviceCMYK,
            _ => ColorSpace::DeviceRGB,
        }
    }

    /// `/Decode` as `[min, max]` per component, else the default: `[0, max]`
    /// for an Indexed index, `[0, 1]` otherwise.
    fn decode_ranges(&self, space: &ColorSpace, n: usize, max: f64) -> Vec<f64> {
        match &self.decode {
            Some(d) if d.len() >= 2 * n => d[..2 * n].to_vec(),
            _ => match space {
                ColorSpace::Indexed { .. } => vec![0.0, max],
                _ => [0.0, 1.0].repeat(n),
            },
        }
    }
}

/// The `bpc`-bit sample starting at bit `bit` of `data`, most significant bit
/// first; 0 past the end.
fn read_sample(data: &[u8], bit: usize, bpc: u32) -> u32 {
    let byte = bit / 8;
    match bpc {
        8 => data.get(byte).copied().unwrap_or(0) as u32,
        16 => {
            let hi = data.get(byte).copied().unwrap_or(0) as u32;
            let lo = data.get(byte + 1).copied().unwrap_or(0) as u32;
            (hi << 8) | lo
        }
        _ if bit % 8 + bpc as usize <= 8 => {
            let shift = 8 - bpc as usize - bit % 8;
            let mask = (1u32 << bpc) - 1;
            (data.get(byte).copied().unwrap_or(0) as u32 >> shift) & mask
        }
        _ => (bit..bit + bpc as usize).fold(0, |value, b| {
            let byte = data.get(b / 8).copied().unwrap_or(0);
            (value << 1) | ((byte >> (7 - b % 8)) & 1) as u32
        }),
    }
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

/// The byte length of a `width` × `height` image as RGBA: `None` when
/// either side is 0 or the length would exceed `i32::MAX`. `decode_image`
/// refuses an image this returns `None` for.
pub fn rgba_len(width: u32, height: u32) -> Option<usize> {
    if width == 0 || height == 0 {
        return None;
    }
    let len = (width as usize)
        .checked_mul(height as usize)?
        .checked_mul(4)?;
    (len <= i32::MAX as usize).then_some(len)
}

/// `StreamDecode` for an image `rgba_len` refuses.
fn check_size(width: u32, height: u32) -> Result<()> {
    match rgba_len(width, height) {
        Some(_) => Ok(()),
        None => Err(JustPdfError::StreamDecode {
            filter: "image".into(),
            detail: format!("image size {width} x {height} is empty or too large"),
        }),
    }
}

/// Decode an image XObject's raw stream data: every filter of `/Filter`
/// in order, the image codec last; `doc` resolves `/JBIG2Globals`. An image
/// whose size `rgba_len` refuses is `StreamDecode`: the dictionary's size,
/// checked before decoding, or for DCT the JPEG header's and for JPX and
/// JBIG2 the codec's. So is an image other than JPX whose
/// `/BitsPerComponent` is outside 1..=16.
pub fn decode_image(raw_data: &[u8], dict: &PdfDict, doc: &PdfDocument) -> Result<DecodedImage> {
    let info = image_info(dict).ok_or_else(|| JustPdfError::StreamDecode {
        filter: "image".into(),
        detail: "missing Width or Height in image dict".into(),
    })?;

    let filter = info.filter.as_deref();
    if filter != Some(b"JPXDecode") && !(1..=16).contains(&info.bits_per_component) {
        return Err(JustPdfError::StreamDecode {
            filter: "image".into(),
            detail: format!(
                "/BitsPerComponent {} outside 1..=16",
                info.bits_per_component
            ),
        });
    }
    let color_space = match dict.get(b"ColorSpace") {
        Some(cs) => ColorSpace::resolve(cs, doc),
        None if info.is_mask => ColorSpace::DeviceGray,
        None => ColorSpace::Unknown(Vec::new()),
    };
    let decode = dict
        .get_array(b"Decode")
        .map(|d| d.iter().filter_map(PdfObject::as_f64).collect::<Vec<f64>>());
    let raw_components = match color_space.num_components() {
        0 => info.num_components,
        n => n as u32,
    };

    let decoded: Result<DecodedImage> = match filter {
        Some(b"DCTDecode") | Some(b"DCT") => {
            let codec_input = stream::decode_stream_cow(raw_data, dict)?;
            let (w, h) = dct::jpeg_dimensions(&codec_input)?;
            check_size(w, h)?;
            let decoded = dct::decode(&codec_input)?;
            Ok(DecodedImage {
                width: decoded.width,
                height: decoded.height,
                components: decoded.color_type.components() as u32,
                bpc: 8,
                data: decoded.data,
                source_format: ImageFormat::Jpeg,
                color_space: ColorSpace::Unknown(Vec::new()),
                decode: None,
            })
        }
        Some(b"JPXDecode") => {
            let codec_input = stream::decode_stream_cow(raw_data, dict)?;
            let jp2_image =
                justjp2::decode(&codec_input).map_err(|e| JustPdfError::StreamDecode {
                    filter: "JPXDecode".into(),
                    detail: format!("JPEG2000 decode error: {e}"),
                })?;
            let num_comp = jp2_image.components.len() as u32;
            if num_comp == 0 || jp2_image.components[0].data.is_empty() {
                return Err(JustPdfError::StreamDecode {
                    filter: "JPXDecode".into(),
                    detail: "empty JPEG2000 image".into(),
                });
            }
            let w = jp2_image.width;
            let h = jp2_image.height;
            check_size(w, h)?;
            let pixel_count = w as usize * h as usize;
            // Interleave components into a flat pixel buffer, clamping i32→u8
            let mut data = Vec::with_capacity(pixel_count * num_comp as usize);
            for i in 0..pixel_count {
                for comp in &jp2_image.components {
                    let val = comp.data.get(i).copied().unwrap_or(0);
                    data.push(val.clamp(0, 255) as u8);
                }
            }
            Ok(DecodedImage {
                width: w,
                height: h,
                components: num_comp,
                bpc: 8,
                data,
                source_format: ImageFormat::Jpeg2000,
                color_space: ColorSpace::Unknown(Vec::new()),
                decode: None,
            })
        }
        Some(b"JBIG2Decode") => {
            let codec_input = stream::decode_stream_cow(raw_data, dict)?;
            let pages = match jbig2_globals(dict, doc)? {
                Some(globals) => justbig2::decode_embedded_with_globals(&codec_input, &globals),
                None => justbig2::decode_embedded(&codec_input),
            }
            .map_err(|e| JustPdfError::StreamDecode {
                filter: "JBIG2Decode".into(),
                detail: format!("JBIG2 decode error: {e}"),
            })?;
            let page = pages
                .into_iter()
                .next()
                .ok_or_else(|| JustPdfError::StreamDecode {
                    filter: "JBIG2Decode".into(),
                    detail: "no pages decoded from JBIG2 stream".into(),
                })?;
            // JBIG2: 1BPP packed, MSB-first, stride-aligned rows
            // Expand to 1-byte-per-pixel grayscale
            // JBIG2 convention: 1=black→0x00, 0=white→0xFF
            let w = page.width;
            let h = page.height;
            check_size(w, h)?;
            let pixel_count = w as usize * h as usize;
            let mut data = Vec::with_capacity(pixel_count);
            for y in 0..h {
                for x in 0..w {
                    let byte_idx = (y * page.stride + x / 8) as usize;
                    let bit_idx = 7 - (x % 8);
                    let bit = if byte_idx < page.data.len() {
                        (page.data[byte_idx] >> bit_idx) & 1
                    } else {
                        0
                    };
                    data.push(if bit != 0 { 0x00 } else { 0xFF });
                }
            }
            Ok(DecodedImage {
                width: w,
                height: h,
                components: 1,
                bpc: 8,
                data,
                source_format: ImageFormat::Jbig2,
                color_space: ColorSpace::Unknown(Vec::new()),
                decode: None,
            })
        }
        Some(b"CCITTFaxDecode") | Some(b"CCF") => {
            // CCITT data is decoded by the stream decoder into 1-byte-per-pixel data
            // (black 0x00 / white 0xFF, reversed when /BlackIs1 is true). We pass it
            // through as 8bpc grayscale.
            check_size(info.width, info.height)?;
            let decoded = stream::decode_stream(raw_data, dict)?;
            Ok(DecodedImage {
                width: info.width,
                height: info.height,
                components: 1,
                bpc: 8,
                data: decoded,
                source_format: ImageFormat::CcittFax,
                color_space: ColorSpace::Unknown(Vec::new()),
                decode: None,
            })
        }
        _ => {
            check_size(info.width, info.height)?;
            let decoded = stream::decode_stream(raw_data, dict)?;
            Ok(DecodedImage {
                width: info.width,
                height: info.height,
                components: raw_components,
                bpc: info.bits_per_component,
                data: decoded,
                source_format: ImageFormat::Raw,
                color_space: ColorSpace::Unknown(Vec::new()),
                decode: None,
            })
        }
    };
    let mut image = decoded?;
    image.color_space = color_space;
    image.decode = decode;
    Ok(image)
}

/// The decoded `/JBIG2Globals` stream named by the `JBIG2Decode` filter's
/// `/DecodeParms`, or `None` when it names none.
fn jbig2_globals(dict: &PdfDict, doc: &PdfDocument) -> Result<Option<Vec<u8>>> {
    let index = stream::get_filters(dict)
        .iter()
        .rposition(|f| f.as_slice() == b"JBIG2Decode");
    let parms = index.and_then(|i| stream::get_decode_params(dict).into_iter().nth(i).flatten());
    let Some(globals) = parms.as_ref().and_then(|p| p.get(b"JBIG2Globals")) else {
        return Ok(None);
    };
    let unreadable = |detail: String| JustPdfError::StreamDecode {
        filter: "JBIG2Decode".into(),
        detail,
    };
    let PdfObject::Reference(r) = globals else {
        return Err(unreadable("/JBIG2Globals is not a stream reference".into()));
    };
    match doc.resolve(r) {
        Ok(PdfObject::Stream { dict, data }) => Ok(Some(stream::decode_stream(&data, &dict)?)),
        Ok(_) => Err(unreadable(format!(
            "/JBIG2Globals {} {} R is not a stream",
            r.obj_num, r.gen_num
        ))),
        Err(e) => Err(unreadable(format!(
            "/JBIG2Globals {} {} R: {e}",
            r.obj_num, r.gen_num
        ))),
    }
}

/// The JPEG bytes of a DCTDecode image stream: the filters before `DCTDecode`
/// applied, the JPEG itself not decoded.
pub fn extract_jpeg_bytes(raw_data: &[u8], dict: &PdfDict) -> Result<Vec<u8>> {
    let filters = stream::get_filters(dict);
    if matches!(
        filters.last().map(Vec::as_slice),
        Some(b"DCTDecode" | b"DCT")
    ) {
        stream::decode_stream(raw_data, dict)
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
