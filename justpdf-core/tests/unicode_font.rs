//! Drawing Unicode text through caller-supplied TrueType fonts (`FontProgram`,
//! `Font`) on `DocumentBuilder` and `DocumentModifier` pages.
//!
//! Expected metrics are the fixtures' table values read with fontTools.

use std::collections::BTreeMap;

use justpdf_core::font::{FontEmbedding, FontProgram};
use justpdf_core::page::{PageInfo, collect_pages};
use justpdf_core::text::extract_page_text_string;
use justpdf_core::writer::{DocumentBuilder, DocumentModifier, PageBuilder, incremental_save};
use justpdf_core::{JustPdfError, PdfDict, PdfDocument, PdfObject, ttf_parser};

/// 18 Hangul syllables and nothing else.
const HANGUL: &[u8] = include_bytes!("fixtures/JustpdfTestHangul-Regular.ttf");
/// The same syllables plus printable ASCII, at 2048 units per em.
const HANGUL_LATIN: &[u8] = include_bytes!("fixtures/JustpdfTestHangulLatin-Regular.ttf");
/// Face 0: `HANGUL`; face 1: `HANGUL_LATIN`.
const HANGUL_TTC: &[u8] = include_bytes!("fixtures/JustpdfTestHangul.ttc");
const NOTO_SANS: &[u8] = include_bytes!("fixtures/NotoSans-Regular.ttf");

fn program(data: &[u8]) -> FontProgram {
    FontProgram::parse(data.to_vec(), 0).unwrap()
}

/// A one-page document drawing `text` at 24 pt in the font `font` makes.
fn one_page(doc: &mut DocumentBuilder, font: &justpdf_core::font::Font, text: &str) {
    let mut page = PageBuilder::new(612.0, 792.0);
    page.begin_text();
    page.set_font(font, 24.0);
    page.move_to(72.0, 700.0);
    page.show_text(text);
    page.end_text();
    doc.add_page(page);
}

fn open(pdf: Vec<u8>) -> (PdfDocument, Vec<PageInfo>) {
    let doc = PdfDocument::from_bytes(pdf).unwrap();
    let pages = collect_pages(&doc).unwrap();
    (doc, pages)
}

fn resolve(doc: &PdfDocument, obj: &PdfObject) -> PdfObject {
    match obj {
        PdfObject::Reference(r) => doc.resolve(r).unwrap(),
        other => other.clone(),
    }
}

fn dict(doc: &PdfDocument, obj: &PdfObject) -> PdfDict {
    match resolve(doc, obj) {
        PdfObject::Dict(d) => d,
        other => panic!("expected a dictionary, got {other:?}"),
    }
}

fn stream_data(doc: &PdfDocument, obj: &PdfObject) -> Vec<u8> {
    match resolve(doc, obj) {
        PdfObject::Stream { dict, data } => doc.decode_stream(&dict, &data).unwrap(),
        other => panic!("expected a stream, got {other:?}"),
    }
}

/// The page's `/Resources /Font` entries, resolved.
fn page_fonts(doc: &PdfDocument, page: &PageInfo) -> BTreeMap<String, PdfDict> {
    let resources = dict(doc, page.resources_ref.as_ref().unwrap());
    let fonts = resources.get_dict(b"Font").unwrap();
    fonts
        .iter()
        .map(|(name, obj)| (String::from_utf8(name.clone()).unwrap(), dict(doc, obj)))
        .collect()
}

/// The page's only Type0 font: (Type0 dictionary, its CIDFont dictionary).
fn only_type0(doc: &PdfDocument, page: &PageInfo) -> (PdfDict, PdfDict) {
    let fonts: Vec<PdfDict> = page_fonts(doc, page)
        .into_values()
        .filter(|f| f.get_name(b"Subtype") == Some(b"Type0"))
        .collect();
    assert_eq!(fonts.len(), 1, "expected one Type0 font");
    let type0 = fonts.into_iter().next().unwrap();
    let cid_font = dict(doc, &type0.get_array(b"DescendantFonts").unwrap()[0]);
    (type0, cid_font)
}

