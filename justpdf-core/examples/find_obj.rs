use justpdf_core::parser::PdfDocument;
use justpdf_core::object::{PdfObject, IndirectRef};

fn main() {
    let data = std::fs::read("/tmp/realfinal_roundtrip.pdf").unwrap();
    let doc = PdfDocument::from_bytes(data).unwrap();
    
    // Try resolving obj 6281
    for obj_num in [6281u32, 6282, 6283, 6286, 1774, 1775] {
        let iref = IndirectRef { obj_num, gen_num: 0 };
        match doc.resolve(&iref) {
            Ok(obj) => {
                let typ = match &obj {
                    PdfObject::Dict(d) => format!("Dict(keys={})", d.len()),
                    PdfObject::Stream { data, .. } => format!("Stream(len={})", data.len()),
                    PdfObject::Null => "Null".to_string(),
                    other => format!("{:?}", other).chars().take(50).collect(),
                };
                println!("obj {}: {}", obj_num, typ);
            }
            Err(e) => println!("obj {}: ERROR: {}", obj_num, e),
        }
    }
    
    // Count total objects accessible
    let refs: Vec<_> = doc.object_refs().collect();
    println!("\nTotal object refs: {}", refs.len());
    let max_obj = refs.iter().map(|r| r.obj_num).max().unwrap_or(0);
    println!("Max obj num: {}", max_obj);
}
