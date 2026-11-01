//! PDF signing: create digital signatures on PDF documents.
//!
//! `sign_pdf` appends an incremental update and signs it in place:
//! 1. Append the signature dictionary with `/ByteRange` and `/Contents`
//!    placeholders, the signature field and widget, its appearance, and new
//!    revisions of the catalog, `/AcroForm`, page and `/Annots` objects that
//!    link the field into the document
//! 2. Write the byte range over its placeholder
//! 3. Compute the digest over the byte range
//! 4. Create the CMS SignedData and write it into `/Contents`

use std::io::Write;

use der::{Decode, Encode};
use rsa::pkcs8::DecodePrivateKey;
use signature::SignatureEncoding;

use crate::error::{JustPdfError, Result};
use crate::object::{IndirectRef, PdfDict, PdfObject, real_syntax, string_syntax};
use crate::writer::serialize::serialize_dict;

use super::types::{DigestAlgorithm, SigningOptions};

/// Size of the /Contents placeholder in bytes (hex string = 2x this).
const PLACEHOLDER_SIZE: usize = 16384;

/// The `/ByteRange` array as reserved, ten digits per value.
const BYTE_RANGE_PLACEHOLDER: &str = "[0 0000000000 0000000000 0000000000]";

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
    let private_key = rsa::RsaPrivateKey::from_pkcs8_der(private_key_der).map_err(|e| {
        JustPdfError::SignatureError {
            detail: format!("failed to parse private key: {}", e),
        }
    })?;

    // Parse the signer certificate
    if cert_chain_der.is_empty() {
        return Err(JustPdfError::SignatureError {
            detail: "no certificates provided".into(),
        });
    }
    let signer_cert = x509_cert::Certificate::from_der(cert_chain_der[0]).map_err(|e| {
        JustPdfError::SignatureError {
            detail: format!("failed to parse certificate: {}", e),
        }
    })?;

    let signing_time = SigningTime::now();

    // Build the PDF with signature placeholder
    let Placeholder {
        pdf: mut pdf_bytes,
        byte_range_offset,
        contents_offset,
        contents_length,
    } = build_pdf_with_placeholder(pdf_data, options, &signing_time)?;

    // The byte range leaves out the whole `/Contents` string, delimiters
    // included, and is written into the file before the digest is taken.
    let byte_range = [
        0i64,
        contents_offset as i64,
        (contents_offset + contents_length) as i64,
        (pdf_bytes.len() - contents_offset - contents_length) as i64,
    ];
    let byte_range_text = format!(
        "[0 {:010} {:010} {:010}]",
        byte_range[1], byte_range[2], byte_range[3]
    );
    if byte_range_text.len() != BYTE_RANGE_PLACEHOLDER.len() {
        return Err(JustPdfError::SignatureError {
            detail: format!("/ByteRange {byte_range_text} does not fit its placeholder"),
        });
    }
    pdf_bytes[byte_range_offset..byte_range_offset + byte_range_text.len()]
        .copy_from_slice(byte_range_text.as_bytes());

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
        &signing_time,
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

    // Insert CMS blob into placeholder, between the delimiters
    let mut result = pdf_bytes;
    let hex_string = hex_encode(&cms_blob, PLACEHOLDER_SIZE);
    result[contents_offset + 1..contents_offset + contents_length - 1]
        .copy_from_slice(hex_string.as_bytes());

    Ok(result)
}

/// A PDF with an appended signature dictionary whose `/ByteRange` and
/// `/Contents` are placeholders.
#[derive(Debug)]
struct Placeholder {
    pdf: Vec<u8>,
    /// Where the `/ByteRange` array's `[` is.
    byte_range_offset: usize,
    /// Where the `/Contents` hex string's `<` is.
    contents_offset: usize,
    /// The length of the `/Contents` hex string, `<` and `>` included.
    contents_length: usize,
}

