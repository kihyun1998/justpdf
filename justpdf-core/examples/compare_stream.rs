use justpdf_core::parser::PdfDocument;
use justpdf_core::object::PdfObject;
use justpdf_core::stream::decode_stream;

fn get_stream_bytes(doc: &PdfDocument, obj_num: u32) -> Option<Vec<u8>> {
    let iref = justpdf_core::object::IndirectRef { obj_num, gen_num: 0 };
    let obj = doc.resolve(&iref).ok()?;
    if let PdfObject::Stream { dict, data } = &obj {
        decode_stream(data, dict).ok()
    } else {
        None
    }
}

fn main() {
    let orig_data = std::fs::read("/Users/parkkihyun/Downloads/realfinal.pdf").unwrap();
    let orig = PdfDocument::from_bytes(orig_data).unwrap();
    
    let rt_data = std::fs::read("/tmp/realfinal_roundtrip.pdf").unwrap();
    let rt = PdfDocument::from_bytes(rt_data).unwrap();
    
    // Check font stream object for Pretendard (MGBK font, CIDFont obj 6286)
    // Its FontDescriptor should have FontFile2
    let iref = justpdf_core::object::IndirectRef { obj_num: 6286, gen_num: 0 };
    let cid_obj = orig.resolve(&iref).unwrap();
    if let PdfObject::Dict(cd) = &cid_obj {
        if let Some(PdfObject::Reference(fd_ref)) = cd.get(b"FontDescriptor") {
            let fd = orig.resolve(fd_ref).unwrap();
            if let PdfObject::Dict(fd_dict) = &fd {
                if let Some(PdfObject::Reference(ff2_ref)) = fd_dict.get(b"FontFile2") {
                    println!("FontFile2 obj: {}", ff2_ref.obj_num);
                    
                    let orig_bytes = get_stream_bytes(&orig, ff2_ref.obj_num);
                    let rt_bytes = get_stream_bytes(&rt, ff2_ref.obj_num);
                    
                    match (orig_bytes, rt_bytes) {
                        (Some(ob), Some(rb)) => {
                            println!("Original FontFile2 decoded: {} bytes", ob.len());
                            println!("Roundtrip FontFile2 decoded: {} bytes", rb.len());
                            println!("Match: {}", ob == rb);
                            if ob != rb {
                                // Find first difference
                                for (i, (a, b)) in ob.iter().zip(rb.iter()).enumerate() {
                                    if a != b {
                                        println!("First diff at byte {}: orig={:#04x} rt={:#04x}", i, a, b);
                                        break;
                                    }
                                }
                            }
                        }
                        (Some(ob), None) => println!("Original: {} bytes, Roundtrip: MISSING", ob.len()),
                        (None, Some(rb)) => println!("Original: MISSING, Roundtrip: {} bytes", rb.len()),
                        _ => println!("Both missing"),
                    }
                }
            }
        }
    }
    
    // Also check content stream 6827
    println!("\nContent stream obj 6827:");
    let orig_cs = get_stream_bytes(&orig, 6827);
    let rt_cs = get_stream_bytes(&rt, 6827);
    match (orig_cs, rt_cs) {
        (Some(ob), Some(rb)) => {
            println!("  Original: {} bytes", ob.len());
            println!("  Roundtrip: {} bytes", rb.len());
            println!("  Match: {}", ob == rb);
            if ob != rb {
                for (i, (a, b)) in ob.iter().zip(rb.iter()).enumerate() {
                    if a != b {
                        println!("  First diff at byte {}: orig={:#04x} rt={:#04x}", i, a, b);
                        println!("  Context orig: {:?}", String::from_utf8_lossy(&ob[i.saturating_sub(20)..i+20.min(ob.len())]));
                        println!("  Context rt:   {:?}", String::from_utf8_lossy(&rb[i.saturating_sub(20)..i+20.min(rb.len())]));
                        break;
                    }
                }
            }
        }
        _ => println!("  Missing"),
    }
}
