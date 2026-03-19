use crate::object::{IndirectRef, PdfObject};
use crate::page::Rect;

/// PDF annotation subtype (28 types per PDF spec + Unknown).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnnotationType {
    Text,
    Link,
    FreeText,
    Line,
    Square,
    Circle,
    Polygon,
    PolyLine,
    Highlight,
    Underline,
    Squiggly,
    StrikeOut,
    Stamp,
    Caret,
    Ink,
    Popup,
    FileAttachment,
    Sound,
    Movie,
    Widget,
    Screen,
    PrinterMark,
    TrapNet,
    Watermark,
    ThreeD,
    Redact,
    RichMedia,
    Unknown(Vec<u8>),
}

impl AnnotationType {
    pub fn from_name(name: &[u8]) -> Self {
        match name {
            b"Text" => Self::Text,
            b"Link" => Self::Link,
            b"FreeText" => Self::FreeText,
            b"Line" => Self::Line,
            b"Square" => Self::Square,
            b"Circle" => Self::Circle,
            b"Polygon" => Self::Polygon,
            b"PolyLine" => Self::PolyLine,
            b"Highlight" => Self::Highlight,
            b"Underline" => Self::Underline,
            b"Squiggly" => Self::Squiggly,
            b"StrikeOut" => Self::StrikeOut,
            b"Stamp" => Self::Stamp,
            b"Caret" => Self::Caret,
            b"Ink" => Self::Ink,
            b"Popup" => Self::Popup,
            b"FileAttachment" => Self::FileAttachment,
            b"Sound" => Self::Sound,
            b"Movie" => Self::Movie,
            b"Widget" => Self::Widget,
            b"Screen" => Self::Screen,
            b"PrinterMark" => Self::PrinterMark,
            b"TrapNet" => Self::TrapNet,
            b"Watermark" => Self::Watermark,
            b"3D" => Self::ThreeD,
            b"Redact" => Self::Redact,
            b"RichMedia" => Self::RichMedia,
            _ => Self::Unknown(name.to_vec()),
        }
    }

    pub fn to_name(&self) -> &[u8] {
        match self {
            Self::Text => b"Text",
            Self::Link => b"Link",
            Self::FreeText => b"FreeText",
            Self::Line => b"Line",
            Self::Square => b"Square",
            Self::Circle => b"Circle",
            Self::Polygon => b"Polygon",
            Self::PolyLine => b"PolyLine",
            Self::Highlight => b"Highlight",
            Self::Underline => b"Underline",
            Self::Squiggly => b"Squiggly",
            Self::StrikeOut => b"StrikeOut",
            Self::Stamp => b"Stamp",
            Self::Caret => b"Caret",
            Self::Ink => b"Ink",
            Self::Popup => b"Popup",
            Self::FileAttachment => b"FileAttachment",
            Self::Sound => b"Sound",
            Self::Movie => b"Movie",
            Self::Widget => b"Widget",
            Self::Screen => b"Screen",
            Self::PrinterMark => b"PrinterMark",
            Self::TrapNet => b"TrapNet",
            Self::Watermark => b"Watermark",
            Self::ThreeD => b"3D",
            Self::Redact => b"Redact",
            Self::RichMedia => b"RichMedia",
            Self::Unknown(n) => n,
        }
    }
}

/// Annotation flags (bitfield from /F entry).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AnnotationFlags(pub u32);

impl AnnotationFlags {
    pub const INVISIBLE: u32 = 1;
    pub const HIDDEN: u32 = 1 << 1;
    pub const PRINT: u32 = 1 << 2;
    pub const NO_ZOOM: u32 = 1 << 3;
    pub const NO_ROTATE: u32 = 1 << 4;
    pub const NO_VIEW: u32 = 1 << 5;
    pub const READ_ONLY: u32 = 1 << 6;
    pub const LOCKED: u32 = 1 << 7;
    pub const TOGGLE_NO_VIEW: u32 = 1 << 8;
    pub const LOCKED_CONTENTS: u32 = 1 << 9;

