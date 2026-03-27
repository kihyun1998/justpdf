//! PDF signing: create digital signatures on PDF documents.
//!
//! Uses the two-pass approach:
//! 1. Create PDF with signature placeholder
//! 2. Compute digest over byte ranges
//! 3. Create CMS SignedData
//! 4. Insert CMS blob into placeholder

use std::io::Write;

use der::{Decode, Encode};
use rsa::pkcs8::DecodePrivateKey;
use signature::SignatureEncoding;

use crate::error::{JustPdfError, Result};

use super::types::{DigestAlgorithm, SigningOptions};

/// Size of the /Contents placeholder in bytes (hex string = 2x this).
const PLACEHOLDER_SIZE: usize = 16384;

/// Sign a PDF document.
///
/// `pdf_data`: the original PDF bytes.
/// `private_key_der`: PKCS#8 DER-encoded RSA private key.
/// `cert_chain_der`: DER-encoded X.509 certificates (signer first, then intermediates).
/// `options`: signing options.
///
/// Returns the signed PDF bytes.
pub fn sign_pdf(
    pdf_data: &[u8],
    private_key_der: &[u8],
    cert_chain_der: &[&[u8]],
    options: &SigningOptions,
) -> Result<Vec<u8>> {
    // Parse the private key
    let private_key = rsa::RsaPrivateKey::from_pkcs8_der(private_key_der)
        .map_err(|e| JustPdfError::SignatureError {
            detail: format!("failed to parse private key: {}", e),
        })?;

    // Parse the signer certificate
    if cert_chain_der.is_empty() {
        return Err(JustPdfError::SignatureError {
            detail: "no certificates provided".into(),
        });
    }
    let signer_cert = x509_cert::Certificate::from_der(cert_chain_der[0])
        .map_err(|e| JustPdfError::SignatureError {
            detail: format!("failed to parse certificate: {}", e),
        })?;

    // Build the PDF with signature placeholder
    let (pdf_bytes, contents_offset, contents_length) =
        build_pdf_with_placeholder(pdf_data, options)?;

    // Compute byte range
    let byte_range = [
        0i64,
        contents_offset as i64,
        (contents_offset + contents_length) as i64,
        (pdf_bytes.len() - contents_offset - contents_length) as i64,
    ];

    // Compute digest over byte ranges
    let digest = compute_digest(&pdf_bytes, &byte_range, options.digest_algorithm)?;

    // Create CMS SignedData
    let cms_blob = create_cms_signed_data(
        &private_key,
        &signer_cert,
        cert_chain_der,
        &digest,
        options.digest_algorithm,
        options.timestamp_token.as_deref(),
    )?;

    if cms_blob.len() > PLACEHOLDER_SIZE {
        return Err(JustPdfError::SignatureError {
            detail: format!(
                "CMS blob ({} bytes) exceeds placeholder ({} bytes)",
                cms_blob.len(),
                PLACEHOLDER_SIZE
            ),
        });
    }

    // Insert CMS blob into placeholder
    let mut result = pdf_bytes;
    let hex_string = hex_encode(&cms_blob, PLACEHOLDER_SIZE);
    result[contents_offset..contents_offset + contents_length]
        .copy_from_slice(hex_string.as_bytes());

    // Fix ByteRange values
    fix_byte_range(&mut result, &byte_range)?;

    Ok(result)
}

