use std::path::Path;

use justpdf_core::font;
use justpdf_core::page;
use justpdf_core::{PdfDocument, PdfObject};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: list_fonts <pdf-file>");
        std::process::exit(1);
    }

    let path = Path::new(&args[1]);
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

    let mut seen_fonts = std::collections::HashSet::new();

    for page_info in &pages {
        // Get Resources dict
        let resources = match &page_info.resources_ref {
            Some(PdfObject::Dict(d)) => d.clone(),
            Some(PdfObject::Reference(r)) => {
                let r = r.clone();
                match doc.resolve(&r) {
                    Ok(obj) => match obj.as_dict() {
                        Some(d) => d.clone(),
                        None => continue,
                    },
                    Err(_) => continue,
                }
            }
            _ => continue,
        };

        // Get Font dict from Resources
        let font_dict = match resources.get(b"Font") {
            Some(PdfObject::Dict(d)) => d.clone(),
            Some(PdfObject::Reference(r)) => {
                let r = r.clone();
                match doc.resolve(&r) {
                    Ok(obj) => match obj.as_dict() {
                        Some(d) => d.clone(),
                        None => continue,
                    },
                    Err(_) => continue,
                }
            }
            _ => continue,
        };

        for (name, value) in font_dict.iter() {
            let font_ref = match value {
                PdfObject::Reference(r) => r.clone(),
                _ => continue,
            };

            if !seen_fonts.insert(font_ref.obj_num) {
                continue;
            }

            let font_obj = match doc.resolve(&font_ref) {
                Ok(obj) => obj.clone(),
                Err(_) => continue,
            };

            if let Some(d) = font_obj.as_dict() {
                let info = font::parse_font_info(d);
                let name_str = std::str::from_utf8(name).unwrap_or("?");
                let base_str = std::str::from_utf8(&info.base_font).unwrap_or("?");
                let subtype_str = std::str::from_utf8(&info.subtype).unwrap_or("?");
                let std14 = if info.is_standard14 { " [Std14]" } else { "" };

                println!("  /{name_str} → {base_str} ({subtype_str}){std14}");
            }
        }
    }
}
