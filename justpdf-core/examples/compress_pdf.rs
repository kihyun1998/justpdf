use std::env;
use std::fs;

use justpdf_core::writer::compress::{compress_pdf, analyze_pdf, CompressOptions};

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: compress_pdf <input.pdf> [preset] [output.pdf]");
        eprintln!("Presets: low, medium, high, extreme (default: high)");
        std::process::exit(1);
    }

    let input_path = &args[1];
    let preset = args.get(2).map(|s| s.as_str()).unwrap_or("high");
    let output_path = args.get(3).cloned().unwrap_or_else(|| {
        let stem = std::path::Path::new(input_path)
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy();
        format!("{}_compressed.pdf", stem)
    });

    // Read input
    let data = fs::read(input_path).unwrap_or_else(|e| {
        eprintln!("Error reading {}: {}", input_path, e);
        std::process::exit(1);
    });

    let original_size = data.len();
    println!("Input:  {} ({:.2} MB)", input_path, original_size as f64 / 1_048_576.0);

    // Analyze
    match analyze_pdf(&data) {
        Ok(info) => {
            println!("Pages:  {}", info.pages);
            println!("Images: {} ({:.2} MB raw)", info.images, info.total_image_bytes as f64 / 1_048_576.0);
            if info.is_encrypted {
                eprintln!("Error: PDF is encrypted");
                std::process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("Analyze error: {}", e);
            std::process::exit(1);
        }
    }

    // Compress
    let options = CompressOptions::from_preset(preset).unwrap_or_else(|| {
        eprintln!("Unknown preset: {}", preset);
        std::process::exit(1);
    });

    println!("\nCompressing with preset '{}'...", preset);

    let (compressed, stats) = match compress_pdf(&data, &options) {
        Ok(result) => result,
        Err(e) => {
            eprintln!("Compress error: {}", e);
            std::process::exit(1);
        }
    };

    // Save
    fs::write(&output_path, &compressed).unwrap_or_else(|e| {
        eprintln!("Error writing {}: {}", output_path, e);
        std::process::exit(1);
    });

    // Report
    let ratio = stats.compressed_size as f64 / stats.original_size as f64;
    let saved = stats.original_size - stats.compressed_size;
    let saved_pct = (1.0 - ratio) * 100.0;

    println!("\n--- Result ---");
    println!("Output: {} ({:.2} MB)", output_path, stats.compressed_size as f64 / 1_048_576.0);
    println!("Saved:  {:.2} MB ({:.1}% reduction)", saved as f64 / 1_048_576.0, saved_pct);
    println!("Images found:        {}", stats.images_found);
    println!("Images recompressed: {}", stats.images_recompressed);
    println!("Images downscaled:   {}", stats.images_downscaled);
    println!("Images skipped:      {}", stats.images_skipped);
    println!("Objects removed (GC): {}", stats.objects_removed_gc);
}