/// Build a PDF with an incremental update containing the signature placeholder.
fn build_pdf_with_placeholder(
    pdf_data: &[u8],
    options: &SigningOptions,
    signing_time: &SigningTime,
) -> Result<Placeholder> {
    let mut buf = pdf_data.to_vec();
    if !buf.ends_with(b"\n") {
        buf.push(b'\n');
    }
    let old_startxref = crate::xref::find_startxref(pdf_data)?;

    // Parse the existing document to get catalog info
    let doc = crate::parser::PdfDocument::from_bytes(pdf_data.to_vec())?;
    if doc.is_encrypted() {
        return Err(JustPdfError::UnsupportedEncryption {
            detail: "signing an encrypted document is not supported".into(),
        });
    }
    let catalog_ref = doc
        .catalog_ref()
        .ok_or(JustPdfError::TrailerNotFound)?
        .clone();
    let pages = crate::page::collect_pages(&doc)?;
    let page_ref = pages
        .get(options.page)
        .ok_or(JustPdfError::PageOutOfRange {
            index: options.page,
            count: pages.len(),
        })?
        .page_ref
        .clone();

    // New objects are numbered above every number the source uses
    let first_new = doc
        .object_refs()
        .map(|r| r.obj_num)
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .filter(|n| n.checked_add(2).is_some())
        .ok_or_else(|| JustPdfError::SignatureError {
            detail: "no object numbers left for the signature".into(),
        })?;
    let sig_value_num = first_new;
    let sig_field_num = first_new + 1;
    let ap_stream_num = first_new + 2; // for visible signature appearance
    let field_ref = IndirectRef {
        obj_num: sig_field_num,
        gen_num: 0,
    };

    // The revised objects that link the field into the document
    let Linked {
        revisions,
        field_name,
    } = link_field(&doc, &catalog_ref, &page_ref, &field_ref)?;

    // Create the signature value dictionary content
    let signer_name = options.signer_name.as_deref().unwrap_or("justpdf");
    let reason = options.reason.as_deref().unwrap_or("");
    let location = options.location.as_deref().unwrap_or("");

    // Build the signature value dict with placeholder
    // We need to track where /Contents appears in the output
    let mut offsets = Vec::new();

    // Write sig value object
    let sig_val_offset = buf.len();
    writeln!(buf, "{} 0 obj", sig_value_num)?;
    write!(
        buf,
        "<< /Type /Sig /Filter /Adobe.PPKLite /SubFilter /adbe.pkcs7.detached "
    )?;

    // ByteRange placeholder, as wide as the values `sign_pdf` writes over it
    write!(buf, "/ByteRange ")?;
    let byte_range_offset = buf.len();
    write!(buf, "{BYTE_RANGE_PLACEHOLDER} ")?;

    // /Contents placeholder — this is the critical part
    write!(buf, "/Contents ")?;
    let contents_offset = buf.len();
    buf.push(b'<');
    buf.extend(std::iter::repeat_n(b'0', PLACEHOLDER_SIZE * 2));
    buf.push(b'>');
    let contents_length = buf.len() - contents_offset;
    write!(buf, " ")?;

    write!(buf, "/Name {} ", string_syntax(signer_name.as_bytes()))?;
    write!(
        buf,
        "/M {} ",
        string_syntax(signing_time.pdf_date().as_bytes())
    )?;
    if !reason.is_empty() {
        write!(buf, "/Reason {} ", string_syntax(reason.as_bytes()))?;
    }
    if !location.is_empty() {
        write!(buf, "/Location {} ", string_syntax(location.as_bytes()))?;
    }
    if let Some(contact_info) = options.contact_info.as_deref() {
        write!(
            buf,
            "/ContactInfo {} ",
            string_syntax(contact_info.as_bytes())
        )?;
    }
    write!(buf, ">>\nendobj\n")?;
    offsets.push((sig_value_num, 0, sig_val_offset));

    // Write signature field
    let sig_field_offset = buf.len();
    writeln!(buf, "{} 0 obj", sig_field_num)?;
    write!(buf, "<< /Type /Annot /Subtype /Widget /FT /Sig ")?;
    write!(buf, "/T {} ", string_syntax(field_name.as_bytes()))?;
    write!(buf, "/V {} 0 R ", sig_value_num)?;
    write!(buf, "/P {} {} R ", page_ref.obj_num, page_ref.gen_num)?;

    if options.visible {
        let rect = options.appearance_rect.unwrap_or([0.0, 0.0, 200.0, 80.0]);
        let [llx, lly, urx, ury] = rect.map(real_syntax);
        write!(buf, "/Rect [{llx} {lly} {urx} {ury}] ")?;
        write!(buf, "/AP << /N {} 0 R >> ", ap_stream_num)?;
        write!(buf, "/F 4 ")?; // Print
    } else {
        write!(buf, "/Rect [0 0 0 0] ")?;
        write!(buf, "/F 132 ")?; // Print + Locked
    }

    write!(buf, ">>\nendobj\n")?;
    offsets.push((sig_field_num, 0, sig_field_offset));

    // Write appearance stream (if visible)
    if options.visible {
        let rect = options.appearance_rect.unwrap_or([0.0, 0.0, 200.0, 80.0]);
        let ap_width = rect[2] - rect[0];
        let ap_height = rect[3] - rect[1];
        let (ap_dict, ap_data) = super::appearance::generate_signature_appearance(
            signer_name,
            if reason.is_empty() {
                None
            } else {
                Some(reason)
            },
            None, // date is embedded in the CMS signing time
            ap_width,
            ap_height,
        );

        let ap_offset = buf.len();
        writeln!(buf, "{} 0 obj", ap_stream_num)?;
        // make_stream already includes /Length and /Filter
        serialize_dict(&mut buf, &ap_dict)?;
        writeln!(buf)?;
        write!(buf, "stream\r\n")?;
        buf.extend_from_slice(&ap_data);
        write!(buf, "\r\nendstream\nendobj\n")?;
        offsets.push((ap_stream_num, 0, ap_offset));
    }

    // Write the revised catalog, AcroForm, page and Annots objects
    for (r, obj) in &revisions {
        offsets.push((r.obj_num, r.gen_num, buf.len()));
        writeln!(buf, "{} {} obj", r.obj_num, r.gen_num)?;
        crate::writer::serialize::serialize_object(&mut buf, obj)?;
        write!(buf, "\nendobj\n")?;
    }

    // Write new xref
    let xref_offset = buf.len();
    writeln!(buf, "xref")?;

    // Write subsections
    offsets.sort_by_key(|(obj_num, _, _)| *obj_num);
    for (obj_num, gen_num, offset) in &offsets {
        writeln!(buf, "{} 1", obj_num)?;
        writeln!(buf, "{:010} {:05} n ", offset, gen_num)?;
    }

    // Trailer
    let xref_size = offsets.last().map_or(first_new, |(n, _, _)| *n) + 1;
    let trailer = crate::writer::modify::incremental_trailer(
        doc.trailer(),
        xref_size,
        &catalog_ref,
        None,
        old_startxref,
    );
    writeln!(buf, "trailer")?;
    crate::writer::serialize::serialize_dict(&mut buf, &trailer)?;
    write!(buf, "\nstartxref\n{}\n%%EOF\n", xref_offset)?;

    Ok(Placeholder {
        pdf: buf,
        byte_range_offset,
        contents_offset,
        contents_length,
    })
}