fn content(doc: &PdfDocument, page: &PageInfo) -> String {
    let bytes = stream_data(doc, page.contents_ref.as_ref().unwrap());
    String::from_utf8_lossy(&bytes).into_owned()
}

fn font_file(doc: &PdfDocument, cid_font: &PdfDict) -> Vec<u8> {
    let descriptor = dict(doc, cid_font.get(b"FontDescriptor").unwrap());
    stream_data(doc, descriptor.get(b"FontFile2").unwrap())
}

fn int(obj: &PdfObject) -> i64 {
    match obj {
        PdfObject::Integer(i) => *i,
        PdfObject::Real(r) if r.fract() == 0.0 => *r as i64,
        other => panic!("expected an integer, got {other:?}"),
    }
}

/// CID → width from a `/W` array of `c [w1 w2 …]` and `c1 c2 w` entries.
fn widths(w: &[PdfObject]) -> BTreeMap<i64, i64> {
    let mut out = BTreeMap::new();
    let mut i = 0;
    while i < w.len() {
        let first = int(&w[i]);
        match &w[i + 1] {
            PdfObject::Array(list) => {
                for (k, width) in list.iter().enumerate() {
                    out.insert(first + k as i64, int(width));
                }
                i += 2;
            }
            last => {
                for cid in first..=int(last) {
                    out.insert(cid, int(&w[i + 2]));
                }
                i += 3;
            }
        }
    }
    out
}

/// The resource names selected with `Tf`, in order.
fn tf_names(content: &str) -> Vec<String> {
    content
        .lines()
        .filter(|l| l.ends_with(" Tf"))
        .map(|l| l.split_whitespace().next().unwrap().to_string())
        .collect()
}

#[test]
fn hangul_with_latin_fallback_extracts_exactly_and_reports_the_emoji() {
    let mut doc = DocumentBuilder::new();
    let hangul = doc.add_font(&program(HANGUL));
    let noto = doc.add_font(&program(NOTO_SANS));
    one_page(&mut doc, &hangul.with_fallback(&noto), "café 한글 😀");

    assert_eq!(
        doc.missing_chars().into_iter().collect::<Vec<_>>(),
        vec!['😀']
    );
    let (pdf, pages) = open(doc.build().unwrap());
    assert_eq!(
        extract_page_text_string(&pdf, &pages[0]).unwrap().trim(),
        "café 한글 😀"
    );
}

#[test]
fn two_different_missing_characters_extract_back_distinctly() {
    let mut doc = DocumentBuilder::new();
    let hangul = doc.add_font(&program(HANGUL));
    one_page(&mut doc, &hangul, "😀한🙂");

    assert_eq!(
        doc.missing_chars().into_iter().collect::<Vec<_>>(),
        vec!['😀', '🙂']
    );
    let (pdf, pages) = open(doc.build().unwrap());
    assert_eq!(
        extract_page_text_string(&pdf, &pages[0]).unwrap().trim(),
        "😀한🙂"
    );

    // CIDs 1..=3 in order of first use; both missing characters map to GID 0.
    let (_, cid_font) = only_type0(&pdf, &pages[0]);
    let map = stream_data(&pdf, cid_font.get(b"CIDToGIDMap").unwrap());
    let gids: Vec<u16> = map
        .chunks(2)
        .map(|c| u16::from_be_bytes([c[0], c[1]]))
        .collect();
    let han = ttf_parser::Face::parse(HANGUL, 0)
        .unwrap()
        .glyph_index('한')
        .unwrap()
        .0;
    assert_eq!(gids, vec![0, 0, han, 0]);
    assert!(content(&pdf, &pages[0]).contains("<000100020003> Tj"));
}

