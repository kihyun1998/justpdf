use std::path::Path;

use justpdf_core::{IndirectRef, PdfDocument, PdfObject};

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 4 {
        eprintln!("Usage: decode_stream <pdf-file> --obj N");
        eprintln!("  Decode and display a stream object's data.");
        std::process::exit(1);
    }

    let path = Path::new(&args[1]);
    let obj_num: u32 = args
        .iter()
        .position(|a| a == "--obj")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| {
            eprintln!("Error: --obj N is required");
            std::process::exit(1);
        });

    let mut doc = match PdfDocument::open(path) {
        Ok(doc) => doc,
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    };

    let iref = IndirectRef {
        obj_num,
        gen_num: 0,
    };

    let obj = match doc.resolve(&iref) {
        Ok(obj) => obj.clone(),
        Err(e) => {
            eprintln!("Error resolving object {obj_num}: {e}");
            std::process::exit(1);
        }
    };

    match &obj {
        PdfObject::Stream { dict, data } => {
            println!("Stream object {} 0:", obj_num);
            println!("  Dict: {}", PdfObject::Dict(dict.clone()));
            println!("  Raw size: {} bytes", data.len());

            match doc.decode_stream(dict, data) {
                Ok(decoded) => {
                    println!("  Decoded size: {} bytes", decoded.len());
                    println!();

                    // Try to display as text
                    if let Ok(text) = std::str::from_utf8(&decoded) {
                        println!("--- Text content ---");
                        println!("{text}");
                    } else {
                        println!("--- Hex dump (first 256 bytes) ---");
                        hex_dump(&decoded, 256);
                    }
                }
                Err(e) => {
                    eprintln!("  Decode error: {e}");
                    std::process::exit(1);
                }
            }
        }
        _ => {
            eprintln!(
                "Object {obj_num} is not a stream (type: {})",
                obj.type_name()
            );
            println!("Value: {obj}");
            std::process::exit(1);
        }
    }
}

fn hex_dump(data: &[u8], max_bytes: usize) {
    let len = data.len().min(max_bytes);
    for (i, chunk) in data[..len].chunks(16).enumerate() {
        let offset = i * 16;
        print!("{offset:08X}  ");

        for (j, &b) in chunk.iter().enumerate() {
            print!("{b:02X} ");
            if j == 7 {
                print!(" ");
            }
        }

        // Padding
        for _ in chunk.len()..16 {
            print!("   ");
        }
        if chunk.len() <= 8 {
            print!(" ");
        }

        print!(" |");
        for &b in chunk {
            if b.is_ascii_graphic() || b == b' ' {
                print!("{}", b as char);
            } else {
                print!(".");
            }
        }
        println!("|");
    }

    if data.len() > max_bytes {
        println!("... ({} more bytes)", data.len() - max_bytes);
    }
}
