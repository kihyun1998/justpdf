//! ZUGFeRD electronic invoice support.
//!
//! Detects ZUGFeRD/Factur-X profiles in PDF documents and extracts
//! the embedded XML invoice data.

use crate::{Result, SpecialError};
use justpdf_core::PdfDocument;

/// ZUGFeRD/Factur-X profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ZugferdProfile {
    Minimum,
    BasicWL,
    Basic,
    EN16931,
    Extended,
    XRechnung,
    Unknown(String),
}

impl std::fmt::Display for ZugferdProfile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Minimum => write!(f, "MINIMUM"),
            Self::BasicWL => write!(f, "BASIC WL"),
            Self::Basic => write!(f, "BASIC"),
            Self::EN16931 => write!(f, "EN 16931"),
            Self::Extended => write!(f, "EXTENDED"),
            Self::XRechnung => write!(f, "XRECHNUNG"),
            Self::Unknown(s) => write!(f, "{s}"),
        }
    }
}

/// ZUGFeRD invoice information extracted from a PDF.
#[derive(Debug)]
pub struct ZugferdInfo {
    /// Detected profile.
    pub profile: ZugferdProfile,
    /// Raw XML invoice data.
    pub xml: String,
    /// Filename of the embedded XML.
    pub filename: String,
}

/// Check if a PDF contains ZUGFeRD/Factur-X invoice data.
pub fn is_zugferd(doc: &PdfDocument) -> bool {
    detect_zugferd_xml(doc).is_some()
}

/// Detect and extract ZUGFeRD invoice information from a PDF.
pub fn extract_zugferd(doc: &PdfDocument) -> Result<ZugferdInfo> {
    let (filename, xml) = detect_zugferd_xml(doc).ok_or(SpecialError::Feature {
        detail: "not a ZUGFeRD/Factur-X PDF \u{2014} no embedded invoice XML found".into(),
    })?;

    let profile = detect_profile(&xml);

    Ok(ZugferdInfo {
        profile,
        xml,
        filename,
    })
}

/// Try to find the embedded ZUGFeRD XML in the PDF's embedded files.
fn detect_zugferd_xml(doc: &PdfDocument) -> Option<(String, String)> {
    // ZUGFeRD embeds XML as a file attachment, typically named:
    // - "factur-x.xml" (Factur-X)
    // - "ZUGFeRD-invoice.xml" (ZUGFeRD 1.0)
    // - "xrechnung.xml" (XRechnung)
    let zugferd_names = [
        "factur-x.xml",
        "zugferd-invoice.xml",
        "xrechnung.xml",
    ];

    let embedded = justpdf_core::embedded_file::read_embedded_files(doc).ok()?;

    for file_spec in &embedded {
        let name_lower = file_spec.filename.to_lowercase();
        for &zf_name in &zugferd_names {
            if name_lower.contains(zf_name) || name_lower.ends_with(".xml") {
                // Try to extract the file content
                if let Ok(data) = justpdf_core::embedded_file::extract_file(doc, file_spec) {
                    if let Ok(xml) = String::from_utf8(data) {
                        if xml.contains("CrossIndustryInvoice") || xml.contains("rsm:") {
                            return Some((file_spec.filename.clone(), xml));
                        }
                    }
                }
            }
        }
    }

    None
}

/// Detect the ZUGFeRD profile from the XML content.
fn detect_profile(xml: &str) -> ZugferdProfile {
    // The profile is indicated in the GuidelineSpecifiedDocumentContextParameter
    let xml_lower = xml.to_lowercase();

    if xml_lower.contains("xrechnung") {
        ZugferdProfile::XRechnung
    } else if xml_lower.contains("extended") {
        ZugferdProfile::Extended
    } else if xml_lower.contains("en16931") || xml_lower.contains("comfort") {
        ZugferdProfile::EN16931
    } else if xml_lower.contains("basic") && xml_lower.contains("wl") {
        ZugferdProfile::BasicWL
    } else if xml_lower.contains("basic") {
        ZugferdProfile::Basic
    } else if xml_lower.contains("minimum") {
        ZugferdProfile::Minimum
    } else {
        ZugferdProfile::Unknown("unknown".into())
    }
}

