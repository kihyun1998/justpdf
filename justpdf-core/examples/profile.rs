use std::time::Instant;

use justpdf_core::page::collect_pages;
use justpdf_core::parser::PdfDocument;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let path = args.get(1).expect("Usage: profile <pdf-file>");

    let start = Instant::now();
    let data = std::fs::read(path).unwrap();
    let read_time = start.elapsed();

    let start = Instant::now();
    let mut doc = PdfDocument::from_bytes(data).unwrap();
    let parse_time = start.elapsed();

    let start = Instant::now();
    let pages = collect_pages(&mut doc).unwrap();
    let pages_time = start.elapsed();

    println!("File: {}", path);
    println!("Pages: {}", pages.len());
    println!("Objects: {}", doc.object_count());
    println!("Read time: {:?}", read_time);
    println!("Parse time: {:?}", parse_time);
    println!("Page collection: {:?}", pages_time);
}