#[test]
fn fallback_switches_fonts_only_where_the_primary_lacks_a_glyph() {
    let mut doc = DocumentBuilder::new();
    let hangul = doc.add_font(&program(HANGUL));
    let noto = doc.add_font(&program(NOTO_SANS));
    one_page(&mut doc, &hangul.with_fallback(&noto), "한글 abc");
    let (pdf, pages) = open(doc.build().unwrap());
    let fonts = page_fonts(&pdf, &pages[0]);
    let names = tf_names(&content(&pdf, &pages[0]));
    // Selected, switched to the fallback for " abc", restored.
    assert_eq!(names.len(), 3, "{names:?}");
    assert_ne!(names[0], names[1]);
    assert_eq!(names[0], names[2]);
    assert_eq!(fonts.len(), 2);

    let mut doc = DocumentBuilder::new();
    let both = doc.add_font(&program(HANGUL_LATIN));
    let noto = doc.add_font(&program(NOTO_SANS));
    one_page(&mut doc, &both.with_fallback(&noto), "한글 abc");
    let (pdf, pages) = open(doc.build().unwrap());
    assert_eq!(tf_names(&content(&pdf, &pages[0])).len(), 1);
    assert_eq!(page_fonts(&pdf, &pages[0]).len(), 1);
    assert_eq!(
        extract_page_text_string(&pdf, &pages[0]).unwrap().trim(),
        "한글 abc"
    );
}

#[test]
fn the_embedded_program_is_a_tagged_subset_of_the_used_glyphs() {
    let mut doc = DocumentBuilder::new();
    let noto = doc.add_font(&program(NOTO_SANS));
    one_page(&mut doc, &noto, "Hello");
    let (pdf, pages) = open(doc.build().unwrap());
    let (type0, cid_font) = only_type0(&pdf, &pages[0]);

    let base_font = String::from_utf8(type0.get_name(b"BaseFont").unwrap().to_vec()).unwrap();
    let (tag, name) = base_font.split_once('+').unwrap();
    assert_eq!(tag.len(), 6);
    assert!(tag.bytes().all(|b| b.is_ascii_uppercase()), "{tag}");
    assert_eq!(name, "NotoSans-Regular");
    assert_eq!(cid_font.get_name(b"BaseFont"), Some(base_font.as_bytes()));
    let descriptor = dict(&pdf, cid_font.get(b"FontDescriptor").unwrap());
    assert_eq!(descriptor.get_name(b"FontName"), Some(base_font.as_bytes()));

    let subset = font_file(&pdf, &cid_font);
    assert!(
        subset.len() < 45_000,
        "subset is {} bytes of {}",
        subset.len(),
        NOTO_SANS.len()
    );
    let source = ttf_parser::Face::parse(NOTO_SANS, 0).unwrap();
    let face = ttf_parser::Face::parse(&subset, 0).unwrap();
    for ch in "Helo".chars() {
        let gid = source.glyph_index(ch).unwrap();
        assert!(
            face.glyph_bounding_box(gid).is_some(),
            "{ch} lost its outline"
        );
    }
    let unused = source.glyph_index('Z').unwrap();
    assert!(face.glyph_bounding_box(unused).is_none());
}

#[test]
fn the_whole_program_option_embeds_the_original_bytes_untagged() {
    let mut doc = DocumentBuilder::new();
    let font = doc.add_font_with(&program(HANGUL_LATIN), FontEmbedding::Whole);
    one_page(&mut doc, &font, "한 A");
    let (pdf, pages) = open(doc.build().unwrap());
    let (type0, cid_font) = only_type0(&pdf, &pages[0]);
    assert_eq!(
        type0.get_name(b"BaseFont"),
        Some(&b"JustpdfTestHangulLatin-Regular"[..])
    );
    assert_eq!(font_file(&pdf, &cid_font), HANGUL_LATIN);
}