/// Build a PDF with an incremental update containing the signature placeholder.
fn build_pdf_with_placeholder(
    pdf_data: &[u8],
    options: &SigningOptions,
) -> Result<(Vec<u8>, usize, usize)> {
    // We'll append an incremental update with:
    // 1. The signature value dictionary (with placeholder /Contents)
    // 2. A signature field
    // 3. Updated AcroForm
    // 4. Updated Catalog (if needed)
    // 5. New xref + trailer

    let mut buf = pdf_data.to_vec();
    let old_startxref = crate::xref::find_startxref(pdf_data)?;

    // Parse the existing document to get catalog info
    let doc = crate::parser::PdfDocument::from_bytes(pdf_data.to_vec())?;
    let catalog_ref = doc
        .catalog_ref()
        .ok_or(JustPdfError::TrailerNotFound)?
        .clone();

    // Allocate object numbers for new objects
    let max_existing = doc.object_count() as u32 + 10;
    let sig_value_num = max_existing + 1;
    let sig_field_num = max_existing + 2;
    let ap_stream_num = max_existing + 3; // for visible signature appearance

    // Create the signature value dictionary content
    let signer_name = options
        .signer_name
        .as_deref()
        .unwrap_or("justpdf");
    let reason = options.reason.as_deref().unwrap_or("");
    let location = options.location.as_deref().unwrap_or("");

    // Build the signature value dict with placeholder
    // We need to track where /Contents appears in the output
    let mut offsets = Vec::new();

    // Write sig value object
    let sig_val_offset = buf.len();
    write!(buf, "{} 0 obj\n", sig_value_num)?;
    write!(buf, "<< /Type /Sig /Filter /Adobe.PPKLite /SubFilter /adbe.pkcs7.detached ")?;

    // ByteRange placeholder (will be fixed up later)
    write!(buf, "/ByteRange [0 0000000000 0000000000 0000000000] ")?;

    // /Contents placeholder — this is the critical part
    write!(buf, "/Contents <")?;
    let contents_offset = buf.len();
    let contents_length = PLACEHOLDER_SIZE * 2; // hex characters
    buf.extend(std::iter::repeat(b'0').take(contents_length));
    write!(buf, "> ")?;

    write!(buf, "/Name ({}) ", escape_pdf_string(signer_name))?;
    if !reason.is_empty() {
        write!(buf, "/Reason ({}) ", escape_pdf_string(reason))?;
    }
    if !location.is_empty() {
        write!(buf, "/Location ({}) ", escape_pdf_string(location))?;
    }
    write!(buf, ">>\nendobj\n")?;
    offsets.push((sig_value_num, sig_val_offset));

    // Write signature field
    let sig_field_offset = buf.len();
    write!(buf, "{} 0 obj\n", sig_field_num)?;
    write!(buf, "<< /Type /Annot /Subtype /Widget /FT /Sig ")?;
    write!(buf, "/T (Signature1) ")?;
    write!(buf, "/V {} 0 R ", sig_value_num)?;

    if options.visible {
        let rect = options.appearance_rect.unwrap_or([0.0, 0.0, 200.0, 80.0]);
        write!(buf, "/Rect [{} {} {} {}] ", rect[0], rect[1], rect[2], rect[3])?;
        write!(buf, "/AP << /N {} 0 R >> ", ap_stream_num)?;
        write!(buf, "/F 4 ")?; // Print (visible)
    } else {
        write!(buf, "/Rect [0 0 0 0] ")?;
        write!(buf, "/F 132 ")?; // Print + Hidden
    }

    write!(buf, ">>\nendobj\n")?;
    offsets.push((sig_field_num, sig_field_offset));

    // Write appearance stream (if visible)
    if options.visible {
        let rect = options.appearance_rect.unwrap_or([0.0, 0.0, 200.0, 80.0]);
        let ap_width = rect[2] - rect[0];
        let ap_height = rect[3] - rect[1];
        let (ap_dict, ap_data) = super::appearance::generate_signature_appearance(
            signer_name,
            if reason.is_empty() { None } else { Some(reason) },
            None, // date is embedded in the CMS signing time
            ap_width,
            ap_height,
        );

        let ap_offset = buf.len();
        write!(buf, "{} 0 obj\n", ap_stream_num)?;
        // Write dict entries (make_stream already includes /Length and /Filter)
        write!(buf, "<< ")?;
        for (key, value) in ap_dict.iter() {
            write!(buf, "/{} ", String::from_utf8_lossy(key))?;
            write_pdf_value(&mut buf, value);
        }
        write!(buf, ">>\n")?;
        write!(buf, "stream\r\n")?;
        buf.extend_from_slice(&ap_data);
        write!(buf, "\r\nendstream\nendobj\n")?;
        offsets.push((ap_stream_num, ap_offset));
    }

    // Write new xref
    let xref_offset = buf.len();
    write!(buf, "xref\n")?;

    // Write subsections
    for (obj_num, offset) in &offsets {
        write!(buf, "{} 1\n", obj_num)?;
        write!(buf, "{:010} {:05} n \r\n", offset, 0)?;
    }

    // Trailer
    let xref_size = if options.visible { ap_stream_num + 1 } else { sig_field_num + 1 };
    write!(buf, "trailer\n")?;
    write!(buf, "<< /Size {} /Root {} 0 R /Prev {} >>\n",
        xref_size, catalog_ref.obj_num, old_startxref)?;
    write!(buf, "startxref\n{}\n%%EOF\n", xref_offset)?;

    Ok((buf, contents_offset, contents_length))
}

