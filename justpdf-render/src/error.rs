use std::fmt;

pub type Result<T> = std::result::Result<T, RenderError>;

#[derive(Debug)]
pub enum RenderError {
    /// Error from justpdf-core.
    Core(justpdf_core::JustPdfError),
    /// Invalid page dimensions or rendering parameters.
    InvalidDimensions { detail: String },
    /// Image decoding/rendering error.
    Image { detail: String },
    /// PNG encoding error.
    Encode { detail: String },
    /// I/O error.
    Io(std::io::Error),
}

impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Core(e) => write!(f, "PDF error: {e}"),
            Self::InvalidDimensions { detail } => write!(f, "invalid dimensions: {detail}"),
            Self::Image { detail } => write!(f, "image error: {detail}"),
            Self::Encode { detail } => write!(f, "encode error: {detail}"),
            Self::Io(e) => write!(f, "I/O error: {e}"),
        }
    }
}

impl std::error::Error for RenderError {}

impl From<justpdf_core::JustPdfError> for RenderError {
    fn from(e: justpdf_core::JustPdfError) -> Self {
        Self::Core(e)
    }
}

impl From<std::io::Error> for RenderError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}
