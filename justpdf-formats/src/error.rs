use std::fmt;

#[derive(Debug)]
pub enum FormatError {
    /// I/O error.
    Io(std::io::Error),
    /// ZIP archive error.
    #[cfg(any(feature = "xps", feature = "epub", feature = "office", feature = "cbz"))]
    Zip(String),
    /// XML parsing error.
    #[cfg(any(feature = "xps", feature = "epub", feature = "svg", feature = "office", feature = "fb2"))]
    Xml(String),
    /// PDF generation error.
    Pdf(justpdf_core::JustPdfError),
    /// Render error.
    Render(justpdf_render::error::RenderError),
    /// Format-specific error.
    Format { detail: String },
    /// Unsupported format.
    UnsupportedFormat { extension: String },
    /// Page/chapter index out of range.
    OutOfRange { index: usize, count: usize },
}

pub type Result<T> = std::result::Result<T, FormatError>;

impl fmt::Display for FormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error: {e}"),
            #[cfg(any(feature = "xps", feature = "epub", feature = "office", feature = "cbz"))]
            Self::Zip(e) => write!(f, "ZIP error: {e}"),
            #[cfg(any(feature = "xps", feature = "epub", feature = "svg", feature = "office", feature = "fb2"))]
            Self::Xml(e) => write!(f, "XML error: {e}"),
            Self::Pdf(e) => write!(f, "PDF error: {e}"),
            Self::Render(e) => write!(f, "render error: {e}"),
            Self::Format { detail } => write!(f, "format error: {detail}"),
            Self::UnsupportedFormat { extension } => write!(f, "unsupported format: .{extension}"),
            Self::OutOfRange { index, count } => write!(f, "index {index} out of range ({count} pages)"),
        }
    }
}

impl std::error::Error for FormatError {}

impl From<std::io::Error> for FormatError {
    fn from(e: std::io::Error) -> Self { Self::Io(e) }
}

impl From<justpdf_core::JustPdfError> for FormatError {
    fn from(e: justpdf_core::JustPdfError) -> Self { Self::Pdf(e) }
}

impl From<justpdf_render::error::RenderError> for FormatError {
    fn from(e: justpdf_render::error::RenderError) -> Self { Self::Render(e) }
}
