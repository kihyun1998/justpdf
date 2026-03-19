use std::io::Write;

use crate::error::Result;
use crate::object::{IndirectRef, PdfDict, PdfObject};

/// Serialize a collection of PDF objects into a complete, valid PDF byte stream.
///
/// `objects` contains `(obj_num, PdfObject)` pairs.
/// `version` is the PDF version, e.g. `(1, 7)`.
/// `catalog_ref` points to the document catalog object.
/// `info_ref` optionally points to the document info dictionary.
pub fn serialize_pdf(
    objects: &[(u32, PdfObject)],
    version: (u8, u8),
    catalog_ref: &IndirectRef,
    info_ref: Option<&IndirectRef>,
) -> Result<Vec<u8>> {
    serialize_pdf_impl(objects, version, catalog_ref, info_ref, None, None)
}

/// Serialize a PDF with encryption.
///
/// `encrypt_ref` and `encrypt_state` together drive per-object encryption
/// and add /Encrypt and /ID to the trailer.
pub fn serialize_pdf_encrypted(
    objects: &[(u32, PdfObject)],
    version: (u8, u8),
    catalog_ref: &IndirectRef,
    info_ref: Option<&IndirectRef>,
    encrypt_ref: &IndirectRef,
    encrypt_state: &crate::crypto::SecurityState,
    id_array: &[PdfObject],
) -> Result<Vec<u8>> {
    serialize_pdf_impl(
        objects,
        version,
        catalog_ref,
        info_ref,
        Some((encrypt_ref, encrypt_state, id_array)),
        None,
    )
}

/// Internal implementation handling both encrypted and unencrypted serialization.
fn serialize_pdf_impl(
    objects: &[(u32, PdfObject)],
    version: (u8, u8),
    catalog_ref: &IndirectRef,
    info_ref: Option<&IndirectRef>,
    encryption: Option<(&IndirectRef, &crate::crypto::SecurityState, &[PdfObject])>,
    _extra_trailer: Option<&PdfDict>,
) -> Result<Vec<u8>> {
    let mut buf: Vec<u8> = Vec::new();

    // --- Header ---
    write!(buf, "%PDF-{}.{}\n", version.0, version.1)?;
    // Binary marker: four bytes > 127 to signal binary content
    buf.extend_from_slice(b"%\xe2\xe3\xcf\xd3\n");

    // --- Body: write each indirect object ---
    // Track byte offsets for xref
    let mut offsets: Vec<(u32, usize)> = Vec::with_capacity(objects.len());

    for (obj_num, obj) in objects {
        let offset = buf.len();
        offsets.push((*obj_num, offset));

        // Encrypt the object if needed
        let write_obj = if let Some((encrypt_ref, state, _)) = &encryption {
            if *obj_num == encrypt_ref.obj_num {
                // Don't encrypt the encryption dictionary itself
                obj.clone()
            } else {
                crate::crypto::encrypt_object(obj, state, *obj_num, 0)?
            }
        } else {
            obj.clone()
        };

        write!(buf, "{} 0 obj\n", obj_num)?;
        serialize_object(&mut buf, &write_obj)?;
        write!(buf, "\nendobj\n")?;
    }

    // --- Cross-reference table ---
    let xref_offset = buf.len();

    // Determine size: max obj_num + 1
    let max_obj_num = offsets.iter().map(|(n, _)| *n).max().unwrap_or(0);
    let xref_size = max_obj_num + 1;

    write!(buf, "xref\n")?;
    write!(buf, "0 {}\n", xref_size)?;

    // Entry 0: free list head
    buf.extend_from_slice(b"0000000000 65535 f \r\n");

    // Build a map for quick lookup
    let mut offset_map = std::collections::HashMap::new();
    for (num, off) in &offsets {
        offset_map.insert(*num, *off);
    }

    // Entries 1..xref_size
    for obj_num in 1..xref_size {
        if let Some(&off) = offset_map.get(&obj_num) {
            write!(buf, "{:010} {:05} n \r\n", off, 0)?;
        } else {
            // Free entry
            buf.extend_from_slice(b"0000000000 00000 f \r\n");
        }
    }

    // --- Trailer ---
    let mut trailer = PdfDict::new();
    trailer.insert(b"Size".to_vec(), PdfObject::Integer(xref_size as i64));
    trailer.insert(
        b"Root".to_vec(),
        PdfObject::Reference(catalog_ref.clone()),
    );
    if let Some(info) = info_ref {
        trailer.insert(b"Info".to_vec(), PdfObject::Reference(info.clone()));
    }

    // Add encryption entries to trailer
    if let Some((encrypt_ref, _, id_array)) = &encryption {
        trailer.insert(
            b"Encrypt".to_vec(),
            PdfObject::Reference((*encrypt_ref).clone()),
        );
        trailer.insert(
            b"ID".to_vec(),
            PdfObject::Array(id_array.to_vec()),
        );
    }

    write!(buf, "trailer\n")?;
    serialize_dict(&mut buf, &trailer)?;
    write!(buf, "\n")?;

    // --- Startxref ---
    write!(buf, "startxref\n{}\n%%EOF\n", xref_offset)?;

    Ok(buf)
}

