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
        buf.push(b'/');
        write_escaped_name(buf, key);
        buf.push(b' ');
        serialize_object(buf, val)?;
        write!(buf, " ")?;
    }
    write!(buf, ">>")?;
    Ok(())
}

/// Serialize a PDF using cross-reference stream (PDF 1.5+).
///
/// This variant is used when object streams are present. It writes the body
/// objects normally, then writes a cross-reference stream instead of a
/// traditional xref table + trailer.
pub fn serialize_pdf_with_xref_stream(
    objects: &[(u32, PdfObject)],
    compressed: &[crate::writer::object_stream::CompressedObjInfo],
    version: (u8, u8),
    catalog_ref: &IndirectRef,
    info_ref: Option<&IndirectRef>,
) -> Result<Vec<u8>> {
    let mut buf: Vec<u8> = Vec::new();

    // --- Header ---
    let ver_major = version.0.max(1);
    let ver_minor = version.1.max(5); // at least 1.5 for object streams
    write!(buf, "%PDF-{}.{}\n", ver_major, ver_minor)?;
    buf.extend_from_slice(b"%\xe2\xe3\xcf\xd3\n");

    // --- Body: write each indirect object ---
    let mut offsets: Vec<(u32, usize)> = Vec::with_capacity(objects.len());

    for (obj_num, obj) in objects {
        let offset = buf.len();
        offsets.push((*obj_num, offset));

        write!(buf, "{} 0 obj\n", obj_num)?;
        serialize_object(&mut buf, obj)?;
        write!(buf, "\nendobj\n")?;
    }

    // --- Cross-reference stream ---
    let max_obj_num = offsets
        .iter()
        .map(|(n, _)| *n)
        .max()
        .unwrap_or(0)
        .max(compressed.iter().map(|c| c.obj_num).max().unwrap_or(0));
    let xref_stm_obj_num = max_obj_num + 1;

    crate::writer::object_stream::write_xref_stream(
        &mut buf,
        &offsets,
        compressed,
        catalog_ref,
        info_ref,
        xref_stm_obj_num,
    )?;

    Ok(buf)
}

