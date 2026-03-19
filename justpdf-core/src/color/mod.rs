pub mod icc;

use crate::error::{JustPdfError, Result};
use crate::object::{IndirectRef, PdfDict, PdfObject};
use crate::parser::PdfDocument;

pub use self::icc::IccProfile;

/// Supported color space types.
#[derive(Debug, Clone)]
pub enum ColorSpace {
    DeviceGray,
    DeviceRGB,
    DeviceCMYK,
    CalGray {
        white_point: [f64; 3],
        black_point: [f64; 3],
        gamma: f64,
    },
    CalRGB {
        white_point: [f64; 3],
        black_point: [f64; 3],
        gamma: [f64; 3],
        matrix: [f64; 9],
    },
    Lab {
        white_point: [f64; 3],
        black_point: [f64; 3],
        range: [f64; 4],
    },
    Indexed {
        base: Box<ColorSpace>,
        hival: u32,
        lookup: Vec<u8>,
    },
    Separation {
        name: Vec<u8>,
        alternate: Box<ColorSpace>,
    },
    DeviceN {
        names: Vec<Vec<u8>>,
        alternate: Box<ColorSpace>,
    },
    /// ICC profile-based color space.
    ///
    /// When the raw ICC profile stream data is available it is parsed into an
    /// [`IccProfile`] which enables accurate color transformation to sRGB.
    ICCBased {
        num_components: u32,
        /// Parsed ICC profile (populated when profile data is available).
        profile: Option<Box<IccProfile>>,
    },
    /// Unknown/unsupported color space.
    Unknown(Vec<u8>),
}

impl PartialEq for ColorSpace {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::DeviceGray, Self::DeviceGray) => true,
            (Self::DeviceRGB, Self::DeviceRGB) => true,
            (Self::DeviceCMYK, Self::DeviceCMYK) => true,
            (Self::CalGray { white_point: w1, black_point: b1, gamma: g1 },
             Self::CalGray { white_point: w2, black_point: b2, gamma: g2 }) => w1 == w2 && b1 == b2 && g1 == g2,
            (Self::CalRGB { white_point: w1, black_point: b1, gamma: g1, matrix: m1 },
             Self::CalRGB { white_point: w2, black_point: b2, gamma: g2, matrix: m2 }) => w1 == w2 && b1 == b2 && g1 == g2 && m1 == m2,
            (Self::Lab { white_point: w1, black_point: b1, range: r1 },
             Self::Lab { white_point: w2, black_point: b2, range: r2 }) => w1 == w2 && b1 == b2 && r1 == r2,
            (Self::Indexed { base: b1, hival: h1, lookup: l1 },
             Self::Indexed { base: b2, hival: h2, lookup: l2 }) => b1 == b2 && h1 == h2 && l1 == l2,
            (Self::Separation { name: n1, alternate: a1 },
             Self::Separation { name: n2, alternate: a2 }) => n1 == n2 && a1 == a2,
            (Self::DeviceN { names: n1, alternate: a1 },
             Self::DeviceN { names: n2, alternate: a2 }) => n1 == n2 && a1 == a2,
            (Self::ICCBased { num_components: n1, .. },
             Self::ICCBased { num_components: n2, .. }) => n1 == n2,
            (Self::Unknown(a), Self::Unknown(b)) => a == b,
            _ => false,
        }
    }
}

impl ColorSpace {
    /// Number of components in this color space.
    pub fn num_components(&self) -> usize {
        match self {
            Self::DeviceGray | Self::CalGray { .. } => 1,
            Self::DeviceRGB | Self::CalRGB { .. } | Self::Lab { .. } => 3,
            Self::DeviceCMYK => 4,
            Self::Indexed { .. } => 1,
            Self::Separation { .. } => 1,
            Self::DeviceN { names, .. } => names.len(),
            Self::ICCBased { num_components, .. } => *num_components as usize,
            Self::Unknown(_) => 0,
        }
    }