/// Compute the digest over byte ranges.
fn compute_digest(
    pdf_data: &[u8],
    byte_range: &[i64; 4],
    algorithm: DigestAlgorithm,
) -> Result<Vec<u8>> {
    super::byterange::compute_byterange_digest(pdf_data, byte_range, algorithm)
}

/// Create a CMS SignedData structure.
fn create_cms_signed_data(
    private_key: &rsa::RsaPrivateKey,
    signer_cert: &x509_cert::Certificate,
    cert_chain_der: &[&[u8]],
    digest: &[u8],
    algorithm: DigestAlgorithm,
    timestamp_token: Option<&[u8]>,
) -> Result<Vec<u8>> {
    use rsa::pkcs1v15::SigningKey;
    use signature::Signer;

    let digest_oid = match algorithm {
        DigestAlgorithm::Sha256 => const_oid::ObjectIdentifier::new_unwrap("2.16.840.1.101.3.4.2.1"),
        DigestAlgorithm::Sha384 => const_oid::ObjectIdentifier::new_unwrap("2.16.840.1.101.3.4.2.2"),
        DigestAlgorithm::Sha512 => const_oid::ObjectIdentifier::new_unwrap("2.16.840.1.101.3.4.2.3"),
    };

    let rsa_oid = const_oid::ObjectIdentifier::new_unwrap("1.2.840.113549.1.1.1");
    let signed_data_oid = const_oid::ObjectIdentifier::new_unwrap("1.2.840.113549.1.7.2");
    let data_oid = const_oid::ObjectIdentifier::new_unwrap("1.2.840.113549.1.7.1");
    let message_digest_oid = const_oid::ObjectIdentifier::new_unwrap("1.2.840.113549.1.9.4");
    let content_type_oid = const_oid::ObjectIdentifier::new_unwrap("1.2.840.113549.1.9.3");
    let signing_time_oid = const_oid::ObjectIdentifier::new_unwrap("1.2.840.113549.1.9.5");

    // Build signed attributes
    let digest_octet = der::asn1::OctetString::new(digest)
        .map_err(|e| JustPdfError::SignatureError {
            detail: format!("OctetString error: {}", e),
        })?;

    // Create the signed attributes DER manually for signing
    // (contentType, signingTime, messageDigest)
    let content_type_attr_value = data_oid.to_der()
        .map_err(|e| JustPdfError::SignatureError { detail: format!("DER error: {}", e) })?;
    let digest_attr_value = digest_octet.to_der()
        .map_err(|e| JustPdfError::SignatureError { detail: format!("DER error: {}", e) })?;

    // Build signed attributes as a SET
    let mut signed_attrs_content = Vec::new();

    // contentType attribute
    append_attribute(&mut signed_attrs_content, &content_type_oid, &content_type_attr_value)?;
    // signingTime attribute (UTCTime)
    let signing_time_der = build_utctime_now();
    append_attribute(&mut signed_attrs_content, &signing_time_oid, &signing_time_der)?;
    // messageDigest attribute
    append_attribute(&mut signed_attrs_content, &message_digest_oid, &digest_attr_value)?;

    // Wrap in SET tag (0x31) for the actual signature input
    let mut signed_attrs_for_sign = vec![0x31u8]; // SET tag
    encode_der_length(signed_attrs_content.len(), &mut signed_attrs_for_sign);
    signed_attrs_for_sign.extend_from_slice(&signed_attrs_content);

    // Sign the signed attributes
    let sig_bytes = match algorithm {
        DigestAlgorithm::Sha256 => {
            let signing_key = SigningKey::<sha2::Sha256>::new(private_key.clone());
            signing_key.sign(&signed_attrs_for_sign).to_vec()
        }
        DigestAlgorithm::Sha384 => {
            let signing_key = SigningKey::<sha2::Sha384>::new(private_key.clone());
            signing_key.sign(&signed_attrs_for_sign).to_vec()
        }
        DigestAlgorithm::Sha512 => {
            let signing_key = SigningKey::<sha2::Sha512>::new(private_key.clone());
            signing_key.sign(&signed_attrs_for_sign).to_vec()
        }
    };

    // Build the complete CMS SignedData structure manually
    // This is built as raw DER because the `cms` crate's builder API
    // is complex to use for construction.
    let mut signed_data_content = Vec::new();

    // version INTEGER 1
    signed_data_content.extend_from_slice(&[0x02, 0x01, 0x01]);

    // digestAlgorithms SET OF { AlgorithmIdentifier }
    let mut digest_alg_set = Vec::new();
    let alg_seq = build_algorithm_identifier(&digest_oid)?;
    digest_alg_set.extend_from_slice(&alg_seq);
    let digest_alg_set_tlv = wrap_der_set(&digest_alg_set);
    signed_data_content.extend_from_slice(&digest_alg_set_tlv);

    // encapContentInfo SEQUENCE { contentType OID }
    let eci_oid = data_oid.to_der()
        .map_err(|e| JustPdfError::SignatureError { detail: format!("DER error: {}", e) })?;
    let eci_seq = wrap_der_sequence(&eci_oid);
    signed_data_content.extend_from_slice(&eci_seq);

    // certificates [0] IMPLICIT CertificateSet
    let mut cert_set_content = Vec::new();
    for cert_der in cert_chain_der {
        cert_set_content.extend_from_slice(cert_der);
    }
    // Context tag [0] CONSTRUCTED
    let mut cert_set_tlv = vec![0xA0];
    encode_der_length(cert_set_content.len(), &mut cert_set_tlv);
    cert_set_tlv.extend_from_slice(&cert_set_content);
    signed_data_content.extend_from_slice(&cert_set_tlv);

    // signerInfos SET OF { SignerInfo }
    let signer_info = build_signer_info(
        signer_cert,
        &digest_oid,
        &rsa_oid,
        &signed_attrs_content,
        &sig_bytes,
        timestamp_token,
    )?;
    let signer_infos_set = wrap_der_set(&signer_info);
    signed_data_content.extend_from_slice(&signer_infos_set);

    // Wrap in SEQUENCE
    let signed_data_seq = wrap_der_sequence(&signed_data_content);

    // Wrap in ContentInfo
    let sd_oid = signed_data_oid.to_der()
        .map_err(|e| JustPdfError::SignatureError { detail: format!("DER error: {}", e) })?;

    // Content [0] EXPLICIT
    let mut explicit_content = vec![0xA0];
    encode_der_length(signed_data_seq.len(), &mut explicit_content);
    explicit_content.extend_from_slice(&signed_data_seq);

    let mut content_info = Vec::new();
    content_info.extend_from_slice(&sd_oid);
    content_info.extend_from_slice(&explicit_content);

    let content_info_seq = wrap_der_sequence(&content_info);

    Ok(content_info_seq)
}

