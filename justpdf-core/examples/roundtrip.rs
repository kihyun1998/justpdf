use justpdf_core::parser::PdfDocument;
use justpdf_core::writer::modify::DocumentModifier;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let input = &args[1];
    let output = &args[2];
    
    let data = std::fs::read(input).unwrap();
    let doc = PdfDocument::from_bytes(data).unwrap();
    let modifier = DocumentModifier::from_document(&doc).unwrap();
    let result = modifier.build().unwrap();
    std::fs::write(output, &result).unwrap();
    println!("Roundtrip: {} -> {}", input, output);
}
