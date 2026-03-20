//! Merge multiple PDF files.
//!
//! Usage: cargo run --example merge_pdfs -- <file1.pdf> <file2.pdf> ... -o merged.pdf

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();

    let output_idx = args.iter().position(|a| a == "-o");
    let output = match output_idx {
        Some(i) => args.get(i + 1).map(|s| s.as_str()).unwrap_or("merged.pdf"),
        None => "merged.pdf",
    };

    let input_files: Vec<&str> = args[1..]
        .iter()
        .filter(|a| *a != "-o" && *a != output)
        .map(|s| s.as_str())
        .collect();

    if input_files.is_empty() {
        eprintln!("Usage: merge_pdfs <file1.pdf> <file2.pdf> ... -o merged.pdf");
        std::process::exit(1);
    }

    let paths: Vec<std::path::PathBuf> = input_files.iter().map(std::path::PathBuf::from).collect();
    let merged = justpdf::merge(&paths)?;
    std::fs::write(output, &merged)?;
    println!("Merged {} files → {output} ({} bytes)", input_files.len(), merged.len());

    Ok(())
}
