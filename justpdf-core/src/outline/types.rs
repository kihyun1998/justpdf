use crate::object::{IndirectRef, PdfObject};
use crate::annot::types::AnnotColor;

/// PDF destination (section 12.3.2 of PDF spec).
#[derive(Debug, Clone, PartialEq)]
pub enum Destination {
    /// [page /XYZ left top zoom]
    XYZ {
        page_ref: IndirectRef,
        left: Option<f64>,
        top: Option<f64>,
        zoom: Option<f64>,
    },
    /// [page /Fit]
    Fit { page_ref: IndirectRef },
    /// [page /FitH top]
    FitH {
        page_ref: IndirectRef,
        top: Option<f64>,
    },
    /// [page /FitV left]
    FitV {
        page_ref: IndirectRef,
        left: Option<f64>,
    },
    /// [page /FitR left bottom right top]
    FitR {
        page_ref: IndirectRef,
        left: f64,
        bottom: f64,
        right: f64,
        top: f64,
    },
    /// [page /FitB]
    FitB { page_ref: IndirectRef },
    /// [page /FitBH top]
    FitBH {
        page_ref: IndirectRef,
        top: Option<f64>,
    },
    /// [page /FitBV left]
    FitBV {
        page_ref: IndirectRef,
        left: Option<f64>,
    },
    /// A named destination (string key to be resolved via the Names tree).
    Named(String),
}

impl Destination {
    /// Parse a destination from a PdfObject (array or name/string for named dest).
    pub fn from_object(obj: &PdfObject) -> Option<Self> {
        match obj {
            PdfObject::Array(arr) => Self::from_array(arr),
            PdfObject::Name(n) => Some(Self::Named(String::from_utf8_lossy(n).into_owned())),
            PdfObject::String(s) => Some(Self::Named(String::from_utf8_lossy(s).into_owned())),
            _ => None,
        }
    }

    /// Parse from a dest array [pageRef /Type ...]
    pub fn from_array(arr: &[PdfObject]) -> Option<Self> {
        if arr.is_empty() {
            return None;
        }
        let page_ref = match &arr[0] {
            PdfObject::Reference(r) => r.clone(),
            PdfObject::Integer(n) => {
                // Page index (integer) - store as a ref with obj_num = page index.
                // The caller should resolve this to an actual page ref.
                IndirectRef {
                    obj_num: *n as u32,
                    gen_num: 0,
                }
            }
            _ => return None,
        };

        let fit_type = arr.get(1).and_then(|o| o.as_name()).unwrap_or(b"Fit");

        // Helper to get optional f64 (null means None).
        let opt_f64 = |idx: usize| -> Option<f64> {
            arr.get(idx).and_then(|o| match o {
                PdfObject::Null => None,
                other => other.as_f64(),
            })
        };

        match fit_type {
            b"XYZ" => Some(Self::XYZ {
                page_ref,
                left: opt_f64(2),
                top: opt_f64(3),
                zoom: opt_f64(4),
            }),
            b"Fit" => Some(Self::Fit { page_ref }),
            b"FitH" => Some(Self::FitH {
                page_ref,
                top: opt_f64(2),
            }),
            b"FitV" => Some(Self::FitV {
                page_ref,
                left: opt_f64(2),
            }),
            b"FitR" => Some(Self::FitR {
                page_ref,
                left: opt_f64(2).unwrap_or(0.0),
                bottom: opt_f64(3).unwrap_or(0.0),
                right: opt_f64(4).unwrap_or(0.0),
                top: opt_f64(5).unwrap_or(0.0),
            }),
            b"FitB" => Some(Self::FitB { page_ref }),
            b"FitBH" => Some(Self::FitBH {
                page_ref,
                top: opt_f64(2),
            }),
            b"FitBV" => Some(Self::FitBV {
                page_ref,
                left: opt_f64(2),
            }),
            _ => Some(Self::Fit { page_ref }),
        }
    }

