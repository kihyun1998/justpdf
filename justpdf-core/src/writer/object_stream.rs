use std::io::Write;

use crate::error::Result;
use crate::object::{PdfDict, PdfObject};
use crate::writer::encode::encode_flate;
use crate::writer::serialize::serialize_object;

/// Pack eligible objects into object streams for compact PDF 1.5+ output.
///
/// Returns a new list of objects where small non-stream objects have been
/// packed into object stream containers, plus the remaining unpacked objects.
///
/// `catalog_obj_num` and `pages_root_obj_num` identify objects that must NOT
/// be packed (the catalog and pages tree root).
///
/// `encrypt_obj_num` optionally identifies the encryption dictionary, which
/// must also remain unpacked.
/// Information about compressed objects for xref stream generation.
#[derive(Debug, Clone)]
pub struct CompressedObjInfo {
    /// Object number of the compressed object.
    pub obj_num: u32,
    /// Object number of the ObjStm that contains it.
    pub objstm_num: u32,
    /// Index of this object within the ObjStm.
    pub index: u32,
}

/// Result of packing objects into object streams.
pub struct PackResult {
    /// The resulting objects (ineligible + ObjStm containers).
    pub objects: Vec<(u32, PdfObject)>,
    /// Info about objects compressed into ObjStms (for xref stream type 2 entries).
    pub compressed: Vec<CompressedObjInfo>,
}

pub fn pack_object_streams(
    objects: &[(u32, PdfObject)],
    max_objects_per_stream: usize,
    catalog_obj_num: u32,
    pages_root_obj_num: Option<u32>,
    encrypt_obj_num: Option<u32>,
) -> Result<PackResult> {
    let mut eligible: Vec<(u32, &PdfObject)> = Vec::new();
    let mut ineligible: Vec<(u32, PdfObject)> = Vec::new();

    for (obj_num, obj) in objects {
        if is_eligible(*obj_num, obj, catalog_obj_num, pages_root_obj_num, encrypt_obj_num) {
            eligible.push((*obj_num, obj));
        } else {
            ineligible.push((*obj_num, obj.clone()));
        }
    }

    if eligible.is_empty() {
        return Ok(PackResult {
            objects: objects.to_vec(),
            compressed: Vec::new(),
        });
    }

    // Determine next available object number for the new object stream containers.
    let mut next_obj_num = objects.iter().map(|(n, _)| *n).max().unwrap_or(0) + 1;

    // Pack eligible objects in batches
    let mut result = ineligible;
    let mut compressed = Vec::new();

    for chunk in eligible.chunks(max_objects_per_stream) {
        let objstm_num = next_obj_num;
        let objstm = build_object_stream(chunk)?;
        result.push((objstm_num, objstm));

        for (index, (obj_num, _)) in chunk.iter().enumerate() {
            compressed.push(CompressedObjInfo {
                obj_num: *obj_num,
                objstm_num,
                index: index as u32,
            });
        }

        next_obj_num += 1;
    }

    Ok(PackResult { objects: result, compressed })
}

/// Check whether an object is eligible for packing into an object stream.
fn is_eligible(
    obj_num: u32,
    obj: &PdfObject,
    catalog_obj_num: u32,
    pages_root_obj_num: Option<u32>,
    encrypt_obj_num: Option<u32>,
) -> bool {
    // Must NOT be a stream object
    if obj.is_stream() {
        return false;
    }

    // Must NOT be the catalog
    if obj_num == catalog_obj_num {
        return false;
    }

    // Must NOT be the pages tree root
    if pages_root_obj_num == Some(obj_num) {
        return false;
    }

    // Must NOT be the encryption dictionary
    if encrypt_obj_num == Some(obj_num) {
        return false;
    }

    // Must NOT be a cross-reference stream (Type == XRef)
    if let PdfObject::Dict(d) = obj {
        if d.get_name(b"Type") == Some(b"XRef") {
            return false;
        }
    }

    // Null objects: technically eligible but not worth packing
    if obj.is_null() {
        return false;
    }

    true
}

