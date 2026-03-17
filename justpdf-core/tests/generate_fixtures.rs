/// Run with: cargo test -p justpdf-core --test generate_fixtures -- --ignored
/// Generates test PDF fixtures in tests/fixtures/
use std::io::Write;
use std::path::Path;

fn fixtures_dir() -> &'static Path {
    Path::new("tests/fixtures")
}

#[test]
#[ignore] // Only run manually to regenerate fixtures
fn generate_all_fixtures() {
    let dir = fixtures_dir();
    std::fs::create_dir_all(dir).unwrap();

    write_fixture(dir, "minimal.pdf", &build_minimal());
    write_fixture(dir, "two_pages.pdf", &build_two_pages());
    write_fixture(dir, "with_text.pdf", &build_with_text());
    write_fixture(dir, "compressed_stream.pdf", &build_compressed_stream());
    write_fixture(dir, "ascii_hex_stream.pdf", &build_ascii_hex_stream());
    write_fixture(dir, "incremental.pdf", &build_incremental());
    write_fixture(dir, "not_a_pdf.txt", b"This is not a PDF file.\n");
    write_fixture(dir, "empty.bin", b"");
    write_fixture(dir, "truncated.pdf", &build_truncated());
    write_fixture(dir, "corrupted_xref.pdf", &build_corrupted_xref());

    println!("Generated all fixtures in {}", dir.display());
}

fn write_fixture(dir: &Path, name: &str, data: &[u8]) {
    let path = dir.join(name);
    std::fs::write(&path, data).unwrap();
    println!("  {} ({} bytes)", name, data.len());
}

