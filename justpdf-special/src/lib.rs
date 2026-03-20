pub mod error;

#[cfg(feature = "ocr")]
pub mod ocr;

#[cfg(feature = "barcode")]
pub mod barcode;

#[cfg(feature = "zugferd")]
pub mod zugferd;

#[cfg(feature = "bidi")]
pub mod bidi;

#[cfg(feature = "deskew")]
pub mod deskew;

pub use error::{Result, SpecialError};
