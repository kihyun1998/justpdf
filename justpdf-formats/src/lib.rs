//! Extended format support for justpdf.
//!
//! Provides parsing, rendering, and PDF conversion for non-PDF document formats:
//! - **XPS/OpenXPS** (feature `xps`)
//! - **EPUB** (feature `epub`)
//! - **SVG** (feature `svg`)
//! - **Office** — DOCX/XLSX/PPTX text extraction (feature `office`)
//! - **CBZ** — Comic Book Archive (feature `cbz`)
//! - **Plain Text** → PDF (feature `plaintext`)

pub mod error;
pub mod common;
pub mod detect;

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

pub use error::{FormatError, Result};
pub use common::{FormatDocument, FormatPage, FormatMetadata};
