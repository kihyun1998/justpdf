use justpdf_core::parser::PdfDocument;
use justpdf_core::page::collect_pages;
use justpdf_core::text::extract_page_text_string;

fn check(path: &str) {
    println!("=== {} ===", path);
    let data = std::fs::read(path).unwrap();
    let doc = PdfDocument::from_bytes(data).unwrap();
    let pages = collect_pages(&doc).unwrap();
    println!("Pages: {}", pages.len());
    
    // Check pages around the boundary (30-40) and end
    for i in [0, 30, 33, 34, 35, 36, 40, 50, 100, pages.len()-1] {
        if i >= pages.len() { continue; }
        let text = extract_page_text_string(&doc, &pages[i]).unwrap_or_else(|e| format!("ERROR: {}", e));
        let preview: String = text.chars().take(60).collect();
        println!("Page {:>3} ({:>5} chars): {:?}", i+1, text.len(), preview);
    }
    println!();
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    for path in &args[1..] {
        check(path);
    }
}