/// Write a PDF name with proper #XX escaping for special characters.
fn write_escaped_name(buf: &mut Vec<u8>, name: &[u8]) {
    for &byte in name {
        if byte == b'#'
            || byte == b'/'
            || byte == b'('
            || byte == b')'
            || byte == b'<'
            || byte == b'>'
            || byte == b'['
            || byte == b']'
            || byte == b'{'
            || byte == b'}'
            || byte == b'%'
            || byte <= b' '
            || byte >= 127
        {
            buf.push(b'#');
            let hex = format!("{:02X}", byte);
            buf.extend_from_slice(hex.as_bytes());
        } else {
            buf.push(byte);
        }
    }
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

    // ── Name escaping in serialization ──────────────────────────────

    #[test]
    fn test_serialize_dict_with_space_in_name_value() {
        // Simulate a font dict with BaseFont "Pretendard Black"
        let mut font_dict = PdfDict::new();
        font_dict.insert(b"Type".to_vec(), PdfObject::Name(b"Font".to_vec()));
        font_dict.insert(b"Subtype".to_vec(), PdfObject::Name(b"Type0".to_vec()));
        font_dict.insert(
            b"BaseFont".to_vec(),
            PdfObject::Name(b"Pretendard Black".to_vec()),
        );

        let mut buf = Vec::new();
        serialize_dict(&mut buf, &font_dict).unwrap();
        let text = String::from_utf8_lossy(&buf);

        // The space in "Pretendard Black" must be escaped
        assert!(
            text.contains("Pretendard#20Black"),
            "BaseFont name must escape space: {}",
            text,
        );
        assert!(
            !text.contains("Pretendard Black"),
            "Raw space must not appear in serialized Name",
        );
    }

    #[test]
    fn test_serialize_dict_key_with_space() {
        let mut d = PdfDict::new();
        d.insert(b"My Key".to_vec(), PdfObject::Integer(42));

        let mut buf = Vec::new();
        serialize_dict(&mut buf, &d).unwrap();
        let text = String::from_utf8_lossy(&buf);

        assert!(
            text.contains("/My#20Key"),
            "Dict key with space must be escaped: {}",
            text,
        );
    }

    #[test]
    fn test_serialize_name_escape_function() {
        let mut buf = Vec::new();
        write_escaped_name(&mut buf, b"Hello World");
        assert_eq!(String::from_utf8(buf).unwrap(), "Hello#20World");

        let mut buf = Vec::new();
        write_escaped_name(&mut buf, b"Normal");
        assert_eq!(String::from_utf8(buf).unwrap(), "Normal");

        let mut buf = Vec::new();
        write_escaped_name(&mut buf, b"A#B");
        assert_eq!(String::from_utf8(buf).unwrap(), "A#23B");

        let mut buf = Vec::new();
        write_escaped_name(&mut buf, &[0xFF, 0x00]);
        assert_eq!(String::from_utf8(buf).unwrap(), "#FF#00");
    }

    #[test]
    fn test_serialize_pdf_with_space_font_roundtrip() {
        // Create a PDF with a font whose BaseFont has a space
        let mut font_dict = PdfDict::new();
        font_dict.insert(b"Type".to_vec(), PdfObject::Name(b"Font".to_vec()));
        font_dict.insert(b"Subtype".to_vec(), PdfObject::Name(b"Type0".to_vec()));
        font_dict.insert(
            b"BaseFont".to_vec(),
            PdfObject::Name(b"Noto Sans KR".to_vec()),
        );
        font_dict.insert(
            b"Encoding".to_vec(),
            PdfObject::Name(b"Identity-H".to_vec()),
        );

        let mut catalog = PdfDict::new();
        catalog.insert(b"Type".to_vec(), PdfObject::Name(b"Catalog".to_vec()));

        let catalog_ref = IndirectRef { obj_num: 1, gen_num: 0 };
        let objects = vec![
            (1, PdfObject::Dict(catalog)),
            (2, PdfObject::Dict(font_dict)),
        ];

        let bytes = serialize_pdf(&objects, (1, 7), &catalog_ref, None).unwrap();

        // Re-parse
        let doc = crate::parser::PdfDocument::from_bytes(bytes).unwrap();
        let font_ref = IndirectRef { obj_num: 2, gen_num: 0 };
        let font_obj = doc.resolve(&font_ref).unwrap();

        if let PdfObject::Dict(d) = &font_obj {
            let basefont = d.get_name(b"BaseFont").unwrap();
            assert_eq!(
                basefont,
                b"Noto Sans KR",
                "BaseFont with spaces must survive roundtrip",
            );
        } else {
            panic!("Font object should be a Dict");
        }
    }

    #[test]
    fn test_serialize_string_with_parens_roundtrip() {
        let mut catalog = PdfDict::new();
        catalog.insert(b"Type".to_vec(), PdfObject::Name(b"Catalog".to_vec()));

        let catalog_ref = IndirectRef { obj_num: 1, gen_num: 0 };
        let objects = vec![
            (1, PdfObject::Dict(catalog)),
            (2, PdfObject::String(b"hello(world)end".to_vec())),
        ];

        let bytes = serialize_pdf(&objects, (1, 7), &catalog_ref, None).unwrap();

        let doc = crate::parser::PdfDocument::from_bytes(bytes).unwrap();
        let ref2 = IndirectRef { obj_num: 2, gen_num: 0 };
        let obj = doc.resolve(&ref2).unwrap();

        if let PdfObject::String(s) = &obj {
            assert_eq!(s, b"hello(world)end", "String with parens must survive roundtrip");
        } else {
            panic!("Expected String, got {:?}", obj);
        }
    }
}
