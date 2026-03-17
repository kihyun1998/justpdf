use std::path::Path;

use justpdf_core::content;
use justpdf_core::page;
use justpdf_core::{IndirectRef, PdfDocument, PdfObject};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: trace_ops <pdf-file> [--page N]");
        std::process::exit(1);
    }

    let path = Path::new(&args[1]);
    let page_num: usize = args
        .iter()
        .position(|a| a == "--page")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.parse().ok())
        .unwrap_or(1);

    let mut doc = match PdfDocument::open(path) {
        Ok(doc) => doc,
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    };

    let pages = match page::collect_pages(&mut doc) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    };

    if page_num == 0 || page_num > pages.len() {
        eprintln!("Page {page_num} out of range (1-{})", pages.len());
        std::process::exit(1);
    }

    let page = &pages[page_num - 1];
    println!(
        "Page {}: MediaBox {} ({:.0}x{:.0} pt)",
        page_num,
        page.media_box,
        page.media_box.width(),
        page.media_box.height()
    );
    println!();

    // Resolve and decode the content stream(s)
    let content_data = match &page.contents_ref {
        Some(PdfObject::Reference(r)) => {
            let r = r.clone();
            let obj = match doc.resolve(&r) {
                Ok(o) => o.clone(),
                Err(e) => {
                    eprintln!("Error resolving contents: {e}");
                    std::process::exit(1);
                }
            };
            decode_content_object(&doc, &obj)
        }
        Some(PdfObject::Array(arr)) => {
            let refs: Vec<IndirectRef> = arr
                .iter()
                .filter_map(|o| o.as_reference().cloned())
                .collect();
            let mut all_data = Vec::new();
            for r in refs {
                let obj = match doc.resolve(&r) {
                    Ok(o) => o.clone(),
                    Err(e) => {
                        eprintln!("Error resolving content stream: {e}");
                        continue;
                    }
                };
                all_data.extend(decode_content_object(&doc, &obj));
                all_data.push(b'\n');
            }
            all_data
        }
        _ => {
            println!("(no content stream)");
            return;
        }
    };

    // Parse the content stream into operations
    let ops = match content::parse_content_stream(&content_data) {
        Ok(ops) => ops,
        Err(e) => {
            eprintln!("Error parsing content stream: {e}");
            std::process::exit(1);
        }
    };

    println!("Operations: {}", ops.len());
    println!("---");
    for op in &ops {
        println!("{op}");
    }
}

fn decode_content_object(doc: &PdfDocument, obj: &PdfObject) -> Vec<u8> {
    match obj {
        PdfObject::Stream { dict, data } => doc.decode_stream(dict, data).unwrap_or_default(),
        _ => Vec::new(),
    }
}
