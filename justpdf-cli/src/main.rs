use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(name = "justpdf", version, about = "PDF tool powered by justpdf")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Display PDF information
    Info {
        /// Input PDF file
        file: PathBuf,
        /// Password for encrypted PDFs
        #[arg(long)]
        password: Option<String>,
    },
    /// Extract text from PDF
    Text {
        /// Input PDF file
        file: PathBuf,
        /// Page number (1-based, default: all pages)
        #[arg(long)]
        page: Option<usize>,
        /// Output format: plain, html, json, markdown
        #[arg(long, default_value = "plain")]
        format: String,
        /// Password for encrypted PDFs
        #[arg(long)]
        password: Option<String>,
    },
    /// Render PDF pages to images
    Render {
        /// Input PDF file
        file: PathBuf,
        /// Page number (1-based, default: 1)
        #[arg(long, default_value = "1")]
        page: usize,
        /// Render all pages
        #[arg(long)]
        all: bool,
        /// DPI resolution
        #[arg(long, default_value = "150")]
        dpi: f64,
        /// Output format: png, jpeg, svg
        #[arg(long, short = 'F', default_value = "png")]
        format: String,
        /// JPEG quality (1-100)
        #[arg(long, default_value = "85")]
        quality: u8,
        /// Output file or directory (for --all)
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Password for encrypted PDFs
        #[arg(long)]
        password: Option<String>,
    },
    /// Merge multiple PDF files
    Merge {
        /// Input PDF files
        files: Vec<PathBuf>,
        /// Output file
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Extract pages from a PDF
    Split {
        /// Input PDF file
        file: PathBuf,
        /// Page range (e.g., "1-5", "1,3,5", "2-")
        #[arg(long)]
        pages: String,
        /// Output file
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Encrypt a PDF file
    Encrypt {
        /// Input PDF file
        file: PathBuf,
        /// User password (required to open)
        #[arg(long)]
        user_password: Option<String>,
        /// Owner password (for full access)
        #[arg(long)]
        owner_password: String,
        /// Disable printing
        #[arg(long)]
        no_print: bool,
        /// Disable copying
        #[arg(long)]
        no_copy: bool,
        /// Output file
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Decrypt a PDF file
    Decrypt {
        /// Input PDF file
        file: PathBuf,
        /// Password
        #[arg(long)]
        password: String,
        /// Output file
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Optimize and clean a PDF
    Clean {
        /// Input PDF file
        file: PathBuf,
        /// Output file
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Compress a PDF (re-encode images, optimize structure)
    Compress(CompressArgs),
    /// Convert between document formats
    Convert {
        /// Input file
        file: PathBuf,
        /// Output file
        #[arg(short, long)]
        output: PathBuf,
        /// Output format (auto-detected from extension if not specified)
        #[arg(long, short = 'F')]
        format: Option<String>,
    },
    /// Digital signature (not yet fully implemented)
    Sign {
        /// Input PDF file
        file: PathBuf,
        /// Certificate file
        #[arg(long)]
        cert: PathBuf,
        /// Certificate password
        #[arg(long)]
        password: Option<String>,
        /// Output file
        #[arg(short, long)]
        output: PathBuf,
    },
}

/// Arguments for the `compress` subcommand.
///
/// A `--preset` supplies the base values; each knob flag below overrides its
/// preset value when given. On/off knobs use the `--x` / `--no-x` convention,
/// and clap rejects supplying both sides of a pair at once.
#[derive(clap::Args)]
struct CompressArgs {
    /// Input PDF file
    file: PathBuf,
    /// Compression preset: low, medium, high, extreme
    #[arg(long, default_value = "medium")]
    preset: String,
    /// Output file
    #[arg(short, long)]
    output: PathBuf,
    /// Password for encrypted PDFs (output is written WITHOUT encryption)
    #[arg(long)]
    password: Option<String>,

    // --- on/off knobs (each overrides the preset in either direction) ---
    /// Run structural optimization (GC + dedup + object streams)
    #[arg(long, conflicts_with = "no_structural")]
    structural: bool,
    /// Disable structural optimization
    #[arg(long)]
    no_structural: bool,
    /// Apply FlateDecode to uncompressed streams
    #[arg(long, conflicts_with = "no_compress_streams")]
    compress_streams: bool,
    /// Disable stream compression
    #[arg(long)]
    no_compress_streams: bool,
    /// Subset embedded fonts (drop unused glyphs)
    #[arg(long, conflicts_with = "no_font_subsetting")]
    font_subsetting: bool,
    /// Disable font subsetting
    #[arg(long)]
    no_font_subsetting: bool,
    /// Strip metadata (XMP, thumbnails, OutputIntents, ...)
    #[arg(long, conflicts_with = "no_strip_metadata")]
    strip_metadata: bool,
    /// Keep metadata
    #[arg(long)]
    no_strip_metadata: bool,
    /// Strip embedded files, JavaScript, and other extras
    #[arg(long, conflicts_with = "no_strip_extras")]
    strip_extras: bool,
    /// Keep embedded files and extras
    #[arg(long)]
    no_strip_extras: bool,
    /// Convert color images to grayscale
    #[arg(long, conflicts_with = "no_grayscale")]
    grayscale: bool,
    /// Keep image color
    #[arg(long)]
    no_grayscale: bool,

    // --- numeric knobs ---
    /// JPEG quality for image re-encoding (1-100)
    #[arg(long, value_parser = clap::value_parser!(u8).range(1..=100), conflicts_with = "no_image_recompress")]
    jpeg_quality: Option<u8>,
    /// Disable image re-encoding entirely
    #[arg(long)]
    no_image_recompress: bool,
    /// Maximum image DPI (downscale above this)
    #[arg(long, conflicts_with = "no_downscale")]
    max_dpi: Option<f64>,
    /// Disable image downscaling entirely
    #[arg(long)]
    no_downscale: bool,
    /// Skip images smaller than this many bytes
    #[arg(long)]
    skip_below: Option<usize>,
}

fn main() {
    let cli = Cli::parse();
    if let Err(e) = run(cli) {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    match cli.command {
        Commands::Info { file, password } => cmd_info(&file, password.as_deref()),
        Commands::Text {
            file,
            page,
            format,
            password,
        } => cmd_text(&file, page, &format, password.as_deref()),
        Commands::Render {
            file,
            page,
            all,
            dpi,
            format,
            quality,
            output,
            password,
        } => cmd_render(
            &file,
            page,
            all,
            dpi,
            &format,
            quality,
            output,
            password.as_deref(),
        ),
        Commands::Merge { files, output } => cmd_merge(&files, &output),
        Commands::Split {
            file,
            pages,
            output,
        } => cmd_split(&file, &pages, &output),
        Commands::Encrypt {
            file,
            user_password,
            owner_password,
            no_print,
            no_copy,
            output,
        } => cmd_encrypt(
            &file,
            user_password.as_deref(),
            &owner_password,
            no_print,
            no_copy,
            &output,
        ),
        Commands::Decrypt {
            file,
            password,
            output,
        } => cmd_decrypt(&file, &password, &output),
        Commands::Clean { file, output } => cmd_clean(&file, &output),
        Commands::Compress(args) => cmd_compress(&args),
        Commands::Convert {
            file,
            output,
            format,
        } => cmd_convert(&file, &output, format.as_deref()),
        Commands::Sign { .. } => {
            eprintln!("Digital signature support is not yet fully implemented.");
            Ok(())
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn open_doc(
    path: &Path,
    password: Option<&str>,
) -> Result<justpdf_core::PdfDocument, Box<dyn std::error::Error>> {
    let mut doc = justpdf_core::PdfDocument::open(path)?;
    if doc.is_encrypted() && !doc.is_authenticated() {
        if let Some(pw) = password {
            doc.authenticate(pw.as_bytes())?;
        } else {
            return Err("PDF is encrypted. Use --password to provide the password.".into());
        }
    }
    Ok(doc)
}

fn format_size(bytes: u64) -> String {
    if bytes > 1_048_576 {
        format!("{:.2} MB", bytes as f64 / 1_048_576.0)
    } else {
        format!("{:.2} KB", bytes as f64 / 1024.0)
    }
}

fn print_info_field(info: &justpdf_core::PdfDict, key: &str) {
    if let Some(obj) = info.get(key.as_bytes()) {
        if let Some(s) = obj.as_str() {
            let text = String::from_utf8_lossy(s);
            println!("{key}: {text}");
        }
    }
}

// ---------------------------------------------------------------------------
// info
// ---------------------------------------------------------------------------

fn cmd_info(file: &Path, password: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let doc = open_doc(file, password)?;
    let pages = justpdf_core::page::collect_pages(&doc)?;

    println!("File: {}", file.display());
    println!("Version: {}.{}", doc.version.0, doc.version.1);
    println!("Pages: {}", pages.len());

    // Get metadata from Info dict
    if let Some(info_ref) = doc.trailer().get_ref(b"Info") {
        if let Ok(info_obj) = doc.resolve(&info_ref.clone()) {
            if let Some(info) = info_obj.as_dict() {
                print_info_field(info, "Title");
                print_info_field(info, "Author");
                print_info_field(info, "Subject");
                print_info_field(info, "Creator");
                print_info_field(info, "Producer");
            }
        }
    }

    println!(
        "Encrypted: {}",
        if doc.is_encrypted() { "Yes" } else { "No" }
    );

    let file_size = std::fs::metadata(file)?.len();
    println!("File size: {}", format_size(file_size));

    Ok(())
}

// ---------------------------------------------------------------------------
// text
// ---------------------------------------------------------------------------

fn cmd_text(
    file: &Path,
    page: Option<usize>,
    format: &str,
    password: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let doc = open_doc(file, password)?;

    let output_format = match format {
        "html" => justpdf_core::text::format::OutputFormat::Html,
        "json" => justpdf_core::text::format::OutputFormat::Json,
        "markdown" | "md" => justpdf_core::text::format::OutputFormat::Markdown,
        "plain" => justpdf_core::text::format::OutputFormat::PlainText,
        other => {
            return Err(
                format!("Unknown format: {other}. Use plain, html, json, or markdown.").into(),
            );
        }
    };

    if let Some(page_num) = page {
        let page_info = justpdf_core::page::get_page(&doc, page_num - 1)?;
        let page_text = justpdf_core::text::extract_page_text(&doc, &page_info)?;
        print!(
            "{}",
            justpdf_core::text::format::format_page(&page_text, output_format)
        );
    } else {
        let pages_text = justpdf_core::text::extract_all_text(&doc)?;
        print!(
            "{}",
            justpdf_core::text::format::format_pages(&pages_text, output_format)
        );
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// render
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn cmd_render(
    file: &Path,
    page: usize,
    all: bool,
    dpi: f64,
    format: &str,
    quality: u8,
    output: Option<PathBuf>,
    password: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let doc = open_doc(file, password)?;
    let pages = justpdf_core::page::collect_pages(&doc)?;

    let render_indices: Vec<usize> = if all {
        (0..pages.len()).collect()
    } else {
        vec![page - 1]
    };

    for &idx in &render_indices {
        if idx >= pages.len() {
            return Err(format!("Page {} out of range (total: {})", idx + 1, pages.len()).into());
        }

        let page_info = &pages[idx];
        let output_path = if all {
            let dir = output.as_deref().unwrap_or(Path::new("."));
            dir.join(format!("page_{:03}.{}", idx + 1, format))
        } else {
            output
                .clone()
                .unwrap_or_else(|| PathBuf::from(format!("output.{format}")))
        };

        match format {
            "svg" => {
                let svg = justpdf_render::render_page_to_svg(&doc, idx)?;
                std::fs::write(&output_path, svg)?;
            }
            _ => {
                let opts = justpdf_render::RenderOptions {
                    dpi,
                    format: match format {
                        "jpeg" | "jpg" => justpdf_render::OutputFormat::Jpeg { quality },
                        _ => justpdf_render::OutputFormat::Png,
                    },
                    ..Default::default()
                };
                let data = justpdf_render::render::render_page_info(&doc, page_info, &opts)?;
                std::fs::write(&output_path, &data)?;
            }
        }

        eprintln!("Rendered page {} -> {}", idx + 1, output_path.display());
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// merge
// ---------------------------------------------------------------------------

fn cmd_merge(files: &[PathBuf], output: &Path) -> Result<(), Box<dyn std::error::Error>> {
    if files.len() < 2 {
        return Err("At least two input files are required for merging.".into());
    }

    let docs: Vec<justpdf_core::PdfDocument> = files
        .iter()
        .map(|f| justpdf_core::PdfDocument::open(f))
        .collect::<justpdf_core::Result<Vec<_>>>()?;
    let refs: Vec<&justpdf_core::PdfDocument> = docs.iter().collect();
    let merged = justpdf_core::writer::merge_documents(&refs)?;
    std::fs::write(output, merged)?;
    eprintln!("Merged {} files -> {}", files.len(), output.display());
    Ok(())
}

// ---------------------------------------------------------------------------
// split
// ---------------------------------------------------------------------------

fn cmd_split(
    file: &Path,
    pages_str: &str,
    output: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let doc = justpdf_core::PdfDocument::open(file)?;
    let total = justpdf_core::page::page_count(&doc)?;
    let indices = parse_page_range(pages_str, total)?;

    if indices.is_empty() {
        return Err("No pages selected.".into());
    }

    let mut modifier = justpdf_core::writer::DocumentModifier::from_document(&doc)?;
    modifier.reorder_pages(&indices)?;
    let result = modifier.build()?;
    std::fs::write(output, result)?;
    eprintln!("Extracted {} pages -> {}", indices.len(), output.display());
    Ok(())
}

fn parse_page_range(s: &str, total: usize) -> Result<Vec<usize>, Box<dyn std::error::Error>> {
    let mut result = Vec::new();
    for part in s.split(',') {
        let part = part.trim();
        if let Some(dash_pos) = part.find('-') {
            let start_str = &part[..dash_pos];
            let end_str = &part[dash_pos + 1..];
            let start: usize = if start_str.is_empty() {
                1
            } else {
                start_str.parse()?
            };
            let end: usize = if end_str.is_empty() {
                total
            } else {
                end_str.parse()?
            };
            for i in start..=end.min(total) {
                result.push(i - 1); // Convert 1-based to 0-based
            }
        } else {
            let page: usize = part.parse()?;
            if page == 0 || page > total {
                return Err(format!("Page {page} out of range (1-{total})").into());
            }
            result.push(page - 1);
        }
    }
    Ok(result)
}

// ---------------------------------------------------------------------------
// encrypt
// ---------------------------------------------------------------------------

fn cmd_encrypt(
    file: &Path,
    user_pw: Option<&str>,
    owner_pw: &str,
    no_print: bool,
    no_copy: bool,
    output: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let doc = justpdf_core::PdfDocument::open(file)?;

    // Build permissions
    let mut perm_bits: i32 = -4; // allow all by default
    if no_print {
        perm_bits &= !(1 << 2); // clear print permission
    }
    if no_copy {
        perm_bits &= !(1 << 4); // clear copy permission
    }

    let config = justpdf_core::crypto::EncryptionConfig {
        user_password: user_pw.unwrap_or("").as_bytes().to_vec(),
        owner_password: owner_pw.as_bytes().to_vec(),
        permissions: justpdf_core::crypto::Permissions::new(perm_bits),
        method: justpdf_core::crypto::EncryptionMethod::AES128,
        encrypt_metadata: true,
    };

    // Rebuild the document with encryption using DocumentBuilder approach:
    // Load all objects into a modifier, then serialize with encryption.
    let mut modifier = justpdf_core::writer::DocumentModifier::from_document(&doc)?;

    // We need to use the DocumentBuilder encryption path.
    // Since DocumentModifier doesn't directly support encryption,
    // we rebuild via DocumentBuilder-like serialization.
    let file_id = justpdf_core::crypto::generate_file_id(b"justpdf", 0);
    let (state, encrypt_dict, id_array) = config.build(&file_id)?;

    let encrypt_ref = modifier.add_object(justpdf_core::PdfObject::Dict(encrypt_dict));

    let mut state = state;
    state.encrypt_obj_num = Some(encrypt_ref.obj_num);

    let catalog_ref = modifier.catalog_ref().clone();

    // Access internal writer for serialization
    let writer = modifier.writer();
    let result = justpdf_core::writer::serialize_pdf_encrypted(
        &writer.objects,
        writer.version,
        &catalog_ref,
        None,
        &encrypt_ref,
        &state,
        &id_array,
    )?;

    std::fs::write(output, result)?;
    eprintln!("Encrypted -> {}", output.display());
    Ok(())
}

// ---------------------------------------------------------------------------
// decrypt
// ---------------------------------------------------------------------------

fn cmd_decrypt(
    file: &Path,
    password: &str,
    output: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut doc = justpdf_core::PdfDocument::open(file)?;

    if !doc.is_encrypted() {
        eprintln!("File is not encrypted.");
        return Ok(());
    }

    doc.authenticate(password.as_bytes())?;

    // Re-serialize without encryption
    let modifier = justpdf_core::writer::DocumentModifier::from_document(&doc)?;
    let result = modifier.build()?;
    std::fs::write(output, result)?;
    eprintln!("Decrypted -> {}", output.display());
    Ok(())
}

// ---------------------------------------------------------------------------
// clean
// ---------------------------------------------------------------------------

fn cmd_clean(file: &Path, output: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let doc = justpdf_core::PdfDocument::open(file)?;
    let mut modifier = justpdf_core::writer::DocumentModifier::from_document(&doc)?;
    modifier.garbage_collect();
    let result = modifier.build()?;
    std::fs::write(output, &result)?;

    let orig_size = std::fs::metadata(file)?.len();
    let new_size = result.len() as u64;
    let reduction = if orig_size > 0 {
        (1.0 - new_size as f64 / orig_size as f64) * 100.0
    } else {
        0.0
    };
    eprintln!(
        "Cleaned: {} -> {} ({:.1}% reduction)",
        format_size(orig_size),
        format_size(new_size),
        reduction
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// compress
// ---------------------------------------------------------------------------

/// Resolve an on/off knob: an explicit flag wins, otherwise the preset stands.
/// clap guarantees `on` and `off` are never both set.
fn resolve_bool(base: bool, on: bool, off: bool) -> bool {
    if on {
        true
    } else if off {
        false
    } else {
        base
    }
}

/// Build the effective `CompressOptions` from a preset plus knob overrides.
/// Each override wins over the preset; absent knobs leave the preset value.
fn resolve_options(args: &CompressArgs) -> Result<justpdf_core::writer::CompressOptions, String> {
    let mut options =
        justpdf_core::writer::CompressOptions::from_preset(&args.preset).ok_or_else(|| {
            format!(
                "Unknown preset: {}. Use low, medium, high, or extreme.",
                args.preset
            )
        })?;

    // Apply knob overrides on top of the preset.
    options.structural = resolve_bool(options.structural, args.structural, args.no_structural);
    options.compress_streams = resolve_bool(
        options.compress_streams,
        args.compress_streams,
        args.no_compress_streams,
    );
    options.font_subsetting = resolve_bool(
        options.font_subsetting,
        args.font_subsetting,
        args.no_font_subsetting,
    );
    options.strip_metadata = resolve_bool(
        options.strip_metadata,
        args.strip_metadata,
        args.no_strip_metadata,
    );
    options.strip_extras = resolve_bool(
        options.strip_extras,
        args.strip_extras,
        args.no_strip_extras,
    );
    options.grayscale = resolve_bool(options.grayscale, args.grayscale, args.no_grayscale);

    if args.no_image_recompress {
        options.jpeg_quality = None;
    } else if let Some(q) = args.jpeg_quality {
        options.jpeg_quality = Some(q);
    }
    if args.no_downscale {
        options.max_image_dpi = None;
    } else if let Some(dpi) = args.max_dpi {
        options.max_image_dpi = Some(dpi);
    }
    if let Some(bytes) = args.skip_below {
        options.skip_below_bytes = bytes;
    }

    Ok(options)
}

fn cmd_compress(args: &CompressArgs) -> Result<(), Box<dyn std::error::Error>> {
    let options = resolve_options(args)?;

    // compress_pdf rejects encrypted PDFs, so decrypt first when needed.
    // open_doc authenticates with --password and errors if it is missing/wrong.
    let doc = open_doc(&args.file, args.password.as_deref())?;
    let was_encrypted = doc.is_encrypted();
    let data = if was_encrypted {
        justpdf_core::writer::DocumentModifier::from_document(&doc)?.build()?
    } else {
        std::fs::read(&args.file)?
    };

    let (compressed, stats) = justpdf_core::writer::compress_pdf(&data, &options)?;
    std::fs::write(&args.output, &compressed)?;

    let reduction = if stats.original_size > 0 {
        (1.0 - stats.compressed_size as f64 / stats.original_size as f64) * 100.0
    } else {
        0.0
    };
    eprintln!(
        "Compressed: {} -> {} ({:.1}% reduction)",
        format_size(stats.original_size as u64),
        format_size(stats.compressed_size as u64),
        reduction
    );
    if was_encrypted {
        eprintln!("Warning: input was encrypted; output is unencrypted (encryption removed).");
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// convert
// ---------------------------------------------------------------------------

fn cmd_convert(
    file: &Path,
    output: &Path,
    format: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    use justpdf_formats::common::FormatDocument;
    use justpdf_formats::detect::{DocumentFormat, detect_format};

    let input_format = detect_format(file);
    let output_ext =
        format.unwrap_or_else(|| output.extension().and_then(|e| e.to_str()).unwrap_or("pdf"));

    match input_format {
        DocumentFormat::PlainText => {
            let doc = justpdf_formats::plaintext::PlainTextDocument::open(file)?;
            match output_ext {
                "pdf" => {
                    std::fs::write(output, doc.to_pdf()?)?;
                }
                "png" => {
                    std::fs::write(output, doc.render_page_png(0, 150.0)?)?;
                }
                _ => return Err(format!("unsupported output format: {output_ext}").into()),
            }
        }
        DocumentFormat::Svg => {
            let doc = justpdf_formats::svg::SvgDocument::open(file)?;
            match output_ext {
                "pdf" => {
                    std::fs::write(output, doc.to_pdf()?)?;
                }
                "png" => {
                    std::fs::write(output, doc.render_page_png(0, 150.0)?)?;
                }
                _ => return Err(format!("unsupported output format: {output_ext}").into()),
            }
        }
        DocumentFormat::Epub => {
            let doc = justpdf_formats::epub::EpubDocument::open(file)?;
            match output_ext {
                "pdf" => {
                    std::fs::write(output, doc.to_pdf()?)?;
                }
                "png" => {
                    std::fs::write(output, doc.render_page_png(0, 150.0)?)?;
                }
                _ => return Err(format!("unsupported output format: {output_ext}").into()),
            }
        }
        DocumentFormat::Cbz => {
            let doc = justpdf_formats::cbz::CbzDocument::open(file)?;
            match output_ext {
                "pdf" => {
                    std::fs::write(output, doc.to_pdf()?)?;
                }
                "png" => {
                    std::fs::write(output, doc.render_page_png(0, 150.0)?)?;
                }
                _ => return Err(format!("unsupported output format: {output_ext}").into()),
            }
        }
        DocumentFormat::Xps => {
            let doc = justpdf_formats::xps::XpsDocument::open(file)?;
            match output_ext {
                "pdf" => {
                    std::fs::write(output, doc.to_pdf()?)?;
                }
                "png" => {
                    std::fs::write(output, doc.render_page_png(0, 150.0)?)?;
                }
                _ => return Err(format!("unsupported output format: {output_ext}").into()),
            }
        }
        DocumentFormat::Docx | DocumentFormat::Xlsx | DocumentFormat::Pptx => {
            let doc = justpdf_formats::office::OfficeDocument::open(file)?;
            match output_ext {
                "pdf" => {
                    std::fs::write(output, doc.to_pdf()?)?;
                }
                "png" => {
                    std::fs::write(output, doc.render_page_png(0, 150.0)?)?;
                }
                _ => return Err(format!("unsupported output format: {output_ext}").into()),
            }
        }
        DocumentFormat::Pdf => match output_ext {
            "svg" => {
                let doc = justpdf_core::PdfDocument::open(file)?;
                let svg = justpdf_render::render_page_to_svg(&doc, 0)?;
                std::fs::write(output, svg)?;
            }
            "png" => {
                let doc = justpdf_core::PdfDocument::open(file)?;
                let opts = justpdf_render::RenderOptions {
                    dpi: 150.0,
                    format: justpdf_render::OutputFormat::Png,
                    ..Default::default()
                };
                let data = justpdf_render::render_page(&doc, 0, &opts)?;
                std::fs::write(output, data)?;
            }
            "txt" => {
                let doc = justpdf_core::PdfDocument::open(file)?;
                let text = justpdf_core::text::extract_all_text_string(&doc)?;
                std::fs::write(output, text)?;
            }
            _ => return Err(format!("unsupported output: {output_ext}").into()),
        },
        _ => return Err(format!("unsupported input format: {input_format}").into()),
    }

    eprintln!("Converted {} -> {}", file.display(), output.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A `CompressArgs` with every knob left unset (preset values pass through).
    fn args_for(preset: &str) -> CompressArgs {
        CompressArgs {
            file: PathBuf::from("in.pdf"),
            preset: preset.to_string(),
            output: PathBuf::from("out.pdf"),
            password: None,
            structural: false,
            no_structural: false,
            compress_streams: false,
            no_compress_streams: false,
            font_subsetting: false,
            no_font_subsetting: false,
            strip_metadata: false,
            no_strip_metadata: false,
            strip_extras: false,
            no_strip_extras: false,
            grayscale: false,
            no_grayscale: false,
            jpeg_quality: None,
            no_image_recompress: false,
            max_dpi: None,
            no_downscale: false,
            skip_below: None,
        }
    }

    #[test]
    fn unset_knobs_preserve_the_preset() {
        let preset = justpdf_core::writer::CompressOptions::preset_high();
        let resolved = resolve_options(&args_for("high")).unwrap();

        assert_eq!(resolved.jpeg_quality, preset.jpeg_quality);
        assert_eq!(resolved.max_image_dpi, preset.max_image_dpi);
        assert_eq!(resolved.skip_below_bytes, preset.skip_below_bytes);
        assert_eq!(resolved.structural, preset.structural);
        assert_eq!(resolved.compress_streams, preset.compress_streams);
        assert_eq!(resolved.font_subsetting, preset.font_subsetting);
        assert_eq!(
            resolved.remove_unused_resources,
            preset.remove_unused_resources
        );
        assert_eq!(resolved.strip_metadata, preset.strip_metadata);
        assert_eq!(resolved.strip_extras, preset.strip_extras);
        assert_eq!(resolved.grayscale, preset.grayscale);
    }

    #[test]
    fn high_with_two_overrides_changes_only_those_two() {
        // The acceptance example: high base, keep metadata, bump quality to 80.
        let base = justpdf_core::writer::CompressOptions::preset_high();
        let mut args = args_for("high");
        args.no_strip_metadata = true;
        args.jpeg_quality = Some(80);

        let resolved = resolve_options(&args).unwrap();

        // The two overridden fields differ from the preset...
        assert_eq!(resolved.jpeg_quality, Some(80));
        assert!(!resolved.strip_metadata);
        assert!(base.strip_metadata, "high should strip metadata by default");

        // ...and everything else still matches high.
        assert_eq!(resolved.max_image_dpi, base.max_image_dpi);
        assert_eq!(resolved.skip_below_bytes, base.skip_below_bytes);
        assert_eq!(resolved.structural, base.structural);
        assert_eq!(resolved.compress_streams, base.compress_streams);
        assert_eq!(resolved.font_subsetting, base.font_subsetting);
        assert_eq!(resolved.strip_extras, base.strip_extras);
        assert_eq!(resolved.grayscale, base.grayscale);
    }

    #[test]
    fn on_off_knobs_override_in_both_directions() {
        // low leaves font_subsetting off; turn it on.
        let mut on = args_for("low");
        on.font_subsetting = true;
        assert!(resolve_options(&on).unwrap().font_subsetting);

        // high turns font_subsetting on; turn it off.
        let mut off = args_for("high");
        off.no_font_subsetting = true;
        assert!(!resolve_options(&off).unwrap().font_subsetting);
    }

    #[test]
    fn disable_flags_clear_the_image_options() {
        let mut args = args_for("high");
        args.no_image_recompress = true;
        args.no_downscale = true;
        let resolved = resolve_options(&args).unwrap();
        assert_eq!(resolved.jpeg_quality, None);
        assert_eq!(resolved.max_image_dpi, None);
    }

    #[test]
    fn numeric_overrides_replace_preset_values() {
        let mut args = args_for("medium");
        args.max_dpi = Some(200.0);
        args.skip_below = Some(123);
        let resolved = resolve_options(&args).unwrap();
        assert_eq!(resolved.max_image_dpi, Some(200.0));
        assert_eq!(resolved.skip_below_bytes, 123);
    }

    #[test]
    fn unknown_preset_is_rejected() {
        assert!(resolve_options(&args_for("bogus")).is_err());
    }
}