/// Extract invoice fields from ZUGFeRD XML using roxmltree.
///
/// Returns a map of common invoice fields (invoice number, date, amounts, etc.).
#[cfg(feature = "zugferd")]
pub fn parse_zugferd_xml(xml: &str) -> Result<std::collections::HashMap<String, String>> {
    let doc = roxmltree::Document::parse(xml).map_err(|e| SpecialError::Feature {
        detail: format!("XML parse error: {e}"),
    })?;

    let mut fields = std::collections::HashMap::new();

    // Walk the XML tree looking for common invoice elements
    for node in doc.descendants() {
        if !node.is_element() {
            continue;
        }
        let tag = node.tag_name().name();
        match tag {
            "ID" => {
                // Could be invoice number, buyer/seller ID, etc.
                if let Some(parent) = node.parent() {
                    if parent.tag_name().name() == "ExchangedDocument" {
                        if let Some(text) = node.text() {
                            fields.insert("invoice_number".into(), text.to_string());
                        }
                    }
                }
            }
            "IssueDateTime" | "DateTimeString" => {
                if let Some(text) = node.text() {
                    fields.insert("issue_date".into(), text.to_string());
                }
            }
            "GrandTotalAmount" | "DuePayableAmount" => {
                if let Some(text) = node.text() {
                    fields.insert(tag.to_string(), text.to_string());
                }
            }
            "TaxBasisTotalAmount" | "TaxTotalAmount" => {
                if let Some(text) = node.text() {
                    fields.insert(tag.to_string(), text.to_string());
                }
            }
            "InvoiceCurrencyCode" => {
                if let Some(text) = node.text() {
                    fields.insert("currency".into(), text.to_string());
                }
            }
            "Name" => {
                // Seller or buyer name
                if let Some(parent) = node.parent() {
                    let pname = parent.tag_name().name();
                    if pname == "SellerTradeParty" {
                        if let Some(text) = node.text() {
                            fields.insert("seller_name".into(), text.to_string());
                        }
                    } else if pname == "BuyerTradeParty" {
                        if let Some(text) = node.text() {
                            fields.insert("buyer_name".into(), text.to_string());
                        }
                    }
                }
            }
            _ => {}
        }
    }

    Ok(fields)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_not_zugferd() {
        // A regular (empty) PDF should not be detected as ZUGFeRD
        let pdf = justpdf_core::writer::DocumentBuilder::new()
            .build()
            .unwrap();
        let doc = PdfDocument::from_bytes(pdf).unwrap();
        assert!(!is_zugferd(&doc));
    }

    #[test]
    fn test_extract_non_zugferd_error() {
        let pdf = justpdf_core::writer::DocumentBuilder::new()
            .build()
            .unwrap();
        let doc = PdfDocument::from_bytes(pdf).unwrap();
        assert!(extract_zugferd(&doc).is_err());
    }

    #[test]
    fn test_detect_profile_xrechnung() {
        let xml = r#"<rsm:CrossIndustryInvoice><GuidelineSpecifiedDocumentContextParameter>urn:factur-x.eu:1p0:xrechnung</GuidelineSpecifiedDocumentContextParameter></rsm:CrossIndustryInvoice>"#;
        assert_eq!(detect_profile(xml), ZugferdProfile::XRechnung);
    }

    #[test]
    fn test_detect_profile_basic() {
        let xml = r#"<rsm:CrossIndustryInvoice>urn:factur-x.eu:1p0:basic</rsm:CrossIndustryInvoice>"#;
        assert_eq!(detect_profile(xml), ZugferdProfile::Basic);
    }

    #[test]
    fn test_detect_profile_extended() {
        let xml = r#"<CrossIndustryInvoice>urn:cen.eu:en16931:2017#conformant#urn:factur-x.eu:1p0:extended</CrossIndustryInvoice>"#;
        assert_eq!(detect_profile(xml), ZugferdProfile::Extended);
    }

    #[test]
    fn test_detect_profile_en16931() {
        let xml = r#"<CrossIndustryInvoice>urn:cen.eu:en16931:2017</CrossIndustryInvoice>"#;
        assert_eq!(detect_profile(xml), ZugferdProfile::EN16931);
    }

    #[test]
    fn test_detect_profile_unknown() {
        let xml = "<some>random xml</some>";
        assert_eq!(
            detect_profile(xml),
            ZugferdProfile::Unknown("unknown".into())
        );
    }

    #[test]
    fn test_zugferd_profile_display() {
        assert_eq!(ZugferdProfile::Minimum.to_string(), "MINIMUM");
        assert_eq!(ZugferdProfile::BasicWL.to_string(), "BASIC WL");
        assert_eq!(ZugferdProfile::EN16931.to_string(), "EN 16931");
        assert_eq!(ZugferdProfile::XRechnung.to_string(), "XRECHNUNG");
    }
}