    /// Convert to a PDF array representation.
    pub fn to_pdf_array(&self) -> PdfObject {
        let opt = |v: &Option<f64>| match v {
            Some(f) => PdfObject::Real(*f),
            None => PdfObject::Null,
        };
        match self {
            Self::XYZ {
                page_ref,
                left,
                top,
                zoom,
            } => PdfObject::Array(vec![
                PdfObject::Reference(page_ref.clone()),
                PdfObject::Name(b"XYZ".to_vec()),
                opt(left),
                opt(top),
                opt(zoom),
            ]),
            Self::Fit { page_ref } => PdfObject::Array(vec![
                PdfObject::Reference(page_ref.clone()),
                PdfObject::Name(b"Fit".to_vec()),
            ]),
            Self::FitH { page_ref, top } => PdfObject::Array(vec![
                PdfObject::Reference(page_ref.clone()),
                PdfObject::Name(b"FitH".to_vec()),
                opt(top),
            ]),
            Self::FitV { page_ref, left } => PdfObject::Array(vec![
                PdfObject::Reference(page_ref.clone()),
                PdfObject::Name(b"FitV".to_vec()),
                opt(left),
            ]),
            Self::FitR {
                page_ref,
                left,
                bottom,
                right,
                top,
            } => PdfObject::Array(vec![
                PdfObject::Reference(page_ref.clone()),
                PdfObject::Name(b"FitR".to_vec()),
                PdfObject::Real(*left),
                PdfObject::Real(*bottom),
                PdfObject::Real(*right),
                PdfObject::Real(*top),
            ]),
            Self::FitB { page_ref } => PdfObject::Array(vec![
                PdfObject::Reference(page_ref.clone()),
                PdfObject::Name(b"FitB".to_vec()),
            ]),
            Self::FitBH { page_ref, top } => PdfObject::Array(vec![
                PdfObject::Reference(page_ref.clone()),
                PdfObject::Name(b"FitBH".to_vec()),
                opt(top),
            ]),
            Self::FitBV { page_ref, left } => PdfObject::Array(vec![
                PdfObject::Reference(page_ref.clone()),
                PdfObject::Name(b"FitBV".to_vec()),
                opt(left),
            ]),
            Self::Named(name) => PdfObject::String(name.as_bytes().to_vec()),
        }
    }
}

/// Outline style flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OutlineStyle {
    pub italic: bool,
    pub bold: bool,
}

impl OutlineStyle {
    pub fn from_flags(flags: i64) -> Self {
        Self {
            italic: flags & 1 != 0,
            bold: flags & 2 != 0,
        }
    }

    pub fn to_flags(self) -> i64 {
        let mut f = 0i64;
        if self.italic {
            f |= 1;
        }
        if self.bold {
            f |= 2;
        }
        f
    }
}

/// A single item in the PDF outline (bookmark) tree.
#[derive(Debug, Clone)]
pub struct OutlineItem {
    pub title: String,
    pub dest: Option<Destination>,
    pub color: Option<AnnotColor>,
    pub style: OutlineStyle,
    pub is_open: bool,
    pub children: Vec<OutlineItem>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page_ref(n: u32) -> IndirectRef {
        IndirectRef {
            obj_num: n,
            gen_num: 0,
        }
    }

    #[test]
    fn test_destination_xyz_from_array() {
        let arr = vec![
            PdfObject::Reference(page_ref(5)),
            PdfObject::Name(b"XYZ".to_vec()),
            PdfObject::Real(100.0),
            PdfObject::Real(700.0),
            PdfObject::Real(1.5),
        ];
        let dest = Destination::from_array(&arr).unwrap();
        match dest {
            Destination::XYZ {
                page_ref: pr,
                left,
                top,
                zoom,
            } => {
                assert_eq!(pr.obj_num, 5);
                assert_eq!(left, Some(100.0));
                assert_eq!(top, Some(700.0));
                assert_eq!(zoom, Some(1.5));
            }
            _ => panic!("expected XYZ destination"),
        }
    }