/// Build a SignerInfo structure.
fn build_signer_info(
    signer_cert: &x509_cert::Certificate,
    digest_oid: &const_oid::ObjectIdentifier,
    sig_oid: &const_oid::ObjectIdentifier,
    signed_attrs_content: &[u8],
    sig_bytes: &[u8],
    timestamp_token: Option<&[u8]>,
) -> Result<Vec<u8>> {
    let mut si_content = Vec::new();

    // version INTEGER 1
    si_content.extend_from_slice(&[0x02, 0x01, 0x01]);

    // sid IssuerAndSerialNumber
    let issuer_der = signer_cert.tbs_certificate.issuer.to_der()
        .map_err(|e| JustPdfError::SignatureError { detail: format!("DER error: {}", e) })?;
    let serial_der = signer_cert.tbs_certificate.serial_number.to_der()
        .map_err(|e| JustPdfError::SignatureError { detail: format!("DER error: {}", e) })?;
    let mut iasn = Vec::new();
    iasn.extend_from_slice(&issuer_der);
    iasn.extend_from_slice(&serial_der);
    let iasn_seq = wrap_der_sequence(&iasn);
    si_content.extend_from_slice(&iasn_seq);

    // digestAlgorithm
    let alg_id = build_algorithm_identifier(digest_oid)?;
    si_content.extend_from_slice(&alg_id);

    // signedAttrs [0] IMPLICIT
    let mut signed_attrs_tlv = vec![0xA0];
    encode_der_length(signed_attrs_content.len(), &mut signed_attrs_tlv);
    signed_attrs_tlv.extend_from_slice(signed_attrs_content);
    si_content.extend_from_slice(&signed_attrs_tlv);

    // signatureAlgorithm
    let sig_alg = build_algorithm_identifier(sig_oid)?;
    si_content.extend_from_slice(&sig_alg);

    // signature OCTET STRING
    let mut sig_octet = vec![0x04u8];
    encode_der_length(sig_bytes.len(), &mut sig_octet);
    sig_octet.extend_from_slice(sig_bytes);
    si_content.extend_from_slice(&sig_octet);

    // unsignedAttrs [1] IMPLICIT — timestamp token
    if let Some(token) = timestamp_token {
        let ts_token_oid = const_oid::ObjectIdentifier::new_unwrap("1.2.840.113549.1.9.16.2.14");
        let mut unsigned_attrs_content = Vec::new();
        append_attribute(&mut unsigned_attrs_content, &ts_token_oid, token)?;
        // Context tag [1] CONSTRUCTED for unsignedAttrs
        let mut unsigned_attrs_tlv = vec![0xA1];
        encode_der_length(unsigned_attrs_content.len(), &mut unsigned_attrs_tlv);
        unsigned_attrs_tlv.extend_from_slice(&unsigned_attrs_content);
        si_content.extend_from_slice(&unsigned_attrs_tlv);
    }

    Ok(wrap_der_sequence(&si_content))
}