/// A PDF text string as text: UTF-16BE after a byte order mark, else each
/// byte as one character.
fn text_string(bytes: &[u8]) -> String {
    match bytes.strip_prefix(&[0xFE, 0xFF]) {
        Some(utf16) => char::decode_utf16(
            utf16
                .chunks_exact(2)
                .map(|pair| u16::from_be_bytes([pair[0], pair[1]])),
        )
        .map(|c| c.unwrap_or(char::REPLACEMENT_CHARACTER))
        .collect(),
        None => bytes.iter().map(|&b| b as char).collect(),
    }
}

/// The objects `link_field` revises, and the name it gave the field.
struct Linked {
    revisions: Vec<(IndirectRef, PdfObject)>,
    field_name: String,
}

/// New revisions of the objects that put the field `field_ref` into the
/// document: its reference appended to `/AcroForm /Fields` (an `/AcroForm`
/// created when the catalog has none, an indirect `/Fields` written out as a
/// direct array) with `/SigFlags` 1 and 2 set, and appended to the `/Annots`
/// of the page `page_ref`. An indirect `/AcroForm` or `/Annots` is revised at
/// its own number only when it resolves to a dictionary or an array
/// respectively; otherwise (an undefined object resolves to null) it reads as
/// absent and the catalog or page is revised instead. The field is named `SignatureN`, the first N no top-level
/// field's `/T` uses.
fn link_field(
    doc: &crate::parser::PdfDocument,
    catalog_ref: &IndirectRef,
    page_ref: &IndirectRef,
    field_ref: &IndirectRef,
) -> Result<Linked> {
    let resolve_dict = |r: &IndirectRef| -> Result<PdfDict> {
        Ok(doc.resolve(r)?.as_dict().cloned().unwrap_or_default())
    };
    let resolve_array = |obj: Option<&PdfObject>| -> Result<Vec<PdfObject>> {
        Ok(match obj {
            Some(PdfObject::Array(items)) => items.clone(),
            Some(PdfObject::Reference(r)) => match doc.resolve(r)? {
                PdfObject::Array(items) => items,
                _ => Vec::new(),
            },
            _ => Vec::new(),
        })
    };

    // An indirect value at `key` of `parent` that resolves to an object of
    // the kind `is_kind` accepts.
    let defined = |parent: &PdfDict, key: &[u8], is_kind: fn(&PdfObject) -> bool| {
        let r = parent.get_ref(key)?;
        doc.resolve(r)
            .ok()
            .filter(is_kind)
            .map(|obj| (r.clone(), obj))
    };

    let mut revisions = Vec::new();
    let mut catalog = resolve_dict(catalog_ref)?;

    let acroform_obj = defined(&catalog, b"AcroForm", |o| o.as_dict().is_some());
    let acroform_ref = acroform_obj.as_ref().map(|(r, _)| r.clone());
    let mut acroform = match (acroform_obj, catalog.get(b"AcroForm")) {
        (Some((_, obj)), _) => obj.as_dict().cloned().unwrap_or_default(),
        (None, Some(PdfObject::Dict(d))) => d.clone(),
        _ => PdfDict::new(),
    };
    let mut fields = resolve_array(acroform.get(b"Fields"))?;

    let mut taken = std::collections::HashSet::new();
    for field in &fields {
        let field = match field {
            PdfObject::Reference(r) => resolve_dict(r)?,
            PdfObject::Dict(d) => d.clone(),
            _ => continue,
        };
        if let Some(name) = field.get(b"T").and_then(PdfObject::as_str) {
            taken.insert(text_string(name));
        }
    }
    let field_name = (1..)
        .map(|n| format!("Signature{n}"))
        .find(|name| !taken.contains(name))
        .unwrap_or_default();

    fields.push(PdfObject::Reference(field_ref.clone()));
    acroform.insert(b"Fields".to_vec(), PdfObject::Array(fields));
    let sig_flags = acroform.get_i64(b"SigFlags").unwrap_or(0) | 3;
    acroform.insert(b"SigFlags".to_vec(), PdfObject::Integer(sig_flags));
    match acroform_ref {
        Some(r) => revisions.push((r, PdfObject::Dict(acroform))),
        None => {
            catalog.insert(b"AcroForm".to_vec(), PdfObject::Dict(acroform));
            revisions.push((catalog_ref.clone(), PdfObject::Dict(catalog)));
        }
    }

    let mut page = resolve_dict(page_ref)?;
    let annots_obj = defined(&page, b"Annots", |o| matches!(o, PdfObject::Array(_)));
    let mut annots = match &annots_obj {
        Some((_, PdfObject::Array(items))) => items.clone(),
        Some(_) => Vec::new(),
        None => match page.get(b"Annots") {
            Some(PdfObject::Array(items)) => items.clone(),
            _ => Vec::new(),
        },
    };
    annots.push(PdfObject::Reference(field_ref.clone()));
    match annots_obj.map(|(r, _)| r) {
        Some(r) => revisions.push((r, PdfObject::Array(annots))),
        None => {
            page.insert(b"Annots".to_vec(), PdfObject::Array(annots));
            revisions.push((page_ref.clone(), PdfObject::Dict(page)));
        }
    }

    Ok(Linked {
        revisions,
        field_name,
    })
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
    signing_time: &SigningTime,
) -> Result<Vec<u8>> {
    use rsa::pkcs1v15::SigningKey;
    use signature::Signer;

    let digest_oid = match algorithm {
        DigestAlgorithm::Sha256 => {
            const_oid::ObjectIdentifier::new_unwrap("2.16.840.1.101.3.4.2.1")
        }
        DigestAlgorithm::Sha384 => {
            const_oid::ObjectIdentifier::new_unwrap("2.16.840.1.101.3.4.2.2")
        }
        DigestAlgorithm::Sha512 => {
            const_oid::ObjectIdentifier::new_unwrap("2.16.840.1.101.3.4.2.3")
        }
    };

    let rsa_oid = const_oid::ObjectIdentifier::new_unwrap("1.2.840.113549.1.1.1");
    let signed_data_oid = const_oid::ObjectIdentifier::new_unwrap("1.2.840.113549.1.7.2");
    let data_oid = const_oid::ObjectIdentifier::new_unwrap("1.2.840.113549.1.7.1");
    let message_digest_oid = const_oid::ObjectIdentifier::new_unwrap("1.2.840.113549.1.9.4");
    let content_type_oid = const_oid::ObjectIdentifier::new_unwrap("1.2.840.113549.1.9.3");
    let signing_time_oid = const_oid::ObjectIdentifier::new_unwrap("1.2.840.113549.1.9.5");

    // Build signed attributes
    let digest_octet =
        der::asn1::OctetString::new(digest).map_err(|e| JustPdfError::SignatureError {
            detail: format!("OctetString error: {}", e),
        })?;

    // Create the signed attributes DER manually for signing
    // (contentType, signingTime, messageDigest)
    let content_type_attr_value = data_oid
        .to_der()
        .map_err(|e| JustPdfError::SignatureError {
            detail: format!("DER error: {}", e),
        })?;
    let digest_attr_value = digest_octet
        .to_der()
        .map_err(|e| JustPdfError::SignatureError {
            detail: format!("DER error: {}", e),
        })?;

    // Build signed attributes as a SET
    let mut signed_attrs_content = Vec::new();

    // contentType attribute
    append_attribute(
        &mut signed_attrs_content,
        &content_type_oid,
        &content_type_attr_value,
    )?;
    // signingTime attribute (UTCTime)
    let signing_time_der = signing_time.utc_time_der();
    append_attribute(
        &mut signed_attrs_content,
        &signing_time_oid,
        &signing_time_der,
    )?;
    // messageDigest attribute
    append_attribute(
        &mut signed_attrs_content,
        &message_digest_oid,
        &digest_attr_value,
    )?;

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
    let eci_oid = data_oid
        .to_der()
        .map_err(|e| JustPdfError::SignatureError {
            detail: format!("DER error: {}", e),
        })?;
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
    let sd_oid = signed_data_oid
        .to_der()
        .map_err(|e| JustPdfError::SignatureError {
            detail: format!("DER error: {}", e),
        })?;

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
    let issuer_der =
        signer_cert
            .tbs_certificate
            .issuer
            .to_der()
            .map_err(|e| JustPdfError::SignatureError {
                detail: format!("DER error: {}", e),
            })?;
    let serial_der = signer_cert
        .tbs_certificate
        .serial_number
        .to_der()
        .map_err(|e| JustPdfError::SignatureError {
            detail: format!("DER error: {}", e),
        })?;
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

// --- DER helpers ---

/// A moment in UTC, to the second.
struct SigningTime {
    year: u64,
    month: u64,
    day: u64,
    hour: u64,
    minute: u64,
    second: u64,
}

impl SigningTime {
    /// The system clock's current time.
    fn now() -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        Self::from_unix(secs)
    }

    /// The time `secs` seconds after 1970-01-01T00:00:00Z.
    fn from_unix(secs: u64) -> Self {
        let mut days = secs / 86400;
        let time_of_day = secs % 86400;
        let leap =
            |y: u64| y.is_multiple_of(4) && (!y.is_multiple_of(100) || y.is_multiple_of(400));
        let mut year = 1970u64;
        loop {
            let days_in_year = if leap(year) { 366 } else { 365 };
            if days < days_in_year {
                break;
            }
            days -= days_in_year;
            year += 1;
        }
        let month_days = [
            31,
            if leap(year) { 29 } else { 28 },
            31,
            30,
            31,
            30,
            31,
            31,
            30,
            31,
            30,
            31,
        ];
        let mut month = 1;
        for md in month_days {
            if days < md {
                break;
            }
            days -= md;
            month += 1;
        }
        Self {
            year,
            month,
            day: days + 1,
            hour: time_of_day / 3600,
            minute: time_of_day % 3600 / 60,
            second: time_of_day % 60,
        }
    }

    /// A DER UTCTime (two-digit year).
    fn utc_time_der(&self) -> Vec<u8> {
        let text = format!(
            "{:02}{:02}{:02}{:02}{:02}{:02}Z",
            self.year % 100,
            self.month,
            self.day,
            self.hour,
            self.minute,
            self.second
        );
        let mut result = vec![0x17, text.len() as u8];
        result.extend_from_slice(text.as_bytes());
        result
    }

    /// A PDF date string, `D:YYYYMMDDHHmmSSZ`.
    fn pdf_date(&self) -> String {
        format!(
            "D:{:04}{:02}{:02}{:02}{:02}{:02}Z",
            self.year, self.month, self.day, self.hour, self.minute, self.second
        )
    }
}

