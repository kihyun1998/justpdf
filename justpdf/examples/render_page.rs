//! Render a PDF page to an image file.
//!
//! Usage: cargo run --example render_page -- <pdf-file> [page] [dpi] [output.png]

use justpdf::Document;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: render_page <pdf-file> [page] [dpi] [output.png]");
        std::process::exit(1);
    }

    let doc = Document::open(&args[1])?;
    let page_idx: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
    let dpi: f64 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(150.0);
    let output = args.get(4).map(|s| s.as_str()).unwrap_or("output.png");

    let page = doc.page(page_idx)?;
    println!("Page {}: {:.0}x{:.0} pt, rotation {}", page_idx + 1, page.width(), page.height(), page.rotation());

    let png = page.render_png(dpi)?;
    std::fs::write(output, &png)?;
    println!("Rendered to {output} ({} bytes, {dpi} DPI)", png.len());

    Ok(())
}