    pub fn has(self, flag: u32) -> bool {
        self.0 & flag != 0
    }
}

/// Border style for annotations.
#[derive(Debug, Clone, PartialEq)]
pub struct BorderStyle {
    pub width: f64,
    pub style: BorderStyleType,
    pub dash_pattern: Vec<f64>,
}

impl Default for BorderStyle {
    fn default() -> Self {
        Self {
            width: 1.0,
            style: BorderStyleType::Solid,
            dash_pattern: Vec::new(),
        }
    }
}

/// Border style type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorderStyleType {
    Solid,
    Dashed,
    Beveled,
    Inset,
    Underline,
}

impl BorderStyleType {
    pub fn from_name(name: &[u8]) -> Self {
        match name {
            b"S" => Self::Solid,
            b"D" => Self::Dashed,
            b"B" => Self::Beveled,
            b"I" => Self::Inset,
            b"U" => Self::Underline,
            _ => Self::Solid,
        }
    }

    pub fn to_name(self) -> &'static [u8] {
        match self {
            Self::Solid => b"S",
            Self::Dashed => b"D",
            Self::Beveled => b"B",
            Self::Inset => b"I",
            Self::Underline => b"U",
        }
    }
}

/// Annotation color.
#[derive(Debug, Clone, PartialEq)]
pub enum AnnotColor {
    Gray(f64),
    Rgb(f64, f64, f64),
    Cmyk(f64, f64, f64, f64),
}

impl AnnotColor {
    pub fn from_array(arr: &[PdfObject]) -> Option<Self> {
        match arr.len() {
            0 => None,
            1 => Some(Self::Gray(arr[0].as_f64()?)),
            3 => Some(Self::Rgb(
                arr[0].as_f64()?,
                arr[1].as_f64()?,
                arr[2].as_f64()?,
            )),
            4 => Some(Self::Cmyk(
                arr[0].as_f64()?,
                arr[1].as_f64()?,
                arr[2].as_f64()?,
                arr[3].as_f64()?,
            )),
            _ => None,
        }
    }

    pub fn to_pdf_array(&self) -> Vec<PdfObject> {
        match self {
            Self::Gray(g) => vec![PdfObject::Real(*g)],
            Self::Rgb(r, g, b) => vec![
                PdfObject::Real(*r),
                PdfObject::Real(*g),
                PdfObject::Real(*b),
            ],
            Self::Cmyk(c, m, y, k) => vec![
                PdfObject::Real(*c),
                PdfObject::Real(*m),
                PdfObject::Real(*y),
                PdfObject::Real(*k),
            ],
        }
    }
}

/// Line ending style.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineEndingStyle {
    None,
    Square,
    Circle,
    Diamond,
    OpenArrow,
    ClosedArrow,
    Butt,
    RArrow,
    Slash,
}

impl LineEndingStyle {
    pub fn from_name(name: &[u8]) -> Self {
        match name {
            b"Square" => Self::Square,
            b"Circle" => Self::Circle,
            b"Diamond" => Self::Diamond,
            b"OpenArrow" => Self::OpenArrow,
            b"ClosedArrow" => Self::ClosedArrow,
            b"Butt" => Self::Butt,
            b"RArrow" => Self::RArrow,
            b"Slash" => Self::Slash,
            _ => Self::None,
        }
    }

    pub fn to_name(self) -> &'static [u8] {
        match self {
            Self::None => b"None",
            Self::Square => b"Square",
            Self::Circle => b"Circle",
            Self::Diamond => b"Diamond",
            Self::OpenArrow => b"OpenArrow",
            Self::ClosedArrow => b"ClosedArrow",
            Self::Butt => b"Butt",
            Self::RArrow => b"RArrow",
            Self::Slash => b"Slash",
        }
    }
}

