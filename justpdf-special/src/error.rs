use std::fmt;

/// Errors from justpdf-special operations.
#[derive(Debug)]
pub enum SpecialError {
    /// I/O error.
    Io(std::io::Error),
    /// Underlying PDF error from justpdf-core.
    Pdf(justpdf_core::JustPdfError),
    /// A feature-specific error (encoding, generation, etc.).
    Feature { detail: String },
    /// A required external tool or resource was not found.
    NotFound { detail: String },
}

/// Convenience alias.
pub type Result<T> = std::result::Result<T, SpecialError>;

impl fmt::Display for SpecialError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error: {e}"),
            Self::Pdf(e) => write!(f, "PDF error: {e}"),
            Self::Feature { detail } => write!(f, "feature error: {detail}"),
            Self::NotFound { detail } => write!(f, "not found: {detail}"),
        }
    }
}

impl std::error::Error for SpecialError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Pdf(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for SpecialError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<justpdf_core::JustPdfError> for SpecialError {
    fn from(e: justpdf_core::JustPdfError) -> Self {
        Self::Pdf(e)
    }
}