// --- PDF helpers ---

/// Write a PdfObject value to a byte buffer (for inline dict serialization).
fn write_pdf_value(buf: &mut Vec<u8>, value: &crate::object::PdfObject) {
    use crate::object::PdfObject;
    match value {
        PdfObject::Name(n) => {
            buf.extend_from_slice(b"/");
            buf.extend_from_slice(n);
            buf.push(b' ');
        }
        PdfObject::Integer(i) => {
            buf.extend_from_slice(format!("{} ", i).as_bytes());
        }
        PdfObject::Real(f) => {
            buf.extend_from_slice(format!("{} ", f).as_bytes());
        }
        PdfObject::Array(arr) => {
            buf.extend_from_slice(b"[");
            for item in arr {
                write_pdf_value(buf, item);
            }
            buf.extend_from_slice(b"] ");
        }
        PdfObject::Dict(d) => {
            buf.extend_from_slice(b"<< ");
            for (k, v) in d.iter() {
                buf.extend_from_slice(b"/");
                buf.extend_from_slice(k);
                buf.push(b' ');
                write_pdf_value(buf, v);
            }
            buf.extend_from_slice(b">> ");
        }
        PdfObject::String(s) => {
            buf.extend_from_slice(b"(");
            buf.extend_from_slice(s);
            buf.extend_from_slice(b") ");
        }
        PdfObject::Reference(r) => {
            buf.extend_from_slice(format!("{} {} R ", r.obj_num, r.gen_num).as_bytes());
        }
        PdfObject::Bool(b_val) => {
            buf.extend_from_slice(if *b_val { b"true " } else { b"false " });
        }
        PdfObject::Null => {
            buf.extend_from_slice(b"null ");
        }
        _ => {} // Stream and other complex types not needed here
    }
}

// --- DER helpers ---

/// Build a DER-encoded UTCTime for the current time.
/// Format: YYMMDDHHMMSSZ (13 bytes), tag 0x17.
fn build_utctime_now() -> Vec<u8> {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // Convert epoch seconds to date/time components
    // Simple conversion without external crate
    let days = secs / 86400;
    let time_of_day = secs % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;

    // Days since 1970-01-01 to (year, month, day)
    let mut y = 1970u64;
    let mut remaining_days = days;
    loop {
        let days_in_year = if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) { 366 } else { 365 };
        if remaining_days < days_in_year {
            break;
        }
        remaining_days -= days_in_year;
        y += 1;
    }
    let is_leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
    let month_days = [31, if is_leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut m = 0u64;
    for md in &month_days {
        if remaining_days < *md as u64 {
            break;
        }
        remaining_days -= *md as u64;
        m += 1;
    }
    let month = m + 1;
    let day = remaining_days + 1;

    // UTCTime uses 2-digit year (YY)
    let yy = y % 100;
    let utc_str = format!("{:02}{:02}{:02}{:02}{:02}{:02}Z", yy, month, day, hours, minutes, seconds);
    let mut result = vec![0x17, utc_str.len() as u8];
    result.extend_from_slice(utc_str.as_bytes());
    result
}