fn build_algorithm_identifier(oid: &const_oid::ObjectIdentifier) -> Result<Vec<u8>> {
    let oid_der = oid.to_der().map_err(|e| JustPdfError::SignatureError {
        detail: format!("DER error: {}", e),
    })?;
    // SEQUENCE { OID, NULL }
    let mut content = Vec::new();
    content.extend_from_slice(&oid_der);
    content.extend_from_slice(&[0x05, 0x00]); // NULL
    Ok(wrap_der_sequence(&content))
}

fn append_attribute(
    buf: &mut Vec<u8>,
    oid: &const_oid::ObjectIdentifier,
    value: &[u8],
) -> Result<()> {
    let oid_der = oid.to_der().map_err(|e| JustPdfError::SignatureError {
        detail: format!("DER error: {}", e),
    })?;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn create_pdf(encrypted: bool) -> Vec<u8> {
        use crate::writer::document::DocumentBuilder;
        use crate::writer::page::PageBuilder;
        let mut doc = DocumentBuilder::new();
        let font = doc.add_standard_font("Helvetica");
        let mut page = PageBuilder::new(612.0, 792.0);
        page.begin_text();
        page.set_font(&font, 12.0);
        page.move_to(72.0, 720.0);
        page.show_text("Signed page");
        page.end_text();
        doc.add_page(page);
        doc.set_title("Signed document");
        if encrypted {
            doc.set_encryption(crate::crypto::EncryptionConfig {
                user_password: Vec::new(),
                owner_password: b"owner".to_vec(),
                permissions: crate::crypto::Permissions::allow_all(),
                method: crate::crypto::EncryptionMethod::AES128,
                encrypt_metadata: true,
            });
        }
        doc.build().unwrap()
    }

    #[test]
    fn test_placeholder_rejects_encrypted_input() {
        let pdf = create_pdf(true);
        let err = build_pdf_with_placeholder(
            &pdf,
            &SigningOptions::default(),
            &SigningTime::from_unix(0),
        )
        .unwrap_err();
        assert!(
            matches!(err, JustPdfError::UnsupportedEncryption { .. }),
            "got {err:?}"
        );
    }

    #[test]
    fn test_placeholder_keeps_trailer_keys() {
        let pdf = create_pdf(false);
        let original = crate::parser::PdfDocument::from_bytes(pdf.clone()).unwrap();
        assert!(original.trailer().get(b"Info").is_some());

        let signed = build_pdf_with_placeholder(
            &pdf,
            &SigningOptions::default(),
            &SigningTime::from_unix(0),
        )
        .unwrap()
        .pdf;
        let reopened = crate::parser::PdfDocument::from_bytes(signed).unwrap();
        assert_eq!(
            reopened.trailer().get(b"Info"),
            original.trailer().get(b"Info")
        );
        assert_eq!(
            reopened.trailer().get(b"Root"),
            original.trailer().get(b"Root")
        );
        assert!(reopened.trailer().get(b"Prev").is_some());
    }

    #[test]
    fn test_placeholder_xref_entries_are_20_bytes() {
        let pdf = create_pdf(false);
        let signed = build_pdf_with_placeholder(
            &pdf,
            &SigningOptions::default(),
            &SigningTime::from_unix(0),
        )
        .unwrap()
        .pdf;
        let start = signed.windows(6).rposition(|w| w == b"\nxref\n").unwrap() + 6;
        let end = start
            + signed[start..]
                .windows(7)
                .position(|w| w == b"trailer")
                .unwrap();
        let mut entries = 0;
        for line in signed[start..end].split_inclusive(|&b| b == b'\n') {
            if line.len() == 20 && line[17] == b'n' {
                assert!(matches!(&line[18..], b" \r" | b" \n" | b"\r\n"), "{line:?}");
                entries += 1;
            } else {
                assert!(
                    line.iter()
                        .take_while(|&&b| b != b'\n')
                        .all(|&b| b.is_ascii_digit() || b == b' '),
                    "not a 20-byte entry or a subsection header: {:?}",
                    String::from_utf8_lossy(line)
                );
            }
        }
        assert!(entries > 0);
    }

    #[test]
    fn test_placeholder_values_read_back_unchanged() {
        use crate::object::PdfObject;
        let pdf = create_pdf(false);
        let first_new = crate::parser::PdfDocument::from_bytes(pdf.clone())
            .unwrap()
            .object_refs()
            .map(|r| r.obj_num)
            .max()
            .unwrap()
            + 1;
        let options = SigningOptions {
            signer_name: Some("A) B\r".into()),
            reason: Some("R(\u{e9}".into()),
            location: Some("L\\".into()),
            visible: true,
            appearance_rect: Some([0.0, f64::NAN, f64::INFINITY, 80.0]),
            ..SigningOptions::default()
        };

        let signed = build_pdf_with_placeholder(&pdf, &options, &SigningTime::from_unix(0))
            .unwrap()
            .pdf;
        let reopened = crate::parser::PdfDocument::from_bytes(signed).unwrap();
        let object = |n: u32| {
            reopened
                .resolve(&crate::object::IndirectRef {
                    obj_num: n,
                    gen_num: 0,
                })
                .unwrap()
        };

        let value = object(first_new);
        let value = value.as_dict().unwrap();
        assert_eq!(value.get_string(b"Name"), Some(b"A) B\r".as_slice()));
        assert_eq!(value.get_string(b"Reason"), Some("R(\u{e9}".as_bytes()));
        assert_eq!(value.get_string(b"Location"), Some(b"L\\".as_slice()));

        let field = object(first_new + 1);
        let max = f64::from(f32::MAX);
        let rect = [0.0, 0.0, max, 80.0].map(PdfObject::Real).to_vec();
        assert_eq!(
            field.as_dict().unwrap().get(b"Rect"),
            Some(&PdfObject::Array(rect))
        );

        let PdfObject::Stream { dict, .. } = object(first_new + 2) else {
            panic!("expected a stream")
        };
        assert_eq!(dict.get_name(b"Subtype"), Some(b"Form".as_slice()));
        let bbox = vec![
            PdfObject::Integer(0),
            PdfObject::Integer(0),
            PdfObject::Real(max),
            PdfObject::Real(0.0),
        ];
        assert_eq!(dict.get(b"BBox"), Some(&PdfObject::Array(bbox)));
    }

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
