use std::path::Path;

use justpdf_core::PdfDocument;
use justpdf_core::page;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: page_info <pdf-file>");
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

    let count = page::page_count(&mut doc).unwrap_or(0);
    println!("PDF Version: {}.{}", doc.version.0, doc.version.1);
    println!("Pages: {count}");
    println!();

    let pages = match page::collect_pages(&mut doc) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Error collecting pages: {e}");
            std::process::exit(1);
        }
    };

    for p in &pages {
        print!("Page {}: MediaBox {}", p.index + 1, p.media_box,);
        if let Some(crop) = &p.crop_box {
            print!(", CropBox {crop}");
        }
        if let Some(trim) = &p.trim_box {
            print!(", TrimBox {trim}");
        }
        if let Some(bleed) = &p.bleed_box {
            print!(", BleedBox {bleed}");
        }
        if let Some(art) = &p.art_box {
            print!(", ArtBox {art}");
        }
        if p.rotate != 0 {
            print!(", Rotate {}", p.rotate);
        }
        println!(
            "  ({:.0}x{:.0} pt)",
            p.media_box.width(),
            p.media_box.height()
        );
    }
}
