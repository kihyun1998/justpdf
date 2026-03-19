use criterion::{criterion_group, criterion_main, Criterion};
use justpdf_core::parser::PdfDocument;
use justpdf_core::page::collect_pages;
use justpdf_core::text::extract_all_text;
use std::path::Path;

fn bench_corpus_parse(c: &mut Criterion) {
    let fixtures_dir = Path::new("justpdf-core/tests/fixtures");
    if !fixtures_dir.exists() {
        return;
    }

    let pdfs: Vec<_> = std::fs::read_dir(fixtures_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "pdf"))
        .collect();

    for entry in &pdfs {
        let path = entry.path();
        let name = path.file_stem().unwrap().to_string_lossy().to_string();

        c.bench_function(&format!("corpus_parse/{name}"), |b| {
            let data = std::fs::read(&path).unwrap();
            b.iter(|| {
                let _doc = PdfDocument::from_bytes(data.clone()).unwrap();
            })
        });
    }
}

fn bench_corpus_text(c: &mut Criterion) {
    let fixtures_dir = Path::new("justpdf-core/tests/fixtures");
    if !fixtures_dir.exists() {
        return;
    }

    let pdfs: Vec<_> = std::fs::read_dir(fixtures_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "pdf"))
        .collect();

    for entry in &pdfs {
        let path = entry.path();
        let name = path.file_stem().unwrap().to_string_lossy().to_string();

        // Skip PDFs that fail to parse
        let data = std::fs::read(&path).unwrap();
        if PdfDocument::from_bytes(data.clone()).is_err() {
            continue;
        }

        c.bench_function(&format!("corpus_text/{name}"), |b| {
            b.iter(|| {
                let doc = PdfDocument::from_bytes(data.clone()).unwrap();
                let _ = extract_all_text(&doc);
            })
        });
    }
}

fn bench_corpus_pages(c: &mut Criterion) {
    let fixtures_dir = Path::new("justpdf-core/tests/fixtures");
    if !fixtures_dir.exists() {
        return;
    }

    let pdfs: Vec<_> = std::fs::read_dir(fixtures_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "pdf"))
        .collect();

    for entry in &pdfs {
        let path = entry.path();
        let name = path.file_stem().unwrap().to_string_lossy().to_string();

        let data = std::fs::read(&path).unwrap();
        if PdfDocument::from_bytes(data.clone()).is_err() {
            continue;
        }

        c.bench_function(&format!("corpus_pages/{name}"), |b| {
            b.iter(|| {
                let doc = PdfDocument::from_bytes(data.clone()).unwrap();
                let _ = collect_pages(&doc);
            })
        });
    }
}

criterion_group!(benches, bench_corpus_parse, bench_corpus_text, bench_corpus_pages);
criterion_main!(benches);
