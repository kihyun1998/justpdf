//! Tree walkers against cyclic and shared-node trees (#52).

use justpdf_core::embedded_file::read_embedded_files;
use justpdf_core::form::parse_acroform;
use justpdf_core::outline::{read_named_destinations, read_outlines};
use justpdf_core::page::{collect_pages, get_page};
use justpdf_core::page_label::read_page_labels;
use justpdf_core::sign::detect_signatures;
use justpdf_core::{JustPdfError, PdfDocument};

/// Builds a PDF whose object `n` (1-based) is `objects[n - 1]`, with a
/// correct xref table and `/Root 1 0 R`.
fn pdf(objects: &[&str]) -> PdfDocument {
    let mut out = b"%PDF-1.7\n".to_vec();
    let mut offsets = Vec::new();
    for (i, body) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n{body}\nendobj\n", i + 1).as_bytes());
    }
    let xref_at = out.len();
    out.extend_from_slice(
        format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes(),
    );
    for off in offsets {
        out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref_at}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );
    PdfDocument::from_bytes(out).unwrap()
}

fn assert_cycle<T: std::fmt::Debug>(result: justpdf_core::Result<T>, obj_num: u32) {
    match result {
        Err(JustPdfError::CircularReference {
            obj_num: n,
            gen_num: 0,
        }) => assert_eq!(n, obj_num),
        other => panic!("expected CircularReference at {obj_num}, got {other:?}"),
    }
}

// ---- page tree: a cycle is an error --------------------------------------

/// Pages node 3 lists its own parent 2 as a kid.
fn cyclic_page_tree() -> PdfDocument {
    pdf(&[
        "<< /Type /Catalog /Pages 2 0 R >>",
        "<< /Type /Pages /Kids [3 0 R] /Count 5 >>",
        "<< /Type /Pages /Parent 2 0 R /Kids [4 0 R 2 0 R] /Count 5 >>",
        "<< /Type /Page /Parent 3 0 R /MediaBox [0 0 100 100] >>",
    ])
}

#[test]
fn collect_pages_rejects_page_tree_cycle() {
    assert_cycle(collect_pages(&cyclic_page_tree()), 2);
}

#[test]
fn get_page_rejects_page_tree_cycle() {
    assert_cycle(get_page(&cyclic_page_tree(), 3), 2);
}

#[test]
fn page_tree_shared_kid_is_not_a_cycle() {
    // Intermediate node 3 is listed twice under the same parent.
    let doc = pdf(&[
        "<< /Type /Catalog /Pages 2 0 R >>",
        "<< /Type /Pages /Kids [3 0 R 3 0 R] /Count 2 >>",
        "<< /Type /Pages /Parent 2 0 R /Kids [4 0 R] /Count 1 >>",
        "<< /Type /Page /Parent 3 0 R /MediaBox [0 0 100 100] >>",
    ]);
    assert_eq!(collect_pages(&doc).unwrap().len(), 2);
    assert_eq!(get_page(&doc, 1).unwrap().page_ref.obj_num, 4);
}

// ---- field trees: a cycle is an error ------------------------------------

/// Field 4 lists its parent 3 as a kid.
fn cyclic_field_tree() -> PdfDocument {
    pdf(&[
        "<< /Type /Catalog /Pages 2 0 R /AcroForm << /Fields [3 0 R] >> >>",
        "<< /Type /Pages /Kids [] /Count 0 >>",
        "<< /T (a) /FT /Sig /Kids [4 0 R] >>",
        "<< /T (b) /Parent 3 0 R /Kids [3 0 R] >>",
    ])
}

#[test]
fn parse_acroform_rejects_field_tree_cycle() {
    assert_cycle(parse_acroform(&cyclic_field_tree()), 3);
}

#[test]
fn detect_signatures_rejects_field_tree_cycle() {
    assert_cycle(detect_signatures(&cyclic_field_tree()), 3);
}

