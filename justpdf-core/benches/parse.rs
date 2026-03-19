use criterion::{criterion_group, criterion_main, Criterion};
use justpdf_core::parser::PdfDocument;
use justpdf_core::writer::document::DocumentBuilder;
use justpdf_core::writer::page::PageBuilder;

fn create_test_pdf(num_pages: usize) -> Vec<u8> {
    let mut doc = DocumentBuilder::new();
    let font = doc.add_standard_font("Helvetica");
    for i in 0..num_pages {
        let mut page = PageBuilder::new(612.0, 792.0);
        page.add_font(&font, "Helvetica");
        page.begin_text();
        page.set_font(&font, 12.0);
        page.move_to(72.0, 720.0);
        page.show_text(&format!(
            "Page {} content with some text for benchmarking.",
            i + 1
        ));
        page.end_text();
        doc.add_page(page);
    }
    doc.build().unwrap()
}

fn bench_parse(c: &mut Criterion) {
    let small_pdf = create_test_pdf(1);
    let medium_pdf = create_test_pdf(10);
    let large_pdf = create_test_pdf(100);

    c.bench_function("parse_1page", |b| {
        b.iter(|| PdfDocument::from_bytes(small_pdf.clone()).unwrap())
    });
    c.bench_function("parse_10pages", |b| {
        b.iter(|| PdfDocument::from_bytes(medium_pdf.clone()).unwrap())
    });
    c.bench_function("parse_100pages", |b| {
        b.iter(|| PdfDocument::from_bytes(large_pdf.clone()).unwrap())
    });
}

fn bench_resolve_objects(c: &mut Criterion) {
    let pdf = create_test_pdf(10);
    c.bench_function("resolve_all_objects_10pages", |b| {
        b.iter(|| {
            let mut doc = PdfDocument::from_bytes(pdf.clone()).unwrap();
            let refs: Vec<_> = doc.object_refs().collect();
            for r in &refs {
                let _ = doc.resolve(r);
            }
        })
    });
}

criterion_group!(benches, bench_parse, bench_resolve_objects);
criterion_main!(benches);