    /// Parse a color space from a PDF name or array.
    pub fn from_pdf_object(obj: &PdfObject) -> Self {
        match obj {
            PdfObject::Name(name) => Self::from_name(name),
            PdfObject::Array(arr) if !arr.is_empty() => Self::from_array(arr),
            _ => Self::Unknown(b"invalid".to_vec()),
        }
    }

    fn from_name(name: &[u8]) -> Self {
        match name {
            b"DeviceGray" | b"G" => Self::DeviceGray,
            b"DeviceRGB" | b"RGB" => Self::DeviceRGB,
            b"DeviceCMYK" | b"CMYK" => Self::DeviceCMYK,
            _ => Self::Unknown(name.to_vec()),
        }
    }

    fn from_array(arr: &[PdfObject]) -> Self {
        let name = match &arr[0] {
            PdfObject::Name(n) => n.as_slice(),
            _ => return Self::Unknown(b"invalid-array".to_vec()),
        };

        match name {
            b"CalGray" => Self::parse_cal_gray(arr),
            b"CalRGB" => Self::parse_cal_rgb(arr),
            b"Lab" => Self::parse_lab(arr),
            b"Indexed" | b"I" => Self::parse_indexed(arr),
            b"Separation" => Self::parse_separation(arr),
            b"DeviceN" => Self::parse_device_n(arr),
            b"ICCBased" => Self::ICCBased {
                num_components: 3, // default, actual value requires reading the stream
                profile: None,
            },
            b"DeviceGray" | b"G" => Self::DeviceGray,
            b"DeviceRGB" | b"RGB" => Self::DeviceRGB,
            b"DeviceCMYK" | b"CMYK" => Self::DeviceCMYK,
            _ => Self::Unknown(name.to_vec()),
        }
    }

    fn parse_cal_gray(arr: &[PdfObject]) -> Self {
        let dict = arr.get(1).and_then(|o| o.as_dict());
        let (wp, bp, gamma) = parse_cal_params(dict);
        Self::CalGray {
            white_point: wp,
            black_point: bp,
            gamma: gamma[0],
        }
    }

    fn parse_cal_rgb(arr: &[PdfObject]) -> Self {
        let dict = arr.get(1).and_then(|o| o.as_dict());
        let (wp, bp, gamma) = parse_cal_params(dict);
        let matrix = dict
            .and_then(|d| d.get_array(b"Matrix"))
            .map(parse_f64_array_9)
            .unwrap_or([1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]);
        Self::CalRGB {
            white_point: wp,
            black_point: bp,
            gamma,
            matrix,
        }
    }

    fn parse_lab(arr: &[PdfObject]) -> Self {
        let dict = arr.get(1).and_then(|o| o.as_dict());
        let (wp, bp, _) = parse_cal_params(dict);
        let range = dict
            .and_then(|d| d.get_array(b"Range"))
            .map(parse_f64_array_4)
            .unwrap_or([-100.0, 100.0, -100.0, 100.0]);
        Self::Lab {
            white_point: wp,
            black_point: bp,
            range,
        }
    }

    fn parse_indexed(arr: &[PdfObject]) -> Self {
        let base = arr
            .get(1)
            .map(ColorSpace::from_pdf_object)
            .unwrap_or(Self::DeviceRGB);
        let hival = arr.get(2).and_then(|o| o.as_i64()).unwrap_or(255) as u32;
        let lookup = match arr.get(3) {
            Some(PdfObject::String(s)) => s.clone(),
            _ => Vec::new(),
        };
        Self::Indexed {
            base: Box::new(base),
            hival,
            lookup,
        }
    }

    fn parse_separation(arr: &[PdfObject]) -> Self {
        let name = match arr.get(1) {
            Some(PdfObject::Name(n)) => n.clone(),
            _ => b"Unknown".to_vec(),
        };
        let alternate = arr
            .get(2)
            .map(ColorSpace::from_pdf_object)
            .unwrap_or(Self::DeviceGray);
        Self::Separation {
            name,
            alternate: Box::new(alternate),
        }
    }

