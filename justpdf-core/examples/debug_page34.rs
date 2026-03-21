use justpdf_core::parser::PdfDocument;
use justpdf_core::page::collect_pages;
use justpdf_core::object::PdfObject;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let path = &args[1];
    let data = std::fs::read(path).unwrap();
    let doc = PdfDocument::from_bytes(data).unwrap();
    let pages = collect_pages(&doc).unwrap();
    
    let page = &pages[33]; // page 34 (0-indexed)
    println!("Page 34 ref: {} {}", page.page_ref.obj_num, page.page_ref.gen_num);
    
    // Get page object
    let page_obj = doc.resolve(&page.page_ref).unwrap();
    if let PdfObject::Dict(dict) = &page_obj {
        // Check Resources → Font
        if let Some(res) = dict.get(b"Resources") {
            let res_dict = match res {
                PdfObject::Dict(d) => d.clone(),
                PdfObject::Reference(r) => {
                    if let PdfObject::Dict(d) = doc.resolve(r).unwrap() { d } else { return; }
                }
                _ => return,
            };
            if let Some(fonts) = res_dict.get(b"Font") {
                let font_dict = match fonts {
                    PdfObject::Dict(d) => d.clone(),
                    PdfObject::Reference(r) => {
                        if let PdfObject::Dict(d) = doc.resolve(r).unwrap() { d } else { return; }
                    }
                    _ => return,
                };
                println!("Fonts on page 34:");
                for (name, val) in font_dict.iter() {
                    let name_str = String::from_utf8_lossy(name);
                    if let PdfObject::Reference(r) = val {
                        let font_obj = doc.resolve(r).unwrap();
                        if let PdfObject::Dict(fd) = &font_obj {
                            let subtype = fd.get_name(b"Subtype").map(|s| String::from_utf8_lossy(s).to_string()).unwrap_or_default();
                            let basefont = fd.get_name(b"BaseFont").map(|s| String::from_utf8_lossy(s).to_string()).unwrap_or_default();
                            let encoding = fd.get_name(b"Encoding").map(|s| String::from_utf8_lossy(s).to_string()).unwrap_or_default();
                            println!("  /{} → obj {} | Subtype={} BaseFont={} Encoding={}", name_str, r.obj_num, subtype, basefont, encoding);
                            
                            // Check if Type0 with DescendantFonts
                            if subtype == "Type0" {
                                if let Some(PdfObject::Array(desc)) = fd.get(b"DescendantFonts") {
                                    for d in desc {
                                        if let PdfObject::Reference(dr) = d {
                                            let cid = doc.resolve(dr).unwrap();
                                            if let PdfObject::Dict(cd) = &cid {
                                                let cs = cd.get_name(b"Subtype").map(|s| String::from_utf8_lossy(s).to_string()).unwrap_or_default();
                                                println!("    CIDFont obj {} Subtype={}", dr.obj_num, cs);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        
        // Check Contents
        if let Some(contents) = dict.get(b"Contents") {
            match contents {
                PdfObject::Reference(r) => {
                    let stream = doc.resolve(r).unwrap();
                    if let PdfObject::Stream { dict: sd, data: sdata } = &stream {
                        let filter = sd.get_name(b"Filter").map(|s| String::from_utf8_lossy(s).to_string()).unwrap_or("none".into());
                        println!("\nContent stream obj {} filter={} raw_len={}", r.obj_num, filter, sdata.len());
                    }
                }
                _ => println!("Contents: {:?}", contents),
            }
        }
    }
}