/// Serialize a single PdfObject into the buffer.
pub(crate) fn serialize_object(buf: &mut Vec<u8>, obj: &PdfObject) -> Result<()> {
    match obj {
        PdfObject::Stream { dict, data } => {
            // Build a dict copy with /Length set
            let mut stream_dict = dict.clone();
            stream_dict.insert(
                b"Length".to_vec(),
                PdfObject::Integer(data.len() as i64),
            );
            serialize_dict(buf, &stream_dict)?;
            buf.extend_from_slice(b"\nstream\r\n");
            buf.extend_from_slice(data);
            buf.extend_from_slice(b"\r\nendstream");
            Ok(())
        }
        _ => {
            // For all non-stream objects, use Display impl
            write!(buf, "{}", obj)?;
            Ok(())
        }
    }
}

/// Serialize a PdfDict in `<< ... >>` format.
pub(crate) fn serialize_dict(buf: &mut Vec<u8>, dict: &PdfDict) -> Result<()> {
    write!(buf, "<< ")?;
    for (key, val) in dict.iter() {
        let key_str = std::str::from_utf8(key).unwrap_or("?");
        write!(buf, "/{} ", key_str)?;
        serialize_object(buf, val)?;
        write!(buf, " ")?;
    }
    write!(buf, ">>")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::object::{IndirectRef, PdfDict, PdfObject};

    #[test]
    fn test_serialize_minimal_pdf() {
        let mut objects = Vec::new();

        // Catalog
        let mut catalog = PdfDict::new();
        catalog.insert(b"Type".to_vec(), PdfObject::Name(b"Catalog".to_vec()));

        let catalog_ref = IndirectRef {
            obj_num: 1,
            gen_num: 0,
        };
        objects.push((1, PdfObject::Dict(catalog)));

        let bytes = serialize_pdf(&objects, (1, 7), &catalog_ref, None).unwrap();
        let text = String::from_utf8_lossy(&bytes);

        assert!(bytes.starts_with(b"%PDF-1.7"));
        assert!(text.contains("1 0 obj"));
        assert!(text.contains("/Type /Catalog"));
        assert!(text.contains("endobj"));
        assert!(text.contains("xref"));
        assert!(text.contains("trailer"));
        assert!(text.contains("startxref"));
        assert!(text.contains("%%EOF"));
    }

    #[test]
    fn test_serialize_stream_object() {
        let mut dict = PdfDict::new();
        dict.insert(
            b"Filter".to_vec(),
            PdfObject::Name(b"FlateDecode".to_vec()),
        );

        let data = b"raw stream data here".to_vec();
        let stream = PdfObject::Stream { dict, data };

        let catalog_ref = IndirectRef {
            obj_num: 1,
            gen_num: 0,
        };
        let objects = vec![(1, stream)];

        let bytes = serialize_pdf(&objects, (1, 7), &catalog_ref, None).unwrap();
        let text = String::from_utf8_lossy(&bytes);

        assert!(text.contains("/Length 20"));
        assert!(text.contains("stream\r\n"));
        assert!(text.contains("raw stream data here"));
        assert!(text.contains("\r\nendstream"));
    }

    #[test]
    fn test_xref_entry_format() {
        let mut catalog = PdfDict::new();
        catalog.insert(b"Type".to_vec(), PdfObject::Name(b"Catalog".to_vec()));

        let catalog_ref = IndirectRef {
            obj_num: 1,
            gen_num: 0,
        };
        let objects = vec![(1, PdfObject::Dict(catalog))];

        let bytes = serialize_pdf(&objects, (1, 7), &catalog_ref, None).unwrap();
        let text = String::from_utf8_lossy(&bytes);

        // Verify xref contains the free entry
        assert!(text.contains("0000000000 65535 f \r\n"));
        // Verify xref contains an in-use entry with 10-digit offset
        assert!(text.contains(" 00000 n \r\n"));
    }

    #[test]
    fn test_serialize_encrypted_pdf() {
        use crate::crypto::{EncryptionConfig, EncryptionMethod, Permissions};

        let config = EncryptionConfig {
            user_password: b"test".to_vec(),
            owner_password: b"owner".to_vec(),
            permissions: Permissions::allow_all(),
            method: EncryptionMethod::RC4_128,
            encrypt_metadata: true,
        };

        let file_id = b"test-serialize-enc";
        let (state, encrypt_dict, id_array) = config.build(file_id).unwrap();

        // Create a minimal document
        let mut objects = Vec::new();

        let mut catalog = PdfDict::new();
        catalog.insert(b"Type".to_vec(), PdfObject::Name(b"Catalog".to_vec()));
        objects.push((1, PdfObject::Dict(catalog)));

        // Add encrypt dict
        let encrypt_ref = IndirectRef {
            obj_num: 2,
            gen_num: 0,
        };
        objects.push((2, PdfObject::Dict(encrypt_dict)));

        // Add a string object that should get encrypted
        objects.push((3, PdfObject::String(b"Hello Secret".to_vec())));

        let catalog_ref = IndirectRef {
            obj_num: 1,
            gen_num: 0,
        };

        let bytes = serialize_pdf_encrypted(
            &objects,
            (1, 7),
            &catalog_ref,
            None,
            &encrypt_ref,
            &state,
            &id_array,
        )
        .unwrap();

        let text = String::from_utf8_lossy(&bytes);

        // Trailer should contain /Encrypt and /ID
        assert!(text.contains("/Encrypt"));
        assert!(text.contains("/ID"));

        // The string "Hello Secret" should NOT appear in plaintext
        // (it should be encrypted)
        // Note: RC4 might produce bytes that happen to look like the original,
        // but statistically very unlikely for a 12-byte string
        assert!(!text.contains("Hello Secret"));
    }
}
