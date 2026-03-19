//! PDF digital signature support (Phase 6.3).
//!
//! Provides signature detection, verification, and creation.

mod appearance;
mod byterange;
mod cert;
mod detect;
mod sign_pdf;
mod timestamp;
mod types;
mod verify;

pub use detect::detect_signatures;
pub use sign_pdf::sign_pdf;
pub use types::*;
pub use verify::verify_signature;

use crate::error::Result;
use crate::parser::PdfDocument;

/// High-level: detect and verify all signatures in a PDF.
pub fn verify_all_signatures(doc: &mut PdfDocument) -> Result<Vec<VerificationResult>> {
    let signatures = detect_signatures(doc)?;
    let pdf_data = doc.raw_data().to_vec();
    let mut results = Vec::new();
    for sig in &signatures {
        results.push(verify_signature(&pdf_data, sig)?);
    }
    Ok(results)
}