    #[test]
    fn test_destination_xyz_with_nulls() {
        let arr = vec![
            PdfObject::Reference(page_ref(3)),
            PdfObject::Name(b"XYZ".to_vec()),
            PdfObject::Null,
            PdfObject::Null,
            PdfObject::Null,
        ];
        let dest = Destination::from_array(&arr).unwrap();
        match dest {
            Destination::XYZ {
                left, top, zoom, ..
            } => {
                assert_eq!(left, None);
                assert_eq!(top, None);
                assert_eq!(zoom, None);
            }
            _ => panic!("expected XYZ destination"),
        }
    }

    #[test]
    fn test_destination_fit_from_array() {
        let arr = vec![
            PdfObject::Reference(page_ref(1)),
            PdfObject::Name(b"Fit".to_vec()),
        ];
        let dest = Destination::from_array(&arr).unwrap();
        assert!(matches!(dest, Destination::Fit { .. }));
    }

    #[test]
    fn test_destination_fith_from_array() {
        let arr = vec![
            PdfObject::Reference(page_ref(2)),
            PdfObject::Name(b"FitH".to_vec()),
            PdfObject::Real(500.0),
        ];
        let dest = Destination::from_array(&arr).unwrap();
        match dest {
            Destination::FitH { top, .. } => assert_eq!(top, Some(500.0)),
            _ => panic!("expected FitH destination"),
        }
    }

    #[test]
    fn test_destination_fitv_from_array() {
        let arr = vec![
            PdfObject::Reference(page_ref(2)),
            PdfObject::Name(b"FitV".to_vec()),
            PdfObject::Real(72.0),
        ];
        let dest = Destination::from_array(&arr).unwrap();
        match dest {
            Destination::FitV { left, .. } => assert_eq!(left, Some(72.0)),
            _ => panic!("expected FitV destination"),
        }
    }

    #[test]
    fn test_destination_fitr_from_array() {
        let arr = vec![
            PdfObject::Reference(page_ref(4)),
            PdfObject::Name(b"FitR".to_vec()),
            PdfObject::Real(10.0),
            PdfObject::Real(20.0),
            PdfObject::Real(300.0),
            PdfObject::Real(400.0),
        ];
        let dest = Destination::from_array(&arr).unwrap();
        match dest {
            Destination::FitR {
                left,
                bottom,
                right,
                top,
                ..
            } => {
                assert_eq!(left, 10.0);
                assert_eq!(bottom, 20.0);
                assert_eq!(right, 300.0);
                assert_eq!(top, 400.0);
            }
            _ => panic!("expected FitR destination"),
        }
    }

    #[test]
    fn test_destination_fitb_variants() {
        let arr = vec![
            PdfObject::Reference(page_ref(1)),
            PdfObject::Name(b"FitB".to_vec()),
        ];
        assert!(matches!(
            Destination::from_array(&arr),
            Some(Destination::FitB { .. })
        ));

        let arr = vec![
            PdfObject::Reference(page_ref(1)),
            PdfObject::Name(b"FitBH".to_vec()),
            PdfObject::Real(200.0),
        ];
        match Destination::from_array(&arr).unwrap() {
            Destination::FitBH { top, .. } => assert_eq!(top, Some(200.0)),
            _ => panic!("expected FitBH"),
        }

        let arr = vec![
            PdfObject::Reference(page_ref(1)),
            PdfObject::Name(b"FitBV".to_vec()),
            PdfObject::Real(50.0),
        ];
        match Destination::from_array(&arr).unwrap() {
            Destination::FitBV { left, .. } => assert_eq!(left, Some(50.0)),
            _ => panic!("expected FitBV"),
        }
    }