#[test]
fn widths_and_descriptor_metrics_come_from_the_font_tables() {
    let mut doc = DocumentBuilder::new();
    let font = doc.add_font(&program(HANGUL_LATIN));
    one_page(&mut doc, &font, "A c한");
    let (pdf, pages) = open(doc.build().unwrap());
    let (_, cid_font) = only_type0(&pdf, &pages[0]);

    // hmtx advances × 1000 / 2048: A 1489, space 573, c 991, 한 1925, .notdef 1925.
    let w = widths(cid_font.get_array(b"W").unwrap());
    assert_eq!(w, BTreeMap::from([(1, 727), (2, 280), (3, 484), (4, 940)]));
    assert_eq!(int(cid_font.get(b"DW").unwrap()), 940);

    let fd = dict(&pdf, cid_font.get(b"FontDescriptor").unwrap());
    let bbox: Vec<i64> = fd.get_array(b"FontBBox").unwrap().iter().map(int).collect();
    assert_eq!(bbox, vec![0, -198, 1005, 827]);
    assert_eq!(int(fd.get(b"Ascent").unwrap()), 856);
    assert_eq!(int(fd.get(b"Descent").unwrap()), -144);
    assert_eq!(int(fd.get(b"CapHeight").unwrap()), 743);
    assert_eq!(int(fd.get(b"ItalicAngle").unwrap()), 0);
    assert_eq!(int(fd.get(b"Flags").unwrap()), 32);
}

#[test]
fn a_font_measures_text_through_its_fallbacks() {
    let mut doc = DocumentBuilder::new();
    let both = doc.add_font(&program(HANGUL_LATIN));
    let hangul = doc.add_font(&program(HANGUL));
    let noto = doc.add_font(&program(NOTO_SANS));

    // (1489 + 573 + 991 + 1925) / 2048 em at 10 pt, unrounded.
    assert!((both.advance("A c한", 10.0) - 24.306640625).abs() < 1e-9);
    // Hangul from the primary (940), "A" from Noto Sans (639).
    let chain = hangul.with_fallback(&noto);
    assert!((chain.advance("한A", 10.0) - 15.79).abs() < 1e-9);
    // Measuring draws nothing.
    assert!(doc.missing_chars().is_empty());
}

#[test]
fn a_collection_face_is_selected_by_index() {
    let face0 = FontProgram::parse(HANGUL_TTC.to_vec(), 0).unwrap();
    let face1 = FontProgram::parse(HANGUL_TTC.to_vec(), 1).unwrap();
    assert_eq!(face0.postscript_name(), "JustpdfTestHangul-Regular");
    assert_eq!(face1.postscript_name(), "JustpdfTestHangulLatin-Regular");
    assert!(face0.glyph_id('A').is_none());
    assert!(face1.glyph_id('A').is_some());

    let mut doc = DocumentBuilder::new();
    let font = doc.add_font(&face1);
    one_page(&mut doc, &font, "한 A");
    let (pdf, pages) = open(doc.build().unwrap());
    assert_eq!(
        extract_page_text_string(&pdf, &pages[0]).unwrap().trim(),
        "한 A"
    );
    let (_, cid_font) = only_type0(&pdf, &pages[0]);
    let embedded = font_file(&pdf, &cid_font);
    assert_eq!(&embedded[..4], &[0, 1, 0, 0], "a standalone TrueType font");

    for (data, index) in [(HANGUL_TTC, 2), (HANGUL, 1)] {
        match FontProgram::parse(data.to_vec(), index) {
            Err(JustPdfError::FontError { detail }) => {
                assert!(detail.contains("face index"), "{detail}")
            }
            other => panic!("expected a face index error, got {other:?}"),
        }
    }
}

#[test]
fn a_cff_font_is_refused_naming_the_format() {
    let mut otto = b"OTTO".to_vec();
    otto.extend_from_slice(&[0; 8]);
    match FontProgram::parse(otto, 0) {
        Err(JustPdfError::FontError { detail }) => assert!(detail.contains("CFF"), "{detail}"),
        other => panic!("expected the CFF error, got {other:?}"),
    }
    match FontProgram::parse(b"not a font".to_vec(), 0) {
        Err(JustPdfError::FontError { .. }) => {}
        other => panic!("expected a font error, got {other:?}"),
    }
}

