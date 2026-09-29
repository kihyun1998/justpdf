#![doc(html_logo_url = "https://raw.githubusercontent.com/kihyun1998/justpdf/master/logo/icons/justpdf-icon-light-128.png")]
#![doc(html_favicon_url = "https://raw.githubusercontent.com/kihyun1998/justpdf/master/logo/favicon/favicon-32.png")]

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