#[test]
fn field_tree_shared_kid_is_not_a_cycle() {
    let doc = pdf(&[
        "<< /Type /Catalog /Pages 2 0 R /AcroForm << /Fields [3 0 R] >> >>",
        "<< /Type /Pages /Kids [] /Count 0 >>",
        "<< /T (a) /FT /Tx /Kids [4 0 R 4 0 R] >>",
        "<< /T (b) /Parent 3 0 R /Kids [5 0 R] >>",
        "<< /T (c) /Parent 4 0 R >>",
    ]);
    assert_eq!(parse_acroform(&doc).unwrap().unwrap().fields.len(), 2);
    assert!(detect_signatures(&doc).unwrap().is_empty());
}

// ---- outlines: a cycle is an error ---------------------------------------

#[test]
fn read_outlines_rejects_child_pointing_at_ancestor() {
    // Item 5 (child of 4) has item 4 as its own first child.
    let doc = pdf(&[
        "<< /Type /Catalog /Pages 2 0 R /Outlines 3 0 R >>",
        "<< /Type /Pages /Kids [] /Count 0 >>",
        "<< /Type /Outlines /First 4 0 R >>",
        "<< /Title (a) /Parent 3 0 R /First 5 0 R >>",
        "<< /Title (b) /Parent 4 0 R /First 4 0 R >>",
    ]);
    assert_cycle(read_outlines(&doc), 4);
}

#[test]
fn read_outlines_rejects_next_pointing_at_ancestor() {
    // Item 5 (child of 4) names item 4 as its next sibling.
    let doc = pdf(&[
        "<< /Type /Catalog /Pages 2 0 R /Outlines 3 0 R >>",
        "<< /Type /Pages /Kids [] /Count 0 >>",
        "<< /Type /Outlines /First 4 0 R >>",
        "<< /Title (a) /Parent 3 0 R /First 5 0 R >>",
        "<< /Title (b) /Parent 4 0 R /Next 4 0 R >>",
    ]);
    assert_cycle(read_outlines(&doc), 4);
}

#[test]
fn read_outlines_sibling_loop_still_stops_quietly() {
    // Items 4 and 5 name each other as next; no ancestor is involved.
    let doc = pdf(&[
        "<< /Type /Catalog /Pages 2 0 R /Outlines 3 0 R >>",
        "<< /Type /Pages /Kids [] /Count 0 >>",
        "<< /Type /Outlines /First 4 0 R >>",
        "<< /Title (a) /Parent 3 0 R /Next 5 0 R >>",
        "<< /Title (b) /Parent 3 0 R /Next 4 0 R >>",
    ]);
    let titles: Vec<_> = read_outlines(&doc)
        .unwrap()
        .into_iter()
        .map(|i| i.title)
        .collect();
    assert_eq!(titles, ["a", "b"]);
}

// ---- number and name trees: a cyclic branch is skipped -------------------

#[test]
fn read_page_labels_skips_cyclic_kid() {
    // Node 4 lists the root 3 as a kid, next to leaf 5. The root has an
    // entry of its own.
    let doc = pdf(&[
        "<< /Type /Catalog /Pages 2 0 R /PageLabels 3 0 R >>",
        "<< /Type /Pages /Kids [] /Count 0 >>",
        "<< /Nums [0 << /S /r >>] /Kids [4 0 R] >>",
        "<< /Kids [3 0 R 5 0 R] >>",
        "<< /Nums [2 << /S /D >>] >>",
    ]);
    let starts: Vec<_> = read_page_labels(&doc)
        .unwrap()
        .iter()
        .map(|r| r.start_page)
        .collect();
    assert_eq!(starts, [0, 2]);
}

#[test]
fn read_page_labels_keeps_shared_kid() {
    let doc = pdf(&[
        "<< /Type /Catalog /Pages 2 0 R /PageLabels 3 0 R >>",
        "<< /Type /Pages /Kids [] /Count 0 >>",
        "<< /Kids [4 0 R 4 0 R] >>",
        "<< /Nums [0 << /S /D >>] >>",
    ]);
    assert_eq!(read_page_labels(&doc).unwrap().len(), 2);
}