#[test]
fn a_collection_face_whose_tables_repeat_the_whole_file_is_refused() {
    // One face of 4,096 table records, each covering the whole 65 KiB file.
    let records = 4096usize;
    let len = 16 + 12 + 16 * records;
    let mut ttc = Vec::with_capacity(len);
    ttc.extend_from_slice(b"ttcf");
    ttc.extend_from_slice(&0x0001_0000u32.to_be_bytes());
    ttc.extend_from_slice(&1u32.to_be_bytes());
    ttc.extend_from_slice(&16u32.to_be_bytes());
    ttc.extend_from_slice(&0x0001_0000u32.to_be_bytes());
    ttc.extend_from_slice(&(records as u16).to_be_bytes());
    ttc.extend_from_slice(&[0; 6]);
    for i in 0..records {
        ttc.extend_from_slice(&(i as u32).to_be_bytes());
        ttc.extend_from_slice(&0u32.to_be_bytes());
        ttc.extend_from_slice(&0u32.to_be_bytes());
        ttc.extend_from_slice(&(len as u32).to_be_bytes());
    }
    assert_eq!(ttc.len(), len);
    match FontProgram::parse(ttc, 0) {
        Err(JustPdfError::FontError { detail }) => {
            assert!(detail.contains("more than the collection"), "{detail}")
        }
        other => panic!("expected a font error, got {other:?}"),
    }
}

#[test]
fn one_program_registered_on_two_documents_numbers_its_characters_independently() {
    let hangul = program(HANGUL);
    let mut first = DocumentBuilder::new();
    let mut second = DocumentBuilder::new();
    let in_first = first.add_font(&hangul);
    let in_second = second.add_font(&hangul);
    one_page(&mut first, &in_first, "한글");
    one_page(&mut second, &in_second, "글");

    let (pdf, pages) = open(second.build().unwrap());
    assert!(content(&pdf, &pages[0]).contains("<0001> Tj"));
    assert_eq!(
        extract_page_text_string(&pdf, &pages[0]).unwrap().trim(),
        "글"
    );
    let (pdf, pages) = open(first.build().unwrap());
    assert_eq!(
        extract_page_text_string(&pdf, &pages[0]).unwrap().trim(),
        "한글"
    );
}

#[test]
#[should_panic(expected = "different document")]
fn a_page_drawn_with_another_documents_font_is_refused() {
    let mut first = DocumentBuilder::new();
    let font = first.add_font(&program(HANGUL));
    let mut second = DocumentBuilder::new();
    one_page(&mut second, &font, "한");
}

#[test]
fn a_standard_font_handle_writes_win_ansi_and_reports_what_it_lacks() {
    let mut doc = DocumentBuilder::new();
    let helvetica = doc.add_standard_font("Helvetica");
    assert_eq!(
        doc.add_standard_font("Helvetica").resource_name(),
        helvetica.resource_name()
    );
    one_page(&mut doc, &helvetica, "café 한");
    assert_eq!(
        doc.missing_chars().into_iter().collect::<Vec<_>>(),
        vec!['한']
    );

    let (pdf, pages) = open(doc.build().unwrap());
    assert!(content(&pdf, &pages[0]).contains("<636166E9203F> Tj"));
    let fonts = page_fonts(&pdf, &pages[0]);
    let font = &fonts[helvetica.resource_name()];
    assert_eq!(font.get_name(b"BaseFont"), Some(&b"Helvetica"[..]));
    assert_eq!(font.get_name(b"Encoding"), Some(&b"WinAnsiEncoding"[..]));
    assert_eq!(
        extract_page_text_string(&pdf, &pages[0]).unwrap().trim(),
        "café ?"
    );
}

