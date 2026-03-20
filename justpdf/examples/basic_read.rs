//! Basic PDF reading example.
//!
//! Usage: cargo run --example basic_read -- <pdf-file>

use justpdf::Document;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: basic_read <pdf-file>");
        std::process::exit(1);
    }

    let doc = Document::open(&args[1])?;

    println!("Version: {}", doc.version_string());
    println!("Pages: {}", doc.page_count());
    println!("Encrypted: {}", doc.is_encrypted());

    if let Some(title) = doc.title() {
        println!("Title: {title}");
    }
    if let Some(author) = doc.author() {
        println!("Author: {author}");
    }

    // Extract text from first page
    if let Ok(page) = doc.page(0) {
        println!("\n--- Page 1 text ---");
        match page.text() {
            Ok(text) => println!("{}", &text[..text.len().min(500)]),
            Err(e) => println!("(text extraction failed: {e})"),
        }
    }

    Ok(())
}
