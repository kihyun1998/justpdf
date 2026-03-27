use justpdf_core::parser::PdfDocument;
use justpdf_core::object::{PdfObject, IndirectRef};

fn check_font(doc: &PdfDocument, label: &str, obj_num: u32) {
    let iref = IndirectRef { obj_num, gen_num: 0 };
    let obj = doc.resolve(&iref).unwrap();
    if let PdfObject::Dict(d) = &obj {
        let subtype = d.get_name(b"Subtype").map(|s| String::from_utf8_lossy(s).to_string()).unwrap_or_default();
        println!("{} font obj {} subtype={}", label, obj_num, subtype);
        
        // Check ToUnicode
        match d.get(b"ToUnicode") {
            Some(PdfObject::Reference(r)) => {
                let tu = doc.resolve(r).unwrap();
                if let PdfObject::Stream { dict, data } = &tu {
                    let decoded = justpdf_core::stream::decode_stream(data, dict).unwrap_or_default();
                    println!("  ToUnicode obj {} raw={} decoded={}", r.obj_num, data.len(), decoded.len());
                    // Show first 100 bytes
                    let preview: String = decoded.iter().take(100).map(|&b| b as char).collect();
                    println!("  Preview: {:?}", preview);
                }
            }
            Some(other) => println!("  ToUnicode: {:?}", other),
            None => println!("  ToUnicode: NONE"),
        }
        
        // Check DescendantFonts for Type0
        if let Some(PdfObject::Array(desc)) = d.get(b"DescendantFonts") {
            for dd in desc {
                if let PdfObject::Reference(r) = dd {
                    let cid = doc.resolve(r).unwrap();
                    if let PdfObject::Dict(cd) = &cid {
                        let dw = cd.get_i64(b"DW");
                        println!("  CIDFont obj {} DW={:?}", r.obj_num, dw);
                        // Check CIDToGIDMap
                        match cd.get(b"CIDToGIDMap") {
                            Some(PdfObject::Name(n)) => println!("  CIDToGIDMap: /{}", String::from_utf8_lossy(n)),
                            Some(PdfObject::Reference(mr)) => {
                                let m = doc.resolve(mr).unwrap();
                                if let PdfObject::Stream { data, .. } = &m {
                                    println!("  CIDToGIDMap: stream obj {} len={}", mr.obj_num, data.len());
                                }
                            }
                            _ => println!("  CIDToGIDMap: NONE"),
                        }
                    }
                }
            }
        }
    }
}

fn main() {
    println!("=== ORIGINAL ===");
    let orig_data = std::fs::read("/Users/parkkihyun/Downloads/realfinal.pdf").unwrap();
    let orig = PdfDocument::from_bytes(orig_data).unwrap();
    check_font(&orig, "ORIG", 6281);  // MGBK - Pretendard Black
    check_font(&orig, "ORIG", 1774);  // MGR - Pretendard Medium
    
    println!("\n=== ROUNDTRIP ===");
    let rt_data = std::fs::read("/tmp/realfinal_roundtrip.pdf").unwrap();
    let rt = PdfDocument::from_bytes(rt_data).unwrap();
    check_font(&rt, "RT", 6281);
    check_font(&rt, "RT", 1774);
}