/// Build a single object stream from a batch of (obj_num, object) pairs.
///
/// The stream content format is:
///   obj_num1 offset1 obj_num2 offset2 ... <data1> <data2> ...
///
/// where offsets are relative to /First (the byte position where object data starts).
fn build_object_stream(objects: &[(u32, &PdfObject)]) -> Result<PdfObject> {
    let n = objects.len();

    // First pass: serialize each object's data
    let mut object_data: Vec<Vec<u8>> = Vec::with_capacity(n);
    for (_obj_num, obj) in objects {
        let mut buf = Vec::new();
        serialize_object(&mut buf, obj)?;
        object_data.push(buf);
    }

    // Compute offsets (relative to start of object data section)
    let mut offsets: Vec<usize> = Vec::with_capacity(n);
    let mut running_offset = 0usize;
    for data in &object_data {
        offsets.push(running_offset);
        running_offset += data.len();
        // Add a space separator between objects (except after the last)
        running_offset += 1;
    }

    // Build the index section: "obj_num1 offset1 obj_num2 offset2 ..."
    let mut index_section = Vec::new();
    for (i, (obj_num, _)) in objects.iter().enumerate() {
        if i > 0 {
            write!(index_section, " ")?;
        }
        write!(index_section, "{} {}", obj_num, offsets[i])?;
    }
    write!(index_section, " ")?; // trailing space before data

    let first = index_section.len();

    // Build the full stream content: index_section + object data
    let mut content = index_section;
    for (i, data) in object_data.iter().enumerate() {
        content.extend_from_slice(data);
        if i < n - 1 {
            content.push(b' ');
        }
    }

    // Compress the stream content
    let compressed = encode_flate(&content)?;

    let mut dict = PdfDict::new();
    dict.insert(b"Type".to_vec(), PdfObject::Name(b"ObjStm".to_vec()));
    dict.insert(b"N".to_vec(), PdfObject::Integer(n as i64));
    dict.insert(b"First".to_vec(), PdfObject::Integer(first as i64));
    dict.insert(
        b"Filter".to_vec(),
        PdfObject::Name(b"FlateDecode".to_vec()),
    );

    Ok(PdfObject::Stream {
        dict,
        data: compressed,
    })
}

