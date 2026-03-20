use std::path::Path;

/// Detected document format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentFormat {
    Pdf,
    Xps,
    Epub,
    Svg,
    Docx,
    Xlsx,
    Pptx,
    Cbz,
    PlainText,
    Unknown,
}

/// Detect document format from file extension.
pub fn detect_format(path: &Path) -> DocumentFormat {
    match path.extension().and_then(|e| e.to_str()).map(|e| e.to_lowercase()).as_deref() {
        Some("pdf") => DocumentFormat::Pdf,
        Some("xps" | "oxps") => DocumentFormat::Xps,
        Some("epub") => DocumentFormat::Epub,
        Some("svg" | "svgz") => DocumentFormat::Svg,
        Some("docx") => DocumentFormat::Docx,
        Some("xlsx") => DocumentFormat::Xlsx,
        Some("pptx") => DocumentFormat::Pptx,
        Some("cbz") => DocumentFormat::Cbz,
        Some("txt" | "text" | "md" | "rst" | "log") => DocumentFormat::PlainText,
        _ => DocumentFormat::Unknown,
    }
}

/// Detect document format from file content (magic bytes).
pub fn detect_format_from_bytes(data: &[u8]) -> DocumentFormat {
    if data.len() < 4 {
        return DocumentFormat::Unknown;
    }

    // PDF magic
    if data.starts_with(b"%PDF") {
        return DocumentFormat::Pdf;
    }

    // ZIP magic (XPS, EPUB, Office, CBZ)
    if data.starts_with(b"PK\x03\x04") {
        // Try to distinguish by inspecting ZIP contents
        // (simplified: just return Unknown for ZIP, let the caller try each format)
        return DocumentFormat::Unknown;
    }

    // SVG (XML with <svg)
    if data.starts_with(b"<?xml") || data.starts_with(b"<svg") {
        // Check for <svg tag
        let search_len = data.len().min(1024);
        let header = std::str::from_utf8(&data[..search_len]).unwrap_or("");
        if header.contains("<svg") {
            return DocumentFormat::Svg;
        }
    }

    DocumentFormat::Unknown
}

impl std::fmt::Display for DocumentFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pdf => write!(f, "PDF"),
            Self::Xps => write!(f, "XPS"),
            Self::Epub => write!(f, "EPUB"),
            Self::Svg => write!(f, "SVG"),
            Self::Docx => write!(f, "DOCX"),
            Self::Xlsx => write!(f, "XLSX"),
            Self::Pptx => write!(f, "PPTX"),
            Self::Cbz => write!(f, "CBZ"),
            Self::PlainText => write!(f, "Plain Text"),
            Self::Unknown => write!(f, "Unknown"),
        }
    }
}
