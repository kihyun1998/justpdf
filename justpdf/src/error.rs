use std::fmt;

/// High-level error type for the justpdf crate.
#[derive(Debug)]
pub enum Error {
    /// Error from the core PDF engine.
    Core(justpdf_core::JustPdfError),
    /// Error from the rendering engine.
    Render(justpdf_render::RenderError),
    /// I/O error.
    Io(std::io::Error),
    /// Page index out of range.
    PageOutOfRange {
        /// The requested page index.
        index: usize,
        /// Total number of pages in the document.
        count: usize,
    },
}

/// A specialized `Result` type for justpdf operations.
pub type Result<T> = std::result::Result<T, Error>;

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Core(e) => write!(f, "{e}"),
            Error::Render(e) => write!(f, "{e}"),
            Error::Io(e) => write!(f, "{e}"),
            Error::PageOutOfRange { index, count } => {
                write!(f, "page index {index} out of range (document has {count} pages)")
            }
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Core(e) => Some(e),
            Error::Render(e) => Some(e),
            Error::Io(e) => Some(e),
            Error::PageOutOfRange { .. } => None,
        }
    }
}

impl From<justpdf_core::JustPdfError> for Error {
    fn from(e: justpdf_core::JustPdfError) -> Self {
        Error::Core(e)
    }
}

impl From<justpdf_render::RenderError> for Error {
    fn from(e: justpdf_render::RenderError) -> Self {
        Error::Render(e)
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}