/// Write a cross-reference stream instead of a traditional xref table.
///
/// This is required when object streams are used (PDF 1.5+).
/// Returns the xref stream object and the byte offset where it was written.
pub fn write_xref_stream(
    buf: &mut Vec<u8>,
    offsets: &[(u32, usize)],
    compressed: &[CompressedObjInfo],
    catalog_ref: &crate::object::IndirectRef,
    info_ref: Option<&crate::object::IndirectRef>,
    xref_stm_obj_num: u32,
) -> Result<()> {
    let max_obj_num = offsets
        .iter()
        .map(|(n, _)| *n)
        .max()
        .unwrap_or(0)
        .max(xref_stm_obj_num)
        .max(compressed.iter().map(|c| c.obj_num).max().unwrap_or(0));
    let size = max_obj_num + 1;

    // Build offset map for type 1 entries
    let mut offset_map = std::collections::HashMap::new();
    for (num, off) in offsets {
        offset_map.insert(*num, *off);
    }

    // Build compressed object map for type 2 entries
    let mut compressed_map: std::collections::HashMap<u32, (u32, u32)> =
        std::collections::HashMap::new();
    for info in compressed {
        compressed_map.insert(info.obj_num, (info.objstm_num, info.index));
    }

    // Determine field widths.
    // W = [w1 w2 w3] where:
    //   field 1: type (1 byte: 0=free, 1=normal, 2=compressed)
    //   field 2: offset or obj stream number
    //   field 3: generation number or index within obj stream
    let max_offset = offsets.iter().map(|(_, o)| *o).max().unwrap_or(0);
    let max_objstm_num = compressed.iter().map(|c| c.objstm_num as usize).max().unwrap_or(0);
    let w2 = bytes_needed(max_offset.max(max_objstm_num) as u64);
    let w1 = 1u8;
    let max_index = compressed.iter().map(|c| c.index).max().unwrap_or(0);
    let w3 = bytes_needed(max_index.max(255) as u64);

    // Build stream data
    let entry_size = (w1 + w2 + w3) as usize;
    let mut stream_data = Vec::with_capacity(entry_size * size as usize);

    for obj_num in 0..size {
        if obj_num == 0 {
            // Free entry: type=0, next free=0, gen=255
            stream_data.push(0u8);
            write_field(&mut stream_data, 0, w2);
            write_field(&mut stream_data, 255, w3);
        } else if let Some(&off) = offset_map.get(&obj_num) {
            // In-use entry: type=1, offset, gen=0
            stream_data.push(1u8);
            write_field(&mut stream_data, off as u64, w2);
            write_field(&mut stream_data, 0, w3);
        } else if let Some(&(objstm_num, index)) = compressed_map.get(&obj_num) {
            // Compressed entry: type=2, objstm number, index within stream
            stream_data.push(2u8);
            write_field(&mut stream_data, objstm_num as u64, w2);
            write_field(&mut stream_data, index as u64, w3);
        } else if obj_num == xref_stm_obj_num {
            // The xref stream itself: type=1, offset = current buf position
            stream_data.push(1u8);
            write_field(&mut stream_data, buf.len() as u64, w2);
            write_field(&mut stream_data, 0, w3);
        } else {
            // Free entry
            stream_data.push(0u8);
            write_field(&mut stream_data, 0, w2);
            write_field(&mut stream_data, 0, w3);
        }
    }

    // Compress stream data
    let compressed = encode_flate(&stream_data)?;

    let mut dict = PdfDict::new();
    dict.insert(b"Type".to_vec(), PdfObject::Name(b"XRef".to_vec()));
    dict.insert(b"Size".to_vec(), PdfObject::Integer(size as i64));
    dict.insert(
        b"W".to_vec(),
        PdfObject::Array(vec![
            PdfObject::Integer(w1 as i64),
            PdfObject::Integer(w2 as i64),
            PdfObject::Integer(w3 as i64),
        ]),
    );
    dict.insert(
        b"Root".to_vec(),
        PdfObject::Reference(catalog_ref.clone()),
    );
    if let Some(info) = info_ref {
        dict.insert(b"Info".to_vec(), PdfObject::Reference(info.clone()));
    }
    dict.insert(
        b"Filter".to_vec(),
        PdfObject::Name(b"FlateDecode".to_vec()),
    );

    let xref_offset = buf.len();

    // Write as an indirect object
    write!(buf, "{} 0 obj\n", xref_stm_obj_num)?;
    // Manually write the stream with correct /Length
    let mut stream_dict = dict;
    stream_dict.insert(
        b"Length".to_vec(),
        PdfObject::Integer(compressed.len() as i64),
    );
    crate::writer::serialize::serialize_dict(buf, &stream_dict)?;
    buf.extend_from_slice(b"\nstream\r\n");
    buf.extend_from_slice(&compressed);
    buf.extend_from_slice(b"\r\nendstream");
    write!(buf, "\nendobj\n")?;

    // startxref
    write!(buf, "startxref\n{}\n%%EOF\n", xref_offset)?;

    Ok(())
}

/// Compute the number of bytes needed to represent `val`.
fn bytes_needed(val: u64) -> u8 {
    if val <= 0xFF {
        1
    } else if val <= 0xFFFF {
        2
    } else if val <= 0xFF_FFFF {
        3
    } else {
        4
    }
}