    fn parse_device_n(arr: &[PdfObject]) -> Self {
        let names = match arr.get(1) {
            Some(PdfObject::Array(a)) => a
                .iter()
                .filter_map(|o| match o {
                    PdfObject::Name(n) => Some(n.clone()),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        };
        let alternate = arr
            .get(2)
            .map(ColorSpace::from_pdf_object)
            .unwrap_or(Self::DeviceCMYK);
        Self::DeviceN {
            names,
            alternate: Box::new(alternate),
        }
    }
}

/// A color value with components.
#[derive(Debug, Clone, PartialEq)]
pub struct Color {
    pub components: Vec<f64>,
}

impl Color {
    pub fn gray(v: f64) -> Self {
        Self {
            components: vec![v],
        }
    }

    pub fn rgb(r: f64, g: f64, b: f64) -> Self {
        Self {
            components: vec![r, g, b],
        }
    }

    pub fn cmyk(c: f64, m: f64, y: f64, k: f64) -> Self {
        Self {
            components: vec![c, m, y, k],
        }
    }

    /// Convert to RGB (approximate).
    pub fn to_rgb(&self, cs: &ColorSpace) -> [f64; 3] {
        match cs {
            ColorSpace::DeviceGray | ColorSpace::CalGray { .. } => {
                let g = self.components.first().copied().unwrap_or(0.0);
                [g, g, g]
            }
            ColorSpace::DeviceRGB | ColorSpace::CalRGB { .. } => {
                let r = self.components.first().copied().unwrap_or(0.0);
                let g = self.components.get(1).copied().unwrap_or(0.0);
                let b = self.components.get(2).copied().unwrap_or(0.0);
                [r, g, b]
            }
            ColorSpace::DeviceCMYK => {
                let c = self.components.first().copied().unwrap_or(0.0);
                let m = self.components.get(1).copied().unwrap_or(0.0);
                let y = self.components.get(2).copied().unwrap_or(0.0);
                let k = self.components.get(3).copied().unwrap_or(0.0);
                cmyk_to_rgb(c, m, y, k)
            }
            ColorSpace::ICCBased {
                profile: Some(p), ..
            } => icc::icc_to_srgb(p, &self.components),
            _ => [0.0, 0.0, 0.0],
        }
    }
}

/// Simple CMYK → RGB conversion.
pub fn cmyk_to_rgb(c: f64, m: f64, y: f64, k: f64) -> [f64; 3] {
    let r = (1.0 - c) * (1.0 - k);
    let g = (1.0 - m) * (1.0 - k);
    let b = (1.0 - y) * (1.0 - k);
    [r, g, b]
}

/// Simple RGB → CMYK conversion.
pub fn rgb_to_cmyk(r: f64, g: f64, b: f64) -> [f64; 4] {
    let k = 1.0 - r.max(g).max(b);
    if k >= 1.0 {
        return [0.0, 0.0, 0.0, 1.0];
    }
    let c = (1.0 - r - k) / (1.0 - k);
    let m = (1.0 - g - k) / (1.0 - k);
    let y = (1.0 - b - k) / (1.0 - k);
    [c, m, y, k]
}

// --- Output Intent (section 14.11.5) ---

/// PDF Output Intent (section 14.11.5).
///
/// Output intents describe the intended reproduction conditions for a PDF
/// document, commonly used in PDF/X and PDF/A workflows.
#[derive(Debug, Clone)]
pub struct OutputIntent {
    /// Subtype: e.g. "GTS_PDFX", "GTS_PDFA1", "ISO_PDFE1"
    pub subtype: String,
    /// Output condition identifier (e.g. "CGATS TR 001")
    pub output_condition_identifier: String,
    /// Human-readable output condition
    pub output_condition: Option<String>,
    /// Registry name
    pub registry_name: Option<String>,
    /// Info string
    pub info: Option<String>,
    /// Reference to the ICC profile stream (DestOutputProfile)
    pub dest_output_profile_ref: Option<IndirectRef>,
}

/// Helper to extract a UTF-8 string from a PdfDict entry (name or string).
fn dict_text(dict: &PdfDict, key: &[u8]) -> Option<String> {
    match dict.get(key) {
        Some(PdfObject::Name(v)) => Some(String::from_utf8_lossy(v).into_owned()),
        Some(PdfObject::String(v)) => Some(String::from_utf8_lossy(v).into_owned()),
        _ => None,
    }
}

/// Parse a single output intent dictionary into an `OutputIntent`.
fn parse_output_intent_dict(dict: &PdfDict) -> OutputIntent {
    let subtype = dict_text(dict, b"S").unwrap_or_default();
    let output_condition_identifier =
        dict_text(dict, b"OutputConditionIdentifier").unwrap_or_default();
    let output_condition = dict_text(dict, b"OutputCondition");
    let registry_name = dict_text(dict, b"RegistryName");
    let info = dict_text(dict, b"Info");
    let dest_output_profile_ref = dict.get_ref(b"DestOutputProfile").cloned();

    OutputIntent {
        subtype,
        output_condition_identifier,
        output_condition,
        registry_name,
        info,
        dest_output_profile_ref,
    }
}

/// Read output intents from the document catalog.
///
/// Parses the `/OutputIntents` array from the catalog dictionary.  Each entry
/// is a dictionary with keys `/S`, `/OutputConditionIdentifier`,
/// `/OutputCondition`, `/RegistryName`, `/Info`, and `/DestOutputProfile`.
pub fn read_output_intents(doc: &PdfDocument) -> Result<Vec<OutputIntent>> {
    let catalog_ref = match doc.catalog_ref() {
        Some(r) => r.clone(),
        None => {
            return Err(JustPdfError::InvalidObject {
                offset: 0,
                detail: "missing catalog reference".into(),
            });
        }
    };

    let catalog = doc.resolve(&catalog_ref)?;
    let catalog_dict = match &catalog {
        PdfObject::Dict(d) => d,
        _ => {
            return Err(JustPdfError::InvalidObject {
                offset: 0,
                detail: "catalog is not a dictionary".into(),
            });
        }
    };

    // Get /OutputIntents — may be an inline array or an indirect reference.
    let intents_obj = match catalog_dict.get(b"OutputIntents") {
        Some(obj) => obj.clone(),
        None => return Ok(Vec::new()),
    };

    // Resolve if indirect reference
    let intents_obj = match intents_obj {
        PdfObject::Reference(r) => doc.resolve(&r)?,
        other => other,
    };

    let arr = match &intents_obj {
        PdfObject::Array(a) => a,
        _ => return Ok(Vec::new()),
    };

    let mut result = Vec::with_capacity(arr.len());
    for item in arr {
        let dict_obj = match item {
            PdfObject::Reference(r) => doc.resolve(r)?,
            PdfObject::Dict(_) => item.clone(),
            _ => continue,
        };
        if let PdfObject::Dict(d) = &dict_obj {
            result.push(parse_output_intent_dict(d));
        }
    }

    Ok(result)
}

// --- Overprint (section 8.6.7) ---

/// Overprint mode settings from ExtGState.
///
/// Overprint controls how colours are composited when painting on a page that
/// already has marks from a previous painting operation.  These settings come
/// from the graphics-state parameters `/OP`, `/op`, and `/OPM`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OverprintState {
    /// Overprint for stroking operations (`/OP`)
    pub stroke: bool,
    /// Overprint for non-stroking operations (`/op`)
    pub fill: bool,
    /// Overprint mode (`/OPM`): 0 = set all components, 1 = nonzero components only
    pub mode: u32,
}

impl Default for OverprintState {
    fn default() -> Self {
        Self {
            stroke: false,
            fill: false,
            mode: 0,
        }
    }
}

/// Parse overprint settings from an ExtGState dictionary.
///
/// Reads `/OP` (stroke overprint), `/op` (fill overprint, defaults to `/OP`
/// if absent per the PDF spec), and `/OPM` (overprint mode).
pub fn parse_overprint(gs_dict: &PdfDict) -> OverprintState {
    let stroke = gs_dict.get_bool(b"OP").unwrap_or(false);
    // Per the spec, /op defaults to the value of /OP when absent.
    let fill = gs_dict.get_bool(b"op").unwrap_or(stroke);
    let mode = gs_dict
        .get_i64(b"OPM")
        .map(|v| v as u32)
        .unwrap_or(0);

    OverprintState {
        stroke,
        fill,
        mode,
    }
}

// --- Helpers ---

fn parse_cal_params(dict: Option<&PdfDict>) -> ([f64; 3], [f64; 3], [f64; 3]) {
    let dict = match dict {
        Some(d) => d,
        None => return ([0.9505, 1.0, 1.089], [0.0; 3], [1.0; 3]),
    };

    let wp = dict
        .get_array(b"WhitePoint")
        .map(parse_f64_array_3)
        .unwrap_or([0.9505, 1.0, 1.089]);
    let bp = dict
        .get_array(b"BlackPoint")
        .map(parse_f64_array_3)
        .unwrap_or([0.0; 3]);
    let gamma = dict
        .get_array(b"Gamma")
        .map(parse_f64_array_3)
        .unwrap_or([1.0; 3]);

    (wp, bp, gamma)
}

fn parse_f64_array_3(arr: &[PdfObject]) -> [f64; 3] {
    [
        arr.first().and_then(|o| o.as_f64()).unwrap_or(0.0),
        arr.get(1).and_then(|o| o.as_f64()).unwrap_or(0.0),
        arr.get(2).and_then(|o| o.as_f64()).unwrap_or(0.0),
    ]
}

fn parse_f64_array_4(arr: &[PdfObject]) -> [f64; 4] {
    [
        arr.first().and_then(|o| o.as_f64()).unwrap_or(0.0),
        arr.get(1).and_then(|o| o.as_f64()).unwrap_or(0.0),
        arr.get(2).and_then(|o| o.as_f64()).unwrap_or(0.0),
        arr.get(3).and_then(|o| o.as_f64()).unwrap_or(0.0),
    ]
}

fn parse_f64_array_9(arr: &[PdfObject]) -> [f64; 9] {
    let mut result = [0.0; 9];
    for (i, item) in arr.iter().enumerate().take(9) {
        result[i] = item.as_f64().unwrap_or(0.0);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_color_spaces() {
        assert_eq!(ColorSpace::DeviceGray.num_components(), 1);
        assert_eq!(ColorSpace::DeviceRGB.num_components(), 3);
        assert_eq!(ColorSpace::DeviceCMYK.num_components(), 4);
    }

    #[test]
    fn test_from_name() {
        let cs = ColorSpace::from_pdf_object(&PdfObject::Name(b"DeviceRGB".to_vec()));
        assert_eq!(cs, ColorSpace::DeviceRGB);
    }

    #[test]
    fn test_cmyk_to_rgb() {
        let [r, g, b] = cmyk_to_rgb(0.0, 0.0, 0.0, 0.0);
        assert!((r - 1.0).abs() < 0.001);
        assert!((g - 1.0).abs() < 0.001);
        assert!((b - 1.0).abs() < 0.001);

        let [r, g, b] = cmyk_to_rgb(0.0, 0.0, 0.0, 1.0);
        assert!((r).abs() < 0.001);
        assert!((g).abs() < 0.001);
        assert!((b).abs() < 0.001);
    }

    #[test]
    fn test_rgb_to_cmyk_roundtrip() {
        let [c, m, y, k] = rgb_to_cmyk(0.8, 0.3, 0.5);
        let [r, g, b] = cmyk_to_rgb(c, m, y, k);
        assert!((r - 0.8).abs() < 0.01);
        assert!((g - 0.3).abs() < 0.01);
        assert!((b - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_indexed() {
        let arr = vec![
            PdfObject::Name(b"Indexed".to_vec()),
            PdfObject::Name(b"DeviceRGB".to_vec()),
            PdfObject::Integer(255),
            PdfObject::String(vec![0; 768]),
        ];
        let cs = ColorSpace::from_pdf_object(&PdfObject::Array(arr));
        assert_eq!(cs.num_components(), 1);
        match cs {
            ColorSpace::Indexed { base, hival, .. } => {
                assert_eq!(*base, ColorSpace::DeviceRGB);
                assert_eq!(hival, 255);
            }
            _ => panic!("expected Indexed"),
        }
    }

    #[test]
    fn test_color_to_rgb() {
        let gray = Color::gray(0.5);
        let [r, g, b] = gray.to_rgb(&ColorSpace::DeviceGray);
        assert!((r - 0.5).abs() < 0.001);
        assert!((g - 0.5).abs() < 0.001);
        assert!((b - 0.5).abs() < 0.001);
    }

    // --- Output Intent tests ---

    #[test]
    fn test_parse_output_intent_from_dict() {
        let mut dict = PdfDict::new();
        dict.insert(b"Type".to_vec(), PdfObject::Name(b"OutputIntent".to_vec()));
        dict.insert(b"S".to_vec(), PdfObject::Name(b"GTS_PDFX".to_vec()));
        dict.insert(
            b"OutputConditionIdentifier".to_vec(),
            PdfObject::String(b"CGATS TR 001".to_vec()),
        );
        dict.insert(
            b"OutputCondition".to_vec(),
            PdfObject::String(b"SWOP (Publication)".to_vec()),
        );
        dict.insert(
            b"RegistryName".to_vec(),
            PdfObject::String(b"http://www.color.org".to_vec()),
        );
        dict.insert(
            b"Info".to_vec(),
            PdfObject::String(b"U.S. Web Coated".to_vec()),
        );
        dict.insert(
            b"DestOutputProfile".to_vec(),
            PdfObject::Reference(IndirectRef {
                obj_num: 42,
                gen_num: 0,
            }),
        );

        let oi = parse_output_intent_dict(&dict);
        assert_eq!(oi.subtype, "GTS_PDFX");
        assert_eq!(oi.output_condition_identifier, "CGATS TR 001");
        assert_eq!(oi.output_condition.as_deref(), Some("SWOP (Publication)"));
        assert_eq!(
            oi.registry_name.as_deref(),
            Some("http://www.color.org")
        );
        assert_eq!(oi.info.as_deref(), Some("U.S. Web Coated"));
        assert_eq!(
            oi.dest_output_profile_ref,
            Some(IndirectRef {
                obj_num: 42,
                gen_num: 0
            })
        );
    }

    #[test]
    fn test_parse_output_intent_minimal() {
        let mut dict = PdfDict::new();
        dict.insert(b"S".to_vec(), PdfObject::Name(b"GTS_PDFA1".to_vec()));
        dict.insert(
            b"OutputConditionIdentifier".to_vec(),
            PdfObject::String(b"sRGB".to_vec()),
        );

        let oi = parse_output_intent_dict(&dict);
        assert_eq!(oi.subtype, "GTS_PDFA1");
        assert_eq!(oi.output_condition_identifier, "sRGB");
        assert!(oi.output_condition.is_none());
        assert!(oi.registry_name.is_none());
        assert!(oi.info.is_none());
        assert!(oi.dest_output_profile_ref.is_none());
    }

    #[test]
    fn test_parse_output_intent_empty_dict() {
        let dict = PdfDict::new();
        let oi = parse_output_intent_dict(&dict);
        assert_eq!(oi.subtype, "");
        assert_eq!(oi.output_condition_identifier, "");
        assert!(oi.output_condition.is_none());
        assert!(oi.registry_name.is_none());
        assert!(oi.info.is_none());
        assert!(oi.dest_output_profile_ref.is_none());
    }

    #[test]
    fn test_parse_multiple_output_intents() {
        let mut dict1 = PdfDict::new();
        dict1.insert(b"S".to_vec(), PdfObject::Name(b"GTS_PDFX".to_vec()));
        dict1.insert(
            b"OutputConditionIdentifier".to_vec(),
            PdfObject::String(b"FOGRA39".to_vec()),
        );

        let mut dict2 = PdfDict::new();
        dict2.insert(b"S".to_vec(), PdfObject::Name(b"GTS_PDFA1".to_vec()));
        dict2.insert(
            b"OutputConditionIdentifier".to_vec(),
            PdfObject::String(b"sRGB IEC61966-2.1".to_vec()),
        );

        let arr = vec![PdfObject::Dict(dict1), PdfObject::Dict(dict2)];

        let intents: Vec<OutputIntent> = arr
            .iter()
            .filter_map(|obj| {
                if let PdfObject::Dict(d) = obj {
                    Some(parse_output_intent_dict(d))
                } else {
                    None
                }
            })
            .collect();

        assert_eq!(intents.len(), 2);
        assert_eq!(intents[0].subtype, "GTS_PDFX");
        assert_eq!(intents[0].output_condition_identifier, "FOGRA39");
        assert_eq!(intents[1].subtype, "GTS_PDFA1");
        assert_eq!(intents[1].output_condition_identifier, "sRGB IEC61966-2.1");
    }

    // --- Overprint tests ---

    #[test]
    fn test_overprint_default() {
        let state = OverprintState::default();
        assert!(!state.stroke);
        assert!(!state.fill);
        assert_eq!(state.mode, 0);
    }

    #[test]
    fn test_parse_overprint_empty_dict() {
        let dict = PdfDict::new();
        let state = parse_overprint(&dict);
        assert_eq!(state, OverprintState::default());
    }

    #[test]
    fn test_parse_overprint_stroke_only() {
        let mut dict = PdfDict::new();
        dict.insert(b"OP".to_vec(), PdfObject::Bool(true));

        let state = parse_overprint(&dict);
        assert!(state.stroke);
        // /op defaults to /OP when absent
        assert!(state.fill);
        assert_eq!(state.mode, 0);
    }

    #[test]
    fn test_parse_overprint_stroke_and_fill_separate() {
        let mut dict = PdfDict::new();
        dict.insert(b"OP".to_vec(), PdfObject::Bool(true));
        dict.insert(b"op".to_vec(), PdfObject::Bool(false));

        let state = parse_overprint(&dict);
        assert!(state.stroke);
        assert!(!state.fill);
        assert_eq!(state.mode, 0);
    }

    #[test]
    fn test_parse_overprint_mode_0() {
        let mut dict = PdfDict::new();
        dict.insert(b"OP".to_vec(), PdfObject::Bool(true));
        dict.insert(b"op".to_vec(), PdfObject::Bool(true));
        dict.insert(b"OPM".to_vec(), PdfObject::Integer(0));

        let state = parse_overprint(&dict);
        assert!(state.stroke);
        assert!(state.fill);
        assert_eq!(state.mode, 0);
    }

    #[test]
    fn test_parse_overprint_mode_1() {
        let mut dict = PdfDict::new();
        dict.insert(b"OP".to_vec(), PdfObject::Bool(true));
        dict.insert(b"op".to_vec(), PdfObject::Bool(true));
        dict.insert(b"OPM".to_vec(), PdfObject::Integer(1));

        let state = parse_overprint(&dict);
        assert!(state.stroke);
        assert!(state.fill);
        assert_eq!(state.mode, 1);
    }

    #[test]
    fn test_overprint_state_equality() {
        let a = OverprintState {
            stroke: true,
            fill: false,
            mode: 1,
        };
        let b = OverprintState {
            stroke: true,
            fill: false,
            mode: 1,
        };
        let c = OverprintState {
            stroke: true,
            fill: true,
            mode: 1,
        };
        assert_eq!(a, b);
        assert_ne!(a, c);
    }
}
