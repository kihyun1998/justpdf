/// All errors that can occur in justpdf-core.
#[derive(Debug, thiserror::Error)]
pub enum JustPdfError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("not a PDF file")]
    NotPdf,

    #[error("unexpected end of file at offset {offset}")]
    UnexpectedEof { offset: usize },

    #[error("invalid token at offset {offset}: {detail}")]
    InvalidToken { offset: usize, detail: String },

    #[error("invalid xref at offset {offset}: {detail}")]
    InvalidXref { offset: usize, detail: String },

    #[error("object not found: {obj_num} {gen_num} R")]
    ObjectNotFound { obj_num: u32, gen_num: u16 },

    #[error("stream decode error ({filter}): {detail}")]
    StreamDecode { filter: String, detail: String },

    #[error("circular reference detected at object {obj_num} {gen_num}")]
    CircularReference { obj_num: u32, gen_num: u16 },

    #[error("unsupported PDF version: {version}")]
    UnsupportedVersion { version: String },

    #[error("invalid object definition at offset {offset}: {detail}")]
    InvalidObject { offset: usize, detail: String },

    #[error("startxref not found")]
    StartXrefNotFound,

    #[error("trailer not found")]
    TrailerNotFound,
}

pub type Result<T> = std::result::Result<T, JustPdfError>;

/// Pretty-print a byte slice as a short preview (for error messages).
#[allow(dead_code)]
pub(crate) fn preview_bytes(data: &[u8], max_len: usize) -> String {
    let len = data.len().min(max_len);
    let s: String = data[..len]
        .iter()
        .map(|&b| {
            if b.is_ascii_graphic() || b == b' ' {
                b as char
            } else {
                '.'
            }
        })
        .collect();
    if data.len() > max_len {
        format!("{s}...")
    } else {
        s
    }
}