/// Write a value as big-endian in exactly `width` bytes.
fn write_field(buf: &mut Vec<u8>, val: u64, width: u8) {
    for i in (0..width).rev() {
        buf.push(((val >> (8 * i as u64)) & 0xFF) as u8);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::object::{IndirectRef, PdfDict, PdfObject};

    #[test]
    fn test_eligible_objects() {
        // Simple dict is eligible
        let dict = PdfObject::Dict(PdfDict::new());
        assert!(is_eligible(10, &dict, 1, Some(2), None));

        // Integer is eligible
        assert!(is_eligible(10, &PdfObject::Integer(42), 1, Some(2), None));

        // Array is eligible
        let arr = PdfObject::Array(vec![PdfObject::Integer(1)]);
        assert!(is_eligible(10, &arr, 1, Some(2), None));
    }

    #[test]
    fn test_ineligible_stream() {
        let stream = PdfObject::Stream {
            dict: PdfDict::new(),
            data: vec![1, 2, 3],
        };
        assert!(!is_eligible(10, &stream, 1, Some(2), None));
    }

    #[test]
    fn test_ineligible_catalog() {
        let dict = PdfObject::Dict(PdfDict::new());
        assert!(!is_eligible(1, &dict, 1, Some(2), None));
    }

    #[test]
    fn test_ineligible_pages_root() {
        let dict = PdfObject::Dict(PdfDict::new());
        assert!(!is_eligible(2, &dict, 1, Some(2), None));
    }

    #[test]
    fn test_ineligible_encrypt() {
        let dict = PdfObject::Dict(PdfDict::new());
        assert!(!is_eligible(5, &dict, 1, Some(2), Some(5)));
    }

    #[test]
    fn test_ineligible_xref_stream() {
        let mut d = PdfDict::new();
        d.insert(b"Type".to_vec(), PdfObject::Name(b"XRef".to_vec()));
        let obj = PdfObject::Dict(d);
        assert!(!is_eligible(10, &obj, 1, Some(2), None));
    }

    #[test]
    fn test_ineligible_null() {
        assert!(!is_eligible(10, &PdfObject::Null, 1, Some(2), None));
    }

    #[test]
    fn test_pack_object_streams_structure() {
        // Create a set of objects: catalog (1), pages root (2), and some simple objects
        let mut catalog = PdfDict::new();
        catalog.insert(b"Type".to_vec(), PdfObject::Name(b"Catalog".to_vec()));

        let mut pages = PdfDict::new();
        pages.insert(b"Type".to_vec(), PdfObject::Name(b"Pages".to_vec()));

        let objects = vec![
            (1, PdfObject::Dict(catalog)),
            (2, PdfObject::Dict(pages)),
            (3, PdfObject::Integer(42)),
            (4, PdfObject::String(b"hello".to_vec())),
            (5, PdfObject::Array(vec![PdfObject::Integer(1), PdfObject::Integer(2)])),
        ];

        let packed = pack_object_streams(&objects, 100, 1, Some(2), None).unwrap();

        // Catalog and pages root should remain as separate objects
        let catalog_entry = packed.objects.iter().find(|(n, _)| *n == 1);
        assert!(catalog_entry.is_some());
        let pages_entry = packed.objects.iter().find(|(n, _)| *n == 2);
        assert!(pages_entry.is_some());

        // Objects 3, 4, 5 should be packed into an object stream
        // So we shouldn't find them as standalone objects anymore
        let obj3 = packed.objects.iter().find(|(n, _)| *n == 3);
        assert!(obj3.is_none(), "object 3 should be packed");
        let obj4 = packed.objects.iter().find(|(n, _)| *n == 4);
        assert!(obj4.is_none(), "object 4 should be packed");

        // There should be an object stream (new obj num = 6)
        let objstm = packed.objects.iter().find(|(_, obj)| {
            if let PdfObject::Stream { dict, .. } = obj {
                dict.get_name(b"Type") == Some(b"ObjStm")
            } else {
                false
            }
        });
        assert!(objstm.is_some(), "should contain an object stream");

        let (_, stream_obj) = objstm.unwrap();
        if let PdfObject::Stream { dict, .. } = stream_obj {
            assert_eq!(dict.get_i64(b"N"), Some(3)); // 3 objects packed
            assert!(dict.get_i64(b"First").unwrap() > 0);
            assert_eq!(dict.get_name(b"Filter"), Some(b"FlateDecode".as_slice()));
        } else {
            panic!("expected stream object");
        }
    }

    #[test]
    fn test_pack_splits_by_max() {
        let mut objects = vec![
            (1, PdfObject::Dict(PdfDict::new())), // catalog
        ];
        // Add 5 eligible objects
        for i in 2..=6 {
            objects.push((i, PdfObject::Integer(i as i64)));
        }

        let packed = pack_object_streams(&objects, 2, 1, None, None).unwrap();

        // 5 eligible objects split into batches of 2 => 3 object streams
        let objstm_count = packed.objects
            .iter()
            .filter(|(_, obj)| {
                if let PdfObject::Stream { dict, .. } = obj {
                    dict.get_name(b"Type") == Some(b"ObjStm")
                } else {
                    false
                }
            })
            .count();
        assert_eq!(objstm_count, 3);
    }

    #[test]
    fn test_pack_no_eligible() {
        let stream = PdfObject::Stream {
            dict: PdfDict::new(),
            data: vec![1, 2, 3],
        };
        let objects = vec![
            (1, PdfObject::Dict(PdfDict::new())), // catalog
            (2, stream),
        ];

        let packed = pack_object_streams(&objects, 100, 1, None, None).unwrap();
        assert_eq!(packed.objects.len(), 2); // unchanged
    }

    #[test]
    fn test_build_object_stream_content() {
        use flate2::read::ZlibDecoder;
        use std::io::Read;

        let obj1 = PdfObject::Integer(42);
        let obj2 = PdfObject::String(b"test".to_vec());
        let items: Vec<(u32, &PdfObject)> = vec![(10, &obj1), (20, &obj2)];

        let result = build_object_stream(&items).unwrap();
        if let PdfObject::Stream { dict, data } = result {
            assert_eq!(dict.get_name(b"Type"), Some(b"ObjStm".as_slice()));
            assert_eq!(dict.get_i64(b"N"), Some(2));

            // Decompress and check the content structure
            let mut decoder = ZlibDecoder::new(&data[..]);
            let mut decompressed = Vec::new();
            decoder.read_to_end(&mut decompressed).unwrap();
            let text = String::from_utf8_lossy(&decompressed);

            // Should start with "10 0 20 <offset>" pattern
            assert!(text.starts_with("10 "), "content should start with first obj num: {}", text);
            assert!(text.contains("20 "), "content should contain second obj num");
            // Should contain the serialized objects
            assert!(text.contains("42"), "content should contain integer 42");
            assert!(text.contains("(test)"), "content should contain string (test)");
        } else {
            panic!("expected stream object");
        }
    }

    #[test]
    fn test_bytes_needed() {
        assert_eq!(bytes_needed(0), 1);
        assert_eq!(bytes_needed(255), 1);
        assert_eq!(bytes_needed(256), 2);
        assert_eq!(bytes_needed(65535), 2);
        assert_eq!(bytes_needed(65536), 3);
        assert_eq!(bytes_needed(0xFF_FFFF), 3);
        assert_eq!(bytes_needed(0x100_0000), 4);
    }

    #[test]
    fn test_write_field() {
        let mut buf = Vec::new();
        write_field(&mut buf, 0x1234, 2);
        assert_eq!(buf, vec![0x12, 0x34]);

        let mut buf = Vec::new();
        write_field(&mut buf, 42, 1);
        assert_eq!(buf, vec![42]);

        let mut buf = Vec::new();
        write_field(&mut buf, 0xABCDEF, 3);
        assert_eq!(buf, vec![0xAB, 0xCD, 0xEF]);
    }

    #[test]
    fn test_write_xref_stream() {
        let mut buf = Vec::new();
        // Write PDF header first
        buf.extend_from_slice(b"%PDF-1.5\n");

        let offsets = vec![(1, 20), (2, 100)];
        let catalog_ref = IndirectRef { obj_num: 1, gen_num: 0 };

        write_xref_stream(&mut buf, &offsets, &[], &catalog_ref, None, 3).unwrap();

        let text = String::from_utf8_lossy(&buf);
        assert!(text.contains("3 0 obj"));
        assert!(text.contains("/Type /XRef"));
        assert!(text.contains("/Root 1 0 R"));
        assert!(text.contains("startxref"));
        assert!(text.contains("%%EOF"));
    }
}
