use criterion::{criterion_group, criterion_main, Criterion};
use justpdf_core::PdfDocument;
use justpdf_render::render::{render_page, RenderOptions};
use std::path::Path;

fn bench_render_page(c: &mut Criterion) {
    // Try to load test PDFs from fixtures
    let fixture_paths = [
        "justpdf-core/tests/fixtures/minimal.pdf",
        "justpdf-core/tests/fixtures/with_text.pdf",
    ];

    for fixture in &fixture_paths {
        let path = Path::new(fixture);
        if !path.exists() {
            continue;
        }

        let data = std::fs::read(path).unwrap();
        let doc = PdfDocument::from_bytes(data).unwrap();
        let opts = RenderOptions::default();
        let name = path.file_stem().unwrap().to_string_lossy();

        c.bench_function(&format!("render_page/{name}"), |b| {
            b.iter(|| {
                render_page(&doc, 0, &opts).unwrap();
            })
        });
    }
}

fn bench_render_dpi(c: &mut Criterion) {
    let path = Path::new("justpdf-core/tests/fixtures/minimal.pdf");
    if !path.exists() {
        return;
    }

    let data = std::fs::read(path).unwrap();
    let doc = PdfDocument::from_bytes(data).unwrap();

    for dpi in [72.0, 150.0, 300.0] {
        let opts = RenderOptions {
            dpi,
            ..Default::default()
        };
        c.bench_function(&format!("render_page/minimal_{dpi}dpi"), |b| {
            b.iter(|| {
                render_page(&doc, 0, &opts).unwrap();
            })
        });
    }
}

criterion_group!(benches, bench_render_page, bench_render_dpi);
criterion_main!(benches);
