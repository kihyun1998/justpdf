use crate::object::{PdfDict, PdfObject};

/// Supported color space types.
#[derive(Debug, Clone, PartialEq)]
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
    /// ICC profile-based (not parsed, just stored as reference).
    ICCBased {
        num_components: u32,
    },
    /// Unknown/unsupported color space.
    Unknown(Vec<u8>),
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
            Self::ICCBased { num_components } => *num_components as usize,
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
}
