//! Extended format support for justpdf.
//!
//! Provides parsing, rendering, and PDF conversion for non-PDF document formats:
//! - **XPS/OpenXPS** (feature `xps`)
//! - **EPUB** (feature `epub`)
//! - **SVG** (feature `svg`)
//! - **Office** — DOCX/XLSX/PPTX text extraction (feature `office`)
//! - **CBZ** — Comic Book Archive (feature `cbz`)
//! - **Plain Text** → PDF (feature `plaintext`)

#![doc(
    html_logo_url = "https://raw.githubusercontent.com/kihyun1998/justpdf/master/logo/icons/justpdf-icon-light-128.png"
)]
#![doc(
    html_favicon_url = "https://raw.githubusercontent.com/kihyun1998/justpdf/master/logo/favicon/favicon-32.png"
)]

pub mod common;
pub mod detect;
pub mod error;

#[cfg(feature = "plaintext")]
pub mod plaintext;

#[cfg(feature = "cbz")]
pub mod cbz;

#[cfg(feature = "svg")]
pub mod svg;

#[cfg(feature = "xps")]
pub mod xps;

#[cfg(feature = "office")]
pub mod office;

#[cfg(feature = "epub")]
pub mod epub;

#[cfg(feature = "mobi")]
pub mod mobi;

#[cfg(feature = "fb2")]
pub mod fb2;

pub use common::{FormatDocument, FormatMetadata, FormatPage};
pub use error::{FormatError, Result};
