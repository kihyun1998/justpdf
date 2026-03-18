use std::path::Path;

use justpdf_core::PdfDocument;
use justpdf_render::{RenderOptions, render_page};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: render <pdf-file> [page] [dpi] [output.png]");
        std::process::exit(1);
    }

    let pdf_path = Path::new(&args[1]);
    let page_index: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
    let dpi: f64 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(150.0);
    let output = args
        .get(4)
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("page_{}.png", page_index + 1));

    let mut doc = PdfDocument::open(pdf_path).expect("failed to open PDF");

    let options = RenderOptions {
        dpi,
        ..Default::default()
    };

    println!("Rendering page {} at {dpi} DPI...", page_index + 1);
    let png_data = render_page(&mut doc, page_index, &options).expect("failed to render");

    std::fs::write(&output, &png_data).expect("failed to write PNG");
    println!("Saved to {output} ({} bytes)", png_data.len());
}