#[test]
fn read_embedded_files_skips_cyclic_kid() {
    let doc = pdf(&[
        "<< /Type /Catalog /Pages 2 0 R /Names << /EmbeddedFiles 3 0 R >> >>",
        "<< /Type /Pages /Kids [] /Count 0 >>",
        "<< /Names [(a.txt) << /Type /Filespec /F (a.txt) >>] /Kids [4 0 R] >>",
        "<< /Kids [3 0 R 5 0 R] >>",
        "<< /Names [(b.txt) << /Type /Filespec /F (b.txt) >>] >>",
    ]);
    let names: Vec<_> = read_embedded_files(&doc)
        .unwrap()
        .into_iter()
        .map(|f| f.filename)
        .collect();
    assert_eq!(names, ["a.txt", "b.txt"]);
}

#[test]
fn read_embedded_files_keeps_shared_kid() {
    let doc = pdf(&[
        "<< /Type /Catalog /Pages 2 0 R /Names << /EmbeddedFiles 3 0 R >> >>",
        "<< /Type /Pages /Kids [] /Count 0 >>",
        "<< /Kids [4 0 R 4 0 R] >>",
        "<< /Names [(a.txt) << /Type /Filespec /F (a.txt) >>] >>",
    ]);
    assert_eq!(read_embedded_files(&doc).unwrap().len(), 2);
}

#[test]
fn read_named_destinations_keeps_shared_kid() {
    let doc = pdf(&[
        "<< /Type /Catalog /Pages 2 0 R /Names 6 0 R >>",
        "<< /Type /Pages /Kids [] /Count 0 >>",
        "<< /Kids [4 0 R 4 0 R] >>",
        "<< /Names [(leaf) [5 0 R /Fit]] >>",
        "<< /Type /Page >>",
        "<< /Dests 3 0 R >>",
    ]);
    assert_eq!(read_named_destinations(&doc).unwrap().len(), 2);
}

#[test]
fn read_named_destinations_skips_cyclic_kid() {
    let doc = pdf(&[
        "<< /Type /Catalog /Pages 2 0 R /Names 6 0 R >>",
        "<< /Type /Pages /Kids [] /Count 0 >>",
        "<< /Names [(root) [7 0 R /Fit]] /Kids [4 0 R] >>",
        "<< /Kids [3 0 R 5 0 R] >>",
        "<< /Names [(leaf) [7 0 R /Fit]] >>",
        "<< /Dests 3 0 R >>",
        "<< /Type /Page >>",
    ]);
    let names: Vec<_> = read_named_destinations(&doc)
        .unwrap()
        .into_iter()
        .map(|(n, _)| n)
        .collect();
    assert_eq!(names, ["root", "leaf"]);
}

#[test]
fn read_named_destinations_skips_cyclic_kid_on_every_path() {
    // Catalog /Names and its /Dests, each inline or indirect. Node 4 lists
    // node 3 (the root when /Dests is indirect) as a kid next to leaf 5.
    let cases: [(&str, &str, &[&str]); 3] = [
        (
            "inline /Names, indirect /Dests",
            "<< /Dests 3 0 R >>",
            &["root", "leaf"],
        ),
        (
            "inline /Names, inline /Dests",
            "<< /Dests << /Names [(top) [7 0 R /Fit]] /Kids [3 0 R] >> >>",
            &["top", "root", "leaf"],
        ),
        (
            "indirect /Names, inline /Dests",
            "6 0 R",
            &["top", "root", "leaf"],
        ),
    ];
    for (layout, names, expected) in cases {
        let catalog = format!("<< /Type /Catalog /Pages 2 0 R /Names {names} >>");
        let doc = pdf(&[
            &catalog,
            "<< /Type /Pages /Kids [] /Count 0 >>",
            "<< /Names [(root) [7 0 R /Fit]] /Kids [4 0 R] >>",
            "<< /Kids [3 0 R 5 0 R] >>",
            "<< /Names [(leaf) [7 0 R /Fit]] >>",
            "<< /Dests << /Names [(top) [7 0 R /Fit]] /Kids [3 0 R] >> >>",
            "<< /Type /Page >>",
        ]);
        let names: Vec<_> = read_named_destinations(&doc)
            .unwrap()
            .into_iter()
            .map(|(n, _)| n)
            .collect();
        assert_eq!(names, expected, "{layout}");
    }
}
