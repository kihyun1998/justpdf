//! Compare justpdf performance with MuPDF's mutool.
//!
//! Usage: cargo run --release --example compare_mupdf -- <pdf-file>
//!
//! Requires `mutool` to be on the PATH for MuPDF comparison.

use std::path::Path;
use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: compare_mupdf <pdf-file>");
        std::process::exit(1);
    }

    let path = Path::new(&args[1]);

    println!("=== justpdf vs MuPDF comparison ===");
    println!("File: {}", path.display());
    println!();

    // --- justpdf parsing ---
    let data = std::fs::read(path).unwrap();
    let file_size = data.len();
    println!("File size: {:.2} MB", file_size as f64 / 1_048_576.0);

    let start = Instant::now();
    let doc = justpdf_core::PdfDocument::from_bytes(data).unwrap();
    let parse_time = start.elapsed();
    println!("\n[justpdf] Parse time: {:?}", parse_time);

    let start = Instant::now();
    let pages = justpdf_core::page::collect_pages(&doc).unwrap();
    let page_count = pages.len();
    let collect_time = start.elapsed();
    println!(
        "[justpdf] Page count: {} (collected in {:?})",
        page_count, collect_time
    );

    let start = Instant::now();
    let _ = justpdf_core::text::extract_all_text(&doc);
    let text_time = start.elapsed();
    println!("[justpdf] Text extraction (all pages): {:?}", text_time);

    println!("[justpdf] Cached objects: {}", doc.cached_object_count());

    // --- MuPDF (mutool) comparison ---
    if which_mutool() {
        println!();

        let start = Instant::now();
        let output = std::process::Command::new("mutool")
            .args(["info", &path.to_string_lossy()])
            .output();
        let mutool_info_time = start.elapsed();
        match output {
            Ok(o) if o.status.success() => {
                println!("[mutool] info time: {:?}", mutool_info_time);
            }
            _ => println!("[mutool] info failed"),
        }

        // Use platform-appropriate null device
        let null_device = if cfg!(windows) { "NUL" } else { "/dev/null" };

        let start = Instant::now();
        let output = std::process::Command::new("mutool")
            .args([
                "draw",
                "-o",
                null_device,
                "-F",
                "png",
                &path.to_string_lossy(),
                "1",
            ])
            .output();
        let mutool_render_time = start.elapsed();
        match output {
            Ok(o) if o.status.success() => {
                println!("[mutool] render page 1 time: {:?}", mutool_render_time);
            }
            _ => println!("[mutool] render failed (or not found)"),
        }
    } else {
        println!("\n[mutool] Not found on PATH -- skipping MuPDF comparison");
    }
}

fn which_mutool() -> bool {
    std::process::Command::new("mutool")
        .arg("-v")
        .output()
        .is_ok()
}
