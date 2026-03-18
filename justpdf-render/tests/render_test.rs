use std::path::Path;

use justpdf_core::PdfDocument;
use justpdf_render::{OutputFormat, RenderOptions, render_page, render_page_to_svg};

#[test]
fn test_render_page_produces_png() {
    let pdf_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../testpdf.pdf");
    if !pdf_path.exists() {
        eprintln!("skipping: testpdf.pdf not found");
        return;
    }

    let mut doc = PdfDocument::open(&pdf_path).expect("failed to open PDF");
    let options = RenderOptions {
        dpi: 72.0,
        ..Default::default()
    };

    let png_data = render_page(&mut doc, 0, &options).expect("failed to render page 0");

    // Check PNG signature
    assert!(png_data.len() > 8, "PNG output too small");
    assert_eq!(&png_data[..4], &[0x89, b'P', b'N', b'G'], "not a valid PNG");
}

#[test]
fn test_render_page_at_150dpi() {
    let pdf_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../testpdf.pdf");
    if !pdf_path.exists() {
        return;
    }

    let mut doc = PdfDocument::open(&pdf_path).expect("failed to open PDF");
    let options = RenderOptions {
        dpi: 150.0,
        ..Default::default()
    };

    let png_data = render_page(&mut doc, 0, &options).expect("failed to render at 150 DPI");
    assert!(png_data.len() > 100);
}

#[test]
fn test_render_page_out_of_range() {
    let pdf_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../testpdf.pdf");
    if !pdf_path.exists() {
        return;
    }

    let mut doc = PdfDocument::open(&pdf_path).expect("failed to open PDF");
    let options = RenderOptions::default();

    let result = render_page(&mut doc, 999, &options);
    assert!(result.is_err());
}

#[test]
fn test_render_multiple_pages() {
    let pdf_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../testpdf.pdf");
    if !pdf_path.exists() {
        return;
    }

    let mut doc = PdfDocument::open(&pdf_path).expect("failed to open PDF");
    let options = RenderOptions {
        dpi: 72.0,
        ..Default::default()
    };

    // Render first 3 pages
    for i in 0..3 {
        let result = render_page(&mut doc, i, &options);
        assert!(result.is_ok(), "failed to render page {i}: {:?}", result.err());
    }
}

#[test]
fn test_render_jpeg_output() {
    let pdf_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../testpdf.pdf");
    if !pdf_path.exists() {
        return;
    }

    let mut doc = PdfDocument::open(&pdf_path).expect("failed to open PDF");
    let options = RenderOptions {
        dpi: 72.0,
        format: OutputFormat::Jpeg { quality: 85 },
        ..Default::default()
    };

    let jpeg_data = render_page(&mut doc, 0, &options).expect("failed to render JPEG");

    // Check JPEG signature (SOI marker)
    assert!(jpeg_data.len() > 2, "JPEG output too small");
    assert_eq!(jpeg_data[0], 0xFF, "not a valid JPEG");
    assert_eq!(jpeg_data[1], 0xD8, "not a valid JPEG");
}

#[test]
fn test_render_all_20_pages() {
    let pdf_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../testpdf.pdf");
    if !pdf_path.exists() {
        return;
    }

    let mut doc = PdfDocument::open(&pdf_path).expect("failed to open PDF");
    let options = RenderOptions {
        dpi: 72.0,
        ..Default::default()
    };

    for i in 0..20 {
        let result = render_page(&mut doc, i, &options);
        assert!(result.is_ok(), "failed to render page {i}: {:?}", result.err());
    }
}

#[test]
fn test_render_page_to_svg() {
    let pdf_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../testpdf.pdf");
    if !pdf_path.exists() {
        eprintln!("skipping: testpdf.pdf not found");
        return;
    }

    let mut doc = PdfDocument::open(&pdf_path).expect("failed to open PDF");
    let svg = render_page_to_svg(&mut doc, 0).expect("failed to render SVG");

    assert!(
        svg.starts_with("<?xml") || svg.starts_with("<svg"),
        "SVG output should start with <?xml or <svg, got: {}",
        &svg[..svg.len().min(80)],
    );
    assert!(svg.contains("<svg"), "SVG output should contain <svg element");
    assert!(svg.contains("</svg>"), "SVG output should contain closing </svg>");
    assert!(svg.len() > 100, "SVG output too small: {} bytes", svg.len());
}
