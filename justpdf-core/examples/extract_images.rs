use std::path::Path;

use justpdf_core::image;
use justpdf_core::page;
use justpdf_core::{PdfDocument, PdfObject};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: extract_images <pdf-file> [--out-dir DIR]");
        std::process::exit(1);
    }

    let path = Path::new(&args[1]);
    let out_dir = args
        .iter()
        .position(|a| a == "--out-dir")
        .and_then(|i| args.get(i + 1))
        .map(|s| Path::new(s.as_str()))
        .unwrap_or(Path::new("."));

    std::fs::create_dir_all(out_dir).ok();

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

    let mut image_count = 0;
    let mut seen = std::collections::HashSet::new();

    for page_info in &pages {
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

        let xobject_dict = match resources.get(b"XObject") {
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

        for (_name, value) in xobject_dict.iter() {
            let xobj_ref = match value {
                PdfObject::Reference(r) => r.clone(),
                _ => continue,
            };

            if !seen.insert(xobj_ref.obj_num) {
                continue;
            }

            let xobj = match doc.resolve(&xobj_ref) {
                Ok(obj) => obj.clone(),
                Err(_) => continue,
            };

            let (dict, raw_data) = match &xobj {
                PdfObject::Stream { dict, data } => (dict, data.as_slice()),
                _ => continue,
            };

            // Check if it's an Image
            if dict.get_name(b"Subtype") != Some(b"Image") {
                continue;
            }

            let info = match image::image_info(dict) {
                Some(i) => i,
                None => continue,
            };

            image_count += 1;
            let filter_str = info
                .filter
                .as_ref()
                .map(|f| std::str::from_utf8(f).unwrap_or("?"))
                .unwrap_or("none");
            let cs_str = std::str::from_utf8(&info.color_space).unwrap_or("?");

            println!(
                "Image {image_count}: obj {} ({}x{}, {cs_str}, {filter_str}, {} bpc)",
                xobj_ref.obj_num, info.width, info.height, info.bits_per_component
            );

            // If JPEG, extract raw bytes
            if filter_str == "DCTDecode" {
                let filename = format!("image_{:03}.jpg", image_count);
                let filepath = out_dir.join(&filename);
                match std::fs::write(&filepath, raw_data) {
                    Ok(()) => println!("  → saved {}", filepath.display()),
                    Err(e) => eprintln!("  → error saving: {e}"),
                }
            }
        }
    }

    if image_count == 0 {
        println!("No images found.");
    } else {
        println!("\nTotal: {image_count} images");
    }
}
