use std::path::Path;

use justpdf_core::{PdfDocument, PdfObject};

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: dump_objects <pdf-file> [--obj N]");
        eprintln!("  Dump all objects or a specific object from a PDF file.");
        std::process::exit(1);
    }

    let path = Path::new(&args[1]);
    let specific_obj: Option<u32> = args
        .iter()
        .position(|a| a == "--obj")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.parse().ok());

    let mut doc = match PdfDocument::open(path) {
        Ok(doc) => doc,
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    };

    println!("PDF Version: {}.{}", doc.version.0, doc.version.1);
    println!("Objects: {}", doc.object_count());
    println!("Trailer: {}", PdfObject::Dict(doc.trailer().clone()));
    println!();

    if let Some(obj_num) = specific_obj {
        let iref = justpdf_core::IndirectRef {
            obj_num,
            gen_num: 0,
        };
        match doc.resolve(&iref) {
            Ok(obj) => println!("obj {} 0: {} = {}", obj_num, obj.type_name(), obj),
            Err(e) => eprintln!("Error resolving object {obj_num}: {e}"),
        }
    } else {
        let mut refs: Vec<_> = doc.object_refs().collect();
        refs.sort_by_key(|r| r.obj_num);

        for iref in refs {
            match doc.resolve(&iref) {
                Ok(obj) => {
                    let summary = format_summary(obj);
                    println!(
                        "obj {} {}: {} {}",
                        iref.obj_num,
                        iref.gen_num,
                        obj.type_name(),
                        summary
                    );
                }
                Err(e) => {
                    eprintln!("obj {} {}: ERROR {}", iref.obj_num, iref.gen_num, e);
                }
            }
        }
    }
}

fn format_summary(obj: &PdfObject) -> String {
    match obj {
        PdfObject::Dict(d) => {
            if let Some(t) = d.get_name(b"Type") {
                let type_str = std::str::from_utf8(t).unwrap_or("?");
                format!("/Type /{type_str} ({} keys)", d.len())
            } else {
                format!("({} keys)", d.len())
            }
        }
        PdfObject::Stream { dict, data } => {
            if let Some(t) = dict.get_name(b"Type") {
                let type_str = std::str::from_utf8(t).unwrap_or("?");
                format!("/Type /{type_str} ({} raw bytes)", data.len())
            } else {
                format!("({} raw bytes)", data.len())
            }
        }
        PdfObject::Array(arr) => format!("({} items)", arr.len()),
        other => format!("{other}"),
    }
}