/// Smallest valid PDF: 1 blank page.
fn build_minimal() -> Vec<u8> {
    let mut pdf = Vec::new();
    pdf.extend_from_slice(b"%PDF-1.4\n");

    let obj1 = pdf.len();
    pdf.extend_from_slice(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n");

    let obj2 = pdf.len();
    pdf.extend_from_slice(b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n");

    let obj3 = pdf.len();
    pdf.extend_from_slice(
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>\nendobj\n",
    );

    write_xref_and_trailer(&mut pdf, &[(0, true), (obj1, false), (obj2, false), (obj3, false)], 1);
    pdf
}

/// 2 pages, different sizes.
fn build_two_pages() -> Vec<u8> {
    let mut pdf = Vec::new();
    pdf.extend_from_slice(b"%PDF-1.4\n");

    let obj1 = pdf.len();
    pdf.extend_from_slice(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n");

    let obj2 = pdf.len();
    pdf.extend_from_slice(
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 >>\nendobj\n",
    );

    let obj3 = pdf.len();
    pdf.extend_from_slice(
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>\nendobj\n",
    );

    let obj4 = pdf.len();
    pdf.extend_from_slice(
        b"4 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 842 595] >>\nendobj\n",
    );

    write_xref_and_trailer(
        &mut pdf,
        &[(0, true), (obj1, false), (obj2, false), (obj3, false), (obj4, false)],
        1,
    );
    pdf
}

/// Page with text content stream.
fn build_with_text() -> Vec<u8> {
    let mut pdf = Vec::new();
    pdf.extend_from_slice(b"%PDF-1.4\n");

    let obj1 = pdf.len();
    pdf.extend_from_slice(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n");

    let obj2 = pdf.len();
    pdf.extend_from_slice(b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n");

    let content = b"BT /F1 24 Tf 72 720 Td (Hello World) Tj ET";
    let obj4 = pdf.len();
    pdf.extend_from_slice(format!("4 0 obj\n<< /Length {} >>\nstream\n", content.len()).as_bytes());
    pdf.extend_from_slice(content);
    pdf.extend_from_slice(b"\nendstream\nendobj\n");

    let obj5 = pdf.len();
    pdf.extend_from_slice(b"5 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n");

    let obj3 = pdf.len();
    pdf.extend_from_slice(
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>\nendobj\n",
    );

    write_xref_and_trailer(
        &mut pdf,
        &[
            (0, true),
            (obj1, false),
            (obj2, false),
            (obj3, false),
            (obj4, false),
            (obj5, false),
        ],
        1,
    );
    pdf
}

/// Page with FlateDecode compressed content stream.
fn build_compressed_stream() -> Vec<u8> {
    use flate2::write::ZlibEncoder;
    use flate2::Compression;

    let mut pdf = Vec::new();
    pdf.extend_from_slice(b"%PDF-1.4\n");

    let obj1 = pdf.len();
    pdf.extend_from_slice(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n");

    let obj2 = pdf.len();
    pdf.extend_from_slice(b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n");

    let content = b"BT /F1 18 Tf 72 700 Td (Compressed content stream) Tj ET";
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(content).unwrap();
    let compressed = encoder.finish().unwrap();

    let obj4 = pdf.len();
    pdf.extend_from_slice(
        format!(
            "4 0 obj\n<< /Length {} /Filter /FlateDecode >>\nstream\n",
            compressed.len()
        )
        .as_bytes(),
    );
    pdf.extend_from_slice(&compressed);
    pdf.extend_from_slice(b"\nendstream\nendobj\n");

    let obj5 = pdf.len();
    pdf.extend_from_slice(b"5 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n");

    let obj3 = pdf.len();
    pdf.extend_from_slice(
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>\nendobj\n",
    );

    write_xref_and_trailer(
        &mut pdf,
        &[
            (0, true),
            (obj1, false),
            (obj2, false),
            (obj3, false),
            (obj4, false),
            (obj5, false),
        ],
        1,
    );
    pdf
}

/// Stream with ASCIIHexDecode filter.
fn build_ascii_hex_stream() -> Vec<u8> {
    let mut pdf = Vec::new();
    pdf.extend_from_slice(b"%PDF-1.4\n");

    let obj1 = pdf.len();
    pdf.extend_from_slice(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n");

    let obj2 = pdf.len();
    pdf.extend_from_slice(b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n");

    let content = b"BT /F1 12 Tf 72 700 Td (ASCIIHex encoded) Tj ET";
    let mut hex_encoded = Vec::new();
    for &b in content {
        hex_encoded.extend_from_slice(format!("{b:02X}").as_bytes());
    }
    hex_encoded.push(b'>');

    let obj4 = pdf.len();
    pdf.extend_from_slice(
        format!(
            "4 0 obj\n<< /Length {} /Filter /ASCIIHexDecode >>\nstream\n",
            hex_encoded.len()
        )
        .as_bytes(),
    );
    pdf.extend_from_slice(&hex_encoded);
    pdf.extend_from_slice(b"\nendstream\nendobj\n");

    let obj5 = pdf.len();
    pdf.extend_from_slice(b"5 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Courier >>\nendobj\n");

    let obj3 = pdf.len();
    pdf.extend_from_slice(
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>\nendobj\n",
    );

    write_xref_and_trailer(
        &mut pdf,
        &[
            (0, true),
            (obj1, false),
            (obj2, false),
            (obj3, false),
            (obj4, false),
            (obj5, false),
        ],
        1,
    );
    pdf
}

/// PDF with incremental update (two xref sections).
fn build_incremental() -> Vec<u8> {
    let mut pdf = Vec::new();
    pdf.extend_from_slice(b"%PDF-1.4\n");

    // Original document
    let obj1 = pdf.len();
    pdf.extend_from_slice(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n");

    let obj2 = pdf.len();
    pdf.extend_from_slice(b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n");

    let obj3 = pdf.len();
    pdf.extend_from_slice(
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>\nendobj\n",
    );

    // First xref + trailer
    let xref1_offset = pdf.len();
    pdf.extend_from_slice(b"xref\n0 4\n");
    pdf.extend_from_slice(b"0000000000 65535 f \n");
    pdf.extend_from_slice(format!("{:010} 00000 n \n", obj1).as_bytes());
    pdf.extend_from_slice(format!("{:010} 00000 n \n", obj2).as_bytes());
    pdf.extend_from_slice(format!("{:010} 00000 n \n", obj3).as_bytes());
    pdf.extend_from_slice(b"trailer\n<< /Size 4 /Root 1 0 R >>\n");
    pdf.extend_from_slice(format!("startxref\n{xref1_offset}\n%%EOF\n").as_bytes());

    // Incremental update: add Info dict (obj 4)
    let obj4 = pdf.len();
    pdf.extend_from_slice(
        b"4 0 obj\n<< /Title (Test Document) /Author (justpdf) >>\nendobj\n",
    );

    // Second xref + trailer with /Prev
    let xref2_offset = pdf.len();
    pdf.extend_from_slice(b"xref\n4 1\n");
    pdf.extend_from_slice(format!("{:010} 00000 n \n", obj4).as_bytes());
    pdf.extend_from_slice(
        format!(
            "trailer\n<< /Size 5 /Root 1 0 R /Info 4 0 R /Prev {xref1_offset} >>\n"
        )
        .as_bytes(),
    );
    pdf.extend_from_slice(format!("startxref\n{xref2_offset}\n%%EOF\n").as_bytes());

    pdf
}

/// Truncated PDF (cut off mid-object).
fn build_truncated() -> Vec<u8> {
    let mut pdf = Vec::new();
    pdf.extend_from_slice(b"%PDF-1.4\n");
    pdf.extend_from_slice(b"1 0 obj\n<< /Type /Catalog /Pa");
    // Cut off here — no endobj, no xref, no trailer
    pdf
}

/// PDF with corrupted xref (bad offsets).
fn build_corrupted_xref() -> Vec<u8> {
    let mut pdf = Vec::new();
    pdf.extend_from_slice(b"%PDF-1.4\n");

    let obj1 = pdf.len();
    pdf.extend_from_slice(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n");

    let obj2 = pdf.len();
    pdf.extend_from_slice(b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n");

    let obj3 = pdf.len();
    pdf.extend_from_slice(
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>\nendobj\n",
    );

    // Xref with deliberately wrong offsets
    let xref_offset = pdf.len();
    pdf.extend_from_slice(b"xref\n0 4\n");
    pdf.extend_from_slice(b"0000000000 65535 f \n");
    pdf.extend_from_slice(b"0000099999 00000 n \n"); // Wrong offset!
    pdf.extend_from_slice(format!("{:010} 00000 n \n", obj2).as_bytes());
    pdf.extend_from_slice(format!("{:010} 00000 n \n", obj3).as_bytes());
    pdf.extend_from_slice(b"trailer\n<< /Size 4 /Root 1 0 R >>\n");
    pdf.extend_from_slice(format!("startxref\n{xref_offset}\n%%EOF\n").as_bytes());

    pdf
}

/// Helper: write xref table + trailer + startxref + %%EOF.
/// entries: [(offset, is_free)], root_obj is the /Root object number.
fn write_xref_and_trailer(pdf: &mut Vec<u8>, entries: &[(usize, bool)], root_obj: u32) {
    let xref_offset = pdf.len();
    pdf.extend_from_slice(format!("xref\n0 {}\n", entries.len()).as_bytes());

    for (i, &(offset, is_free)) in entries.iter().enumerate() {
        if is_free {
            let gen_num = if i == 0 { 65535 } else { 0 };
            pdf.extend_from_slice(format!("{:010} {:05} f \n", offset, gen_num).as_bytes());
        } else {
            pdf.extend_from_slice(format!("{:010} 00000 n \n", offset).as_bytes());
        }
    }

    pdf.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root {} 0 R >>\n",
            entries.len(),
            root_obj
        )
        .as_bytes(),
    );
    pdf.extend_from_slice(format!("startxref\n{xref_offset}\n%%EOF\n").as_bytes());
}
