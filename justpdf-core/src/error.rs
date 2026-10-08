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

    #[error("tree walk visit limit exceeded at object {obj_num} {gen_num}")]
    LimitExceeded { obj_num: u32, gen_num: u16 },

    #[error("page index {index} out of range (document has {count} pages)")]
    PageOutOfRange { index: usize, count: usize },

    #[error("unsupported PDF version: {version}")]
    UnsupportedVersion { version: String },

    #[error("invalid object definition at offset {offset}: {detail}")]
    InvalidObject { offset: usize, detail: String },

    #[error("startxref not found")]
    StartXrefNotFound,

    #[error("trailer not found")]
    TrailerNotFound,

    #[error("annotation error: {detail}")]
    AnnotationError { detail: String },

    #[error("form error: {detail}")]
    FormError { detail: String },

    #[error("encryption error: {detail}")]
    EncryptionError { detail: String },

    #[error("incorrect password")]
    IncorrectPassword,

    #[error("unsupported encryption: {detail}")]
    UnsupportedEncryption { detail: String },

    #[error("document is encrypted, call authenticate() first")]
    EncryptedDocument,

    #[error("signature error: {detail}")]
    SignatureError { detail: String },

    #[error("outline error: {detail}")]
    OutlineError { detail: String },

    #[error("embedded file error: {detail}")]
    EmbeddedFileError { detail: String },

    #[error("optional content error: {detail}")]
    OptionalContentError { detail: String },

    #[error("repair error: {detail}")]
    RepairError { detail: String },
}

pub type Result<T> = std::result::Result<T, JustPdfError>;

impl JustPdfError {
    /// A copy of this error, for a variant that carries only data. `None`
    /// for `Io` and `CircularReference`.
    pub(crate) fn rebuilt(&self) -> Option<JustPdfError> {
        use JustPdfError::*;
        Some(match self {
            Io(_) | CircularReference { .. } => return None,
            NotPdf => NotPdf,
            UnexpectedEof { offset } => UnexpectedEof { offset: *offset },
            InvalidToken { offset, detail } => InvalidToken {
                offset: *offset,
                detail: detail.clone(),
            },
            InvalidXref { offset, detail } => InvalidXref {
                offset: *offset,
                detail: detail.clone(),
            },
            ObjectNotFound { obj_num, gen_num } => ObjectNotFound {
                obj_num: *obj_num,
                gen_num: *gen_num,
            },
            StreamDecode { filter, detail } => StreamDecode {
                filter: filter.clone(),
                detail: detail.clone(),
            },
            LimitExceeded { obj_num, gen_num } => LimitExceeded {
                obj_num: *obj_num,
                gen_num: *gen_num,
            },
            PageOutOfRange { index, count } => PageOutOfRange {
                index: *index,
                count: *count,
            },
            UnsupportedVersion { version } => UnsupportedVersion {
                version: version.clone(),
            },
            InvalidObject { offset, detail } => InvalidObject {
                offset: *offset,
                detail: detail.clone(),
            },
            StartXrefNotFound => StartXrefNotFound,
            TrailerNotFound => TrailerNotFound,
            AnnotationError { detail } => AnnotationError {
                detail: detail.clone(),
            },
            FormError { detail } => FormError {
                detail: detail.clone(),
            },
            EncryptionError { detail } => EncryptionError {
                detail: detail.clone(),
            },
            IncorrectPassword => IncorrectPassword,
            UnsupportedEncryption { detail } => UnsupportedEncryption {
                detail: detail.clone(),
            },
            EncryptedDocument => EncryptedDocument,
            SignatureError { detail } => SignatureError {
                detail: detail.clone(),
            },
            OutlineError { detail } => OutlineError {
                detail: detail.clone(),
            },
            EmbeddedFileError { detail } => EmbeddedFileError {
                detail: detail.clone(),
            },
            OptionalContentError { detail } => OptionalContentError {
                detail: detail.clone(),
            },
            RepairError { detail } => RepairError {
                detail: detail.clone(),
            },
        })
    }
}

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
