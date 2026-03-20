//! Create a simple PDF document.
//!
//! Usage: cargo run --example create_pdf -- [output.pdf]

use justpdf::{DocumentBuilder, PageBuilder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let output = args.get(1).map(|s| s.as_str()).unwrap_or("created.pdf");

    let mut builder = DocumentBuilder::new();
    builder.set_title("Created by justpdf");
    builder.set_author("justpdf example");

    let font = builder.add_standard_font("Helvetica");

    // Page 1
    let mut page1 = PageBuilder::new(612.0, 792.0);
    page1.add_font(&font, "Helvetica");
    page1.begin_text();
    page1.set_font(&font, 36.0);
    page1.move_to(72.0, 700.0);
    page1.show_text("Hello, justpdf!");
    page1.end_text();

    page1.begin_text();
    page1.set_font(&font, 14.0);
    page1.move_to(72.0, 650.0);
    page1.show_text("This PDF was created with the justpdf Rust library.");
    page1.end_text();

    // Draw a rectangle
    page1.set_fill_rgb(0.2, 0.4, 0.8);
    page1.fill_rect(72.0, 500.0, 468.0, 2.0);

    builder.add_page(page1);

    // Page 2
    let mut page2 = PageBuilder::new(612.0, 792.0);
    page2.add_font(&font, "Helvetica");
    page2.begin_text();
    page2.set_font(&font, 24.0);
    page2.move_to(72.0, 700.0);
    page2.show_text("Page 2");
    page2.end_text();
    builder.add_page(page2);

    let bytes = builder.build()?;
    std::fs::write(output, &bytes)?;
    println!("Created {output} ({} bytes, 2 pages)", bytes.len());

    Ok(())
}
