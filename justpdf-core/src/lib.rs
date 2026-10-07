#![doc(
    html_logo_url = "https://raw.githubusercontent.com/kihyun1998/justpdf/master/logo/icons/justpdf-icon-light-128.png"
)]
#![doc(
    html_favicon_url = "https://raw.githubusercontent.com/kihyun1998/justpdf/master/logo/favicon/favicon-32.png"
)]

pub mod action;
pub mod annot;
pub mod color;
pub mod content;
pub mod crypto;
pub mod embedded_file;
pub mod error;
pub mod font;
pub mod form;
pub mod function;
pub mod image;
pub mod journal;
pub mod linearized;
pub mod object;
pub mod ocg;
pub mod outline;
pub mod page;
pub mod page_label;
pub mod parser;
pub mod repair;
pub mod sign;
pub mod stream;
pub mod text;
pub mod tokenizer;
mod tree_walk;
pub mod writer;
pub mod xref;

pub use error::{JustPdfError, Result};
pub use object::{IndirectRef, PdfDict, PdfObject};
pub use parser::PdfDocument;
/// The TrueType parser core reads fonts with, for callers of [`font::truetype_glyph_candidates`].
pub use ttf_parser;