/// A parsed PDF annotation.
#[derive(Debug, Clone)]
pub struct Annotation {
    pub annot_type: AnnotationType,
    pub rect: Rect,
    pub contents: Option<String>,
    pub name: Option<String>,
    pub modified: Option<String>,
    pub flags: AnnotationFlags,
    pub color: Option<AnnotColor>,
    pub border: Option<BorderStyle>,
    pub appearance_ref: Option<IndirectRef>,
    pub popup_ref: Option<IndirectRef>,
    pub obj_ref: Option<IndirectRef>,
    pub data: AnnotationData,
}

/// Type-specific annotation data.
#[derive(Debug, Clone)]
pub enum AnnotationData {
    /// No additional data.
    None,
    /// Markup annotations (Highlight, Underline, StrikeOut, Squiggly).
    Markup {
        quad_points: Vec<f64>,
    },
    /// Line annotation.
    Line {
        start: (f64, f64),
        end: (f64, f64),
        line_endings: (LineEndingStyle, LineEndingStyle),
        leader_line_length: f64,
        leader_line_extension: f64,
        caption: bool,
        interior_color: Option<AnnotColor>,
    },
    /// Ink annotation (freehand drawing).
    Ink {
        ink_list: Vec<Vec<(f64, f64)>>,
    },
    /// Link annotation.
    Link {
        uri: Option<String>,
        dest: Option<PdfObject>,
    },
    /// FreeText annotation.
    FreeText {
        da: String,
        justification: i64,
    },
    /// FileAttachment annotation.
    FileAttachment {
        fs_ref: Option<IndirectRef>,
        icon_name: String,
    },
    /// Stamp annotation.
    Stamp {
        icon_name: String,
    },
    /// Shape annotations (Square, Circle, Polygon, PolyLine).
    Shape {
        vertices: Vec<(f64, f64)>,
        interior_color: Option<AnnotColor>,
    },
    /// Redact annotation.
    Redact {
        overlay_text: Option<String>,
        repeat: bool,
        interior_color: Option<AnnotColor>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_annotation_type_roundtrip() {
        for name in &[
            b"Text".as_slice(),
            b"Link",
            b"Highlight",
            b"Ink",
            b"3D",
            b"Redact",
        ] {
            let t = AnnotationType::from_name(name);
            assert_eq!(t.to_name(), *name);
        }
    }

    #[test]
    fn test_unknown_annotation_type() {
        let t = AnnotationType::from_name(b"CustomType");
        assert!(matches!(t, AnnotationType::Unknown(_)));
    }

    #[test]
    fn test_annot_flags() {
        let flags = AnnotationFlags(AnnotationFlags::PRINT | AnnotationFlags::LOCKED);
        assert!(flags.has(AnnotationFlags::PRINT));
        assert!(flags.has(AnnotationFlags::LOCKED));
        assert!(!flags.has(AnnotationFlags::HIDDEN));
    }

    #[test]
    fn test_annot_color_from_array() {
        let rgb = AnnotColor::from_array(&[
            PdfObject::Real(1.0),
            PdfObject::Real(1.0),
            PdfObject::Real(0.0),
        ]);
        assert_eq!(rgb, Some(AnnotColor::Rgb(1.0, 1.0, 0.0)));

        let gray = AnnotColor::from_array(&[PdfObject::Real(0.5)]);
        assert_eq!(gray, Some(AnnotColor::Gray(0.5)));

        let empty = AnnotColor::from_array(&[]);
        assert_eq!(empty, None);
    }

    #[test]
    fn test_border_style_from_name() {
        assert_eq!(BorderStyleType::from_name(b"S"), BorderStyleType::Solid);
        assert_eq!(BorderStyleType::from_name(b"D"), BorderStyleType::Dashed);
        assert_eq!(BorderStyleType::from_name(b"X"), BorderStyleType::Solid);
    }

    #[test]
    fn test_line_ending_roundtrip() {
        for style in &[
            LineEndingStyle::None,
            LineEndingStyle::OpenArrow,
            LineEndingStyle::ClosedArrow,
            LineEndingStyle::Circle,
        ] {
            let name = style.to_name();
            assert_eq!(LineEndingStyle::from_name(name), *style);
        }
    }
}