#[test]
fn a_standard_font_can_close_an_embedded_fonts_chain() {
    let mut doc = DocumentBuilder::new();
    let hangul = doc.add_font(&program(HANGUL));
    let helvetica = doc.add_standard_font("Helvetica");
    one_page(&mut doc, &hangul.with_fallback(&helvetica), "한글 ok 😀");
    assert_eq!(
        doc.missing_chars().into_iter().collect::<Vec<_>>(),
        vec!['😀']
    );
    let (pdf, pages) = open(doc.build().unwrap());
    assert_eq!(
        extract_page_text_string(&pdf, &pages[0]).unwrap().trim(),
        "한글 ok 😀"
    );
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

/// A one-page source document drawn with a standard font.
fn source_document() -> PdfDocument {
    let mut doc = DocumentBuilder::new();
    let helvetica = doc.add_standard_font("Helvetica");
    one_page(&mut doc, &helvetica, "source");
    PdfDocument::from_bytes(doc.build().unwrap()).unwrap()
}

/// A page drawing `text` in a font the modifier registers.
fn modifier_with_page(source: &PdfDocument, text: &str) -> DocumentModifier {
    let mut modifier = DocumentModifier::from_document(source).unwrap();
    let hangul = modifier.add_font(&program(HANGUL));
    let helvetica = modifier.add_standard_font("Helvetica");
    let mut page = PageBuilder::new(612.0, 792.0);
    page.begin_text();
    page.set_font(&hangul.with_fallback(&helvetica), 24.0);
    page.move_to(72.0, 700.0);
    page.show_text(text);
    page.end_text();
    modifier.insert_page(1, page).unwrap();
    assert!(modifier.missing_chars().is_empty());
    modifier
}

#[test]
fn a_modifier_draws_with_an_embedded_font_on_an_existing_document() {
    let source = source_document();

    let plain = modifier_with_page(&source, "검토 ok").build().unwrap();
    // A full rewrite writes the Type0 font dictionary as a plain object.
    assert!(contains(&plain, b"/Identity-H"));
    let (pdf, pages) = open(plain);
    assert_eq!(pages.len(), 2);
    assert_eq!(
        extract_page_text_string(&pdf, &pages[1]).unwrap().trim(),
        "검토 ok"
    );

    let saved = incremental_save(&source, modifier_with_page(&source, "검토 ok")).unwrap();
    let (pdf, pages) = open(saved);
    assert_eq!(
        extract_page_text_string(&pdf, &pages[1]).unwrap().trim(),
        "검토 ok"
    );

    let mut modifier = modifier_with_page(&source, "검토 ok");
    let packed = modifier.pack_object_streams(100).unwrap();
    let bytes = modifier.build_with_xref_stream(&packed).unwrap();
    // The font was packed into an object stream and not written again beside it.
    assert!(!contains(&bytes, b"/Identity-H"));
    let (pdf, pages) = open(bytes);
    assert_eq!(
        extract_page_text_string(&pdf, &pages[1]).unwrap().trim(),
        "검토 ok"
    );
}

#[test]
fn a_modifier_refuses_a_page_drawn_with_another_documents_font() {
    let source = source_document();
    let mut other = DocumentBuilder::new();
    let font = other.add_font(&program(HANGUL));
    let mut page = PageBuilder::new(612.0, 792.0);
    page.set_font(&font, 12.0);
    let mut modifier = DocumentModifier::from_document(&source).unwrap();
    assert!(modifier.insert_page(1, page).is_err());
}

#[test]
fn a_standard_font_registered_before_garbage_collect_is_still_written() {
    let source = source_document();
    let mut modifier = DocumentModifier::from_document(&source).unwrap();
    let courier = modifier.add_standard_font("Courier");
    modifier.garbage_collect();
    let mut page = PageBuilder::new(612.0, 792.0);
    page.set_font(&courier, 12.0);
    page.show_text("hello");
    modifier.insert_page(1, page).unwrap();

    let (pdf, pages) = open(modifier.build().unwrap());
    let fonts = page_fonts(&pdf, &pages[1]);
    assert_eq!(
        fonts[courier.resource_name()].get_name(b"BaseFont"),
        Some(&b"Courier"[..])
    );
}