fn build_algorithm_identifier(oid: &const_oid::ObjectIdentifier) -> Result<Vec<u8>> {
    let oid_der = oid.to_der()
        .map_err(|e| JustPdfError::SignatureError { detail: format!("DER error: {}", e) })?;
    // SEQUENCE { OID, NULL }
    let mut content = Vec::new();
    content.extend_from_slice(&oid_der);
    content.extend_from_slice(&[0x05, 0x00]); // NULL
    Ok(wrap_der_sequence(&content))
}

fn append_attribute(buf: &mut Vec<u8>, oid: &const_oid::ObjectIdentifier, value: &[u8]) -> Result<()> {
    let oid_der = oid.to_der()
        .map_err(|e| JustPdfError::SignatureError { detail: format!("DER error: {}", e) })?;
    let value_set = wrap_der_set(value);
    let mut attr_content = Vec::new();
    attr_content.extend_from_slice(&oid_der);
    attr_content.extend_from_slice(&value_set);
    let attr_seq = wrap_der_sequence(&attr_content);
    buf.extend_from_slice(&attr_seq);
    Ok(())
}

fn wrap_der_sequence(content: &[u8]) -> Vec<u8> {
    let mut result = vec![0x30u8];
    encode_der_length(content.len(), &mut result);
    result.extend_from_slice(content);
    result
}

fn wrap_der_set(content: &[u8]) -> Vec<u8> {
    let mut result = vec![0x31u8];
    encode_der_length(content.len(), &mut result);
    result.extend_from_slice(content);
    result
}

fn encode_der_length(len: usize, buf: &mut Vec<u8>) {
    if len < 0x80 {
        buf.push(len as u8);
    } else if len < 0x100 {
        buf.push(0x81);
        buf.push(len as u8);
    } else if len < 0x10000 {
        buf.push(0x82);
        buf.push((len >> 8) as u8);
        buf.push(len as u8);
    } else {
        buf.push(0x83);
        buf.push((len >> 16) as u8);
        buf.push((len >> 8) as u8);
        buf.push(len as u8);
    }
}

/// Hex-encode bytes, padded to a fixed length with trailing zeros.
fn hex_encode(data: &[u8], total_bytes: usize) -> String {
    let mut hex = String::with_capacity(total_bytes * 2);
    for byte in data {
        hex.push_str(&format!("{:02X}", byte));
    }
    // Pad remaining with zeros
    while hex.len() < total_bytes * 2 {
        hex.push('0');
    }
    hex
}

/// Fix the /ByteRange values in the PDF.
fn fix_byte_range(pdf: &mut [u8], byte_range: &[i64; 4]) -> Result<()> {
    // Find the /ByteRange pattern and replace placeholder values
    let needle = b"/ByteRange [0 0000000000 0000000000 0000000000]";
    let replacement = format!(
        "/ByteRange [0 {:010} {:010} {:010}]",
        byte_range[1], byte_range[2], byte_range[3]
    );

    if let Some(pos) = find_bytes(pdf, needle) {
        pdf[pos..pos + replacement.len()].copy_from_slice(replacement.as_bytes());
        Ok(())
    } else {
        Err(JustPdfError::SignatureError {
            detail: "could not find ByteRange placeholder".into(),
        })
    }
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn escape_pdf_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('(', "\\(")
        .replace(')', "\\)")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hex_encode() {
        let data = vec![0xDE, 0xAD, 0xBE, 0xEF];
        let hex = hex_encode(&data, 8);
        assert_eq!(hex, "DEADBEEF00000000");
    }

    #[test]
    fn test_hex_encode_exact() {
        let data = vec![0xFF, 0x00];
        let hex = hex_encode(&data, 2);
        assert_eq!(hex, "FF00");
    }

    #[test]
    fn test_find_bytes() {
        let data = b"Hello ByteRange World";
        assert_eq!(find_bytes(data, b"ByteRange"), Some(6));
        assert_eq!(find_bytes(data, b"Missing"), None);
    }

    #[test]
    fn test_wrap_der_sequence() {
        let content = vec![0x02, 0x01, 0x01]; // INTEGER 1
        let seq = wrap_der_sequence(&content);
        assert_eq!(seq, vec![0x30, 0x03, 0x02, 0x01, 0x01]);
    }

    #[test]
    fn test_sign_pdf_no_key() {
        // Invalid key should fail gracefully
        let pdf = b"%PDF-1.4\n";
        let opts = SigningOptions::default();
        let result = sign_pdf(pdf, b"bad key", &[b"bad cert"], &opts);
        assert!(result.is_err());
    }
}