    #[test]
    fn test_destination_named_from_name() {
        let obj = PdfObject::Name(b"chapter1".to_vec());
        let dest = Destination::from_object(&obj).unwrap();
        assert_eq!(dest, Destination::Named("chapter1".to_string()));
    }

    #[test]
    fn test_destination_named_from_string() {
        let obj = PdfObject::String(b"section2".to_vec());
        let dest = Destination::from_object(&obj).unwrap();
        assert_eq!(dest, Destination::Named("section2".to_string()));
    }

    #[test]
    fn test_destination_from_object_null() {
        assert!(Destination::from_object(&PdfObject::Null).is_none());
    }

    #[test]
    fn test_destination_empty_array() {
        assert!(Destination::from_array(&[]).is_none());
    }

    #[test]
    fn test_destination_page_index_integer() {
        let arr = vec![
            PdfObject::Integer(0),
            PdfObject::Name(b"Fit".to_vec()),
        ];
        let dest = Destination::from_array(&arr).unwrap();
        match dest {
            Destination::Fit { page_ref: pr } => assert_eq!(pr.obj_num, 0),
            _ => panic!("expected Fit destination"),
        }
    }

    #[test]
    fn test_destination_unknown_type_defaults_to_fit() {
        let arr = vec![
            PdfObject::Reference(page_ref(1)),
            PdfObject::Name(b"UnknownType".to_vec()),
        ];
        let dest = Destination::from_array(&arr).unwrap();
        assert!(matches!(dest, Destination::Fit { .. }));
    }

    #[test]
    fn test_destination_roundtrip_xyz() {
        let original = Destination::XYZ {
            page_ref: page_ref(5),
            left: Some(100.0),
            top: Some(700.0),
            zoom: None,
        };
        let pdf = original.to_pdf_array();
        let restored = Destination::from_object(&pdf).unwrap();
        assert_eq!(original, restored);
    }

    #[test]
    fn test_destination_roundtrip_fit() {
        let original = Destination::Fit {
            page_ref: page_ref(3),
        };
        let pdf = original.to_pdf_array();
        let restored = Destination::from_object(&pdf).unwrap();
        assert_eq!(original, restored);
    }

    #[test]
    fn test_destination_roundtrip_fitr() {
        let original = Destination::FitR {
            page_ref: page_ref(1),
            left: 10.0,
            bottom: 20.0,
            right: 300.0,
            top: 400.0,
        };
        let pdf = original.to_pdf_array();
        let restored = Destination::from_object(&pdf).unwrap();
        assert_eq!(original, restored);
    }

    #[test]
    fn test_destination_roundtrip_named() {
        let original = Destination::Named("chapter1".to_string());
        let pdf = original.to_pdf_array();
        let restored = Destination::from_object(&pdf).unwrap();
        assert_eq!(original, restored);
    }

    #[test]
    fn test_outline_style_from_flags() {
        assert_eq!(
            OutlineStyle::from_flags(0),
            OutlineStyle {
                italic: false,
                bold: false
            }
        );
        assert_eq!(
            OutlineStyle::from_flags(1),
            OutlineStyle {
                italic: true,
                bold: false
            }
        );
        assert_eq!(
            OutlineStyle::from_flags(2),
            OutlineStyle {
                italic: false,
                bold: true
            }
        );
        assert_eq!(
            OutlineStyle::from_flags(3),
            OutlineStyle {
                italic: true,
                bold: true
            }
        );
    }

    #[test]
    fn test_outline_style_roundtrip() {
        let style = OutlineStyle {
            italic: true,
            bold: true,
        };
        assert_eq!(style.to_flags(), 3);
        assert_eq!(OutlineStyle::from_flags(style.to_flags()), style);
    }

    #[test]
    fn test_outline_style_default() {
        let style = OutlineStyle::default();
        assert!(!style.italic);
        assert!(!style.bold);
        assert_eq!(style.to_flags(), 0);
    }
}
