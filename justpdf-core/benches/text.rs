use criterion::{Criterion, criterion_group, criterion_main};
use justpdf_core::font::FontProgram;
use justpdf_core::page::collect_pages;
use justpdf_core::parser::PdfDocument;
use justpdf_core::text::{extract_all_text_string, extract_page_text};
use justpdf_core::writer::document::DocumentBuilder;
use justpdf_core::writer::page::PageBuilder;

fn create_text_pdf(num_pages: usize, lines_per_page: usize) -> Vec<u8> {
    let mut doc = DocumentBuilder::new();
    let font = doc.add_standard_font("Helvetica");
    for i in 0..num_pages {
        let mut page = PageBuilder::new(612.0, 792.0);
        page.begin_text();
        page.set_font(&font, 12.0);
        for line in 0..lines_per_page {
            let y = 720.0 - (line as f64 * 14.0);
            page.move_to(72.0, y);
            page.show_text(&format!(
                "Page {} line {}: The quick brown fox jumps over the lazy dog.",
                i + 1,
                line + 1
            ));
        }
        page.end_text();
        doc.add_page(page);
    }
    doc.build().unwrap()
}

fn bench_extract_page_text(c: &mut Criterion) {
    let pdf_data = create_text_pdf(1, 20);
    c.bench_function("extract_text_1page_20lines", |b| {
        b.iter(|| {
            let doc = PdfDocument::from_bytes(pdf_data.clone()).unwrap();
            let pages = collect_pages(&doc).unwrap();
            for page in &pages {
                let _ = extract_page_text(&doc, page);
            }
        })
    });
}

fn bench_extract_all_text(c: &mut Criterion) {
    let small = create_text_pdf(5, 10);
    let medium = create_text_pdf(20, 20);

    c.bench_function("extract_all_text_5pages", |b| {
        b.iter(|| {
            let doc = PdfDocument::from_bytes(small.clone()).unwrap();
            extract_all_text_string(&doc).unwrap()
        })
    });
    c.bench_function("extract_all_text_20pages", |b| {
        b.iter(|| {
            let doc = PdfDocument::from_bytes(medium.clone()).unwrap();
            extract_all_text_string(&doc).unwrap()
        })
    });
}

fn bench_collect_pages(c: &mut Criterion) {
    let pdf_data = create_text_pdf(50, 5);
    c.bench_function("collect_pages_50pages", |b| {
        b.iter(|| {
            let doc = PdfDocument::from_bytes(pdf_data.clone()).unwrap();
            collect_pages(&doc).unwrap()
        })
    });
}

const HANGUL: &[u8] = include_bytes!("../tests/fixtures/JustpdfTestHangul-Regular.ttf");
const NOTO_SANS: &[u8] = include_bytes!("../tests/fixtures/NotoSans-Regular.ttf");

fn bench_show_text(c: &mut Criterion) {
    let line = "The quick brown fox jumps over the lazy dog 한글 ".repeat(2);
    let mut doc = DocumentBuilder::new();
    let hangul = doc.add_font(&FontProgram::parse(HANGUL, 0).unwrap());
    let noto = doc.add_font(&FontProgram::parse(NOTO_SANS, 0).unwrap());
    let embedded = hangul.with_fallback(&noto);
    let helvetica = doc.add_standard_font("Helvetica");
    for (name, font) in [
        ("show_text_embedded_fallback_100lines", &embedded),
        ("show_text_helvetica_100lines", &helvetica),
    ] {
        c.bench_function(name, |b| {
            b.iter(|| {
                let mut page = PageBuilder::new(612.0, 792.0);
                page.begin_text();
                page.set_font(font, 12.0);
                for _ in 0..100 {
                    page.show_text(&line);
                }
                page.end_text();
                page
            })
        });
    }
}

criterion_group!(
    benches,
    bench_show_text,
    bench_extract_page_text,
    bench_extract_all_text,
    bench_collect_pages
);
criterion_main!(benches);
