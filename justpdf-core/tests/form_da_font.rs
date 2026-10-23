//! Form field appearances drawn in the AcroForm `/DR` font their `/DA` names
//! (#178).

use justpdf_core::font::FontProgram;
use justpdf_core::font::subset::subset_font;
use justpdf_core::form::appearance::{AppearanceError, generate_field_appearance};
use justpdf_core::form::{FieldFlags, FieldType, FormField};
use justpdf_core::page::{Rect, collect_pages};
use justpdf_core::text::extract_page_text_string;
use justpdf_core::writer::{DocumentBuilder, DocumentModifier, PageBuilder};
use justpdf_core::{IndirectRef, PdfDict, PdfDocument, PdfObject, ttf_parser};

const HANGUL: &[u8] = include_bytes!("fixtures/JustpdfTestHangul-Regular.ttf");
const NOTO_SANS: &[u8] = include_bytes!("fixtures/NotoSans-Regular.ttf");

fn name(n: &str) -> PdfObject {
    PdfObject::Name(n.as_bytes().to_vec())
}

fn dict(entries: Vec<(&str, PdfObject)>) -> PdfDict {
    let mut d = PdfDict::new();
    for (k, v) in entries {
        d.insert(k.as_bytes().to_vec(), v);
    }
    d
}

fn stream(data: &[u8]) -> PdfObject {
    PdfObject::Stream {
        dict: dict(vec![("Length", PdfObject::Integer(data.len() as i64))]),
        data: data.to_vec(),
    }
}

/// A one-page source document.
fn source() -> PdfDocument {
    let mut builder = DocumentBuilder::new();
    builder.add_page(PageBuilder::new(200.0, 100.0));
    PdfDocument::from_bytes(builder.build().unwrap()).unwrap()
}

/// Give `modifier`'s catalog an AcroForm whose `/DR /Font` is `fonts`.
fn set_dr(modifier: &mut DocumentModifier, fonts: PdfDict) {
    let acroform = dict(vec![(
        "DR",
        PdfObject::Dict(dict(vec![("Font", PdfObject::Dict(fonts))])),
    )]);
    let catalog = modifier.catalog_ref();
    let Some(PdfObject::Dict(mut catalog_dict)) =
        modifier.find_object_pub(catalog.obj_num).cloned()
    else {
        panic!("catalog")
    };
    catalog_dict.insert(b"AcroForm".to_vec(), PdfObject::Dict(acroform));
    modifier.set_object(catalog.obj_num, PdfObject::Dict(catalog_dict));
}

/// `doc` with an AcroForm whose `/DR /Font` holds `fonts`, each added as an
/// indirect object.
fn with_dr(doc: &PdfDocument, fonts: Vec<(&str, PdfObject)>) -> DocumentModifier {
    let mut modifier = DocumentModifier::from_document(doc).unwrap();
    let mut font_dict = PdfDict::new();
    for (n, font) in fonts {
        let r = modifier.add_object(font);
        font_dict.insert(n.as_bytes().to_vec(), PdfObject::Reference(r));
    }
    set_dr(&mut modifier, font_dict);
    modifier
}

fn field(field_type: FieldType, value: &str, da: Option<&str>) -> FormField {
    FormField {
        name: "f".to_string(),
        partial_name: "f".to_string(),
        field_type,
        value: Some(PdfObject::String(value.as_bytes().to_vec())),
        default_value: None,
        flags: FieldFlags::default(),
        options: vec![value.to_string()],
        rect: Some(Rect {
            llx: 0.0,
            lly: 0.0,
            urx: 200.0,
            ury: 20.0,
        }),
        default_appearance: da.map(str::to_string),
        quadding: 0,
        field_ref: IndirectRef {
            obj_num: 1,
            gen_num: 0,
        },
        widgets: Vec::new(),
        page_obj_num: None,
    }
}

fn helvetica_win_ansi() -> PdfObject {
    PdfObject::Dict(dict(vec![
        ("Type", name("Font")),
        ("Subtype", name("Type1")),
        ("BaseFont", name("Helvetica")),
        ("Encoding", name("WinAnsiEncoding")),
    ]))
}

/// The appearance stream `r` points to: (decoded content, dictionary).
fn appearance(modifier: &DocumentModifier, r: &IndirectRef) -> (Vec<u8>, PdfDict) {
    let Some(PdfObject::Stream { dict, data }) = modifier.find_object_pub(r.obj_num) else {
        panic!("expected an appearance stream")
    };
    (
        justpdf_core::stream::decode_stream(data, dict).unwrap(),
        dict.clone(),
    )
}

fn shown_strings(content: &[u8]) -> Vec<Vec<u8>> {
    justpdf_core::content::parse_content_stream(content)
        .unwrap()
        .into_iter()
        .filter(|op| op.operator == b"Tj")
        .flat_map(|op| op.operands)
        .filter_map(|o| match o {
            justpdf_core::content::Operand::String(s) => Some(s),
            _ => None,
        })
        .collect()
}

/// The appearance's `/Resources /Font` entry `font`.
fn declared_font<'a>(dict: &'a PdfDict, font: &str) -> Option<&'a PdfObject> {
    dict.get_dict(b"Resources")?
        .get_dict(b"Font")?
        .get(font.as_bytes())
}

/// `field` gets "not encodable", and nothing is added to `modifier`.
fn assert_not_encodable(modifier: &mut DocumentModifier, field: &FormField) {
    let before = modifier.writer().objects.len();
    assert_eq!(
        generate_field_appearance(field, modifier),
        Err(AppearanceError::NotEncodable),
        "{:?} {:?}",
        field.value_as_string(),
        field.default_appearance
    );
    assert_eq!(modifier.writer().objects.len(), before);
}

/// Text extraction of `doc`'s first page after `modifier` makes it draw the
/// appearance `ap`.
fn extract_drawn(doc: &PdfDocument, mut modifier: DocumentModifier, ap: IndirectRef) -> String {
    // Text extraction does not enter Form XObjects, so the page draws the
    // appearance's content with its resources.
    let (ap_content, ap_dict) = appearance(&modifier, &ap);
    let page_ref = collect_pages(doc).unwrap()[0].page_ref.clone();
    let content = modifier.add_object(stream(&ap_content));
    let Some(PdfObject::Dict(mut page)) = modifier.find_object_pub(page_ref.obj_num).cloned()
    else {
        panic!("page")
    };
    page.insert(
        b"Resources".to_vec(),
        ap_dict.get(b"Resources").unwrap().clone(),
    );
    page.insert(b"Contents".to_vec(), PdfObject::Reference(content));
    modifier.set_object(page_ref.obj_num, PdfObject::Dict(page));
    let out = PdfDocument::from_bytes(modifier.build().unwrap()).unwrap();
    let pages = collect_pages(&out).unwrap();
    extract_page_text_string(&out, &pages[0]).unwrap()
}

#[test]
fn a_text_field_draws_in_its_dr_font_and_declares_it() {
    let doc = source();
    let mut modifier = with_dr(&doc, vec![("Helv", helvetica_win_ansi())]);
    let f = field(FieldType::Text, "café", Some("/Helv 12 Tf 0 g"));
    let ap = generate_field_appearance(&f, &mut modifier).unwrap();
    let (content, ap_dict) = appearance(&modifier, &ap);
    assert_eq!(shown_strings(&content), vec![vec![0x63, 0x61, 0x66, 0xE9]]);
    assert!(
        matches!(
            declared_font(&ap_dict, "Helv"),
            Some(PdfObject::Reference(_))
        ),
        "{ap_dict:?}"
    );
    assert_eq!(extract_drawn(&doc, modifier, ap).trim(), "café");
}

#[test]
fn differences_give_the_code_of_their_glyph() {
    let doc = source();
    // StandardEncoding has no Euro; only the /Differences entry gives it a code.
    let encoding = dict(vec![
        ("Type", name("Encoding")),
        (
            "Differences",
            PdfObject::Array(vec![PdfObject::Integer(128), name("Euro")]),
        ),
    ]);
    let font = PdfObject::Dict(dict(vec![
        ("Type", name("Font")),
        ("Subtype", name("Type1")),
        ("BaseFont", name("Helvetica")),
        ("Encoding", PdfObject::Dict(encoding)),
    ]));
    let mut modifier = with_dr(&doc, vec![("F1", font)]);
    let f = field(FieldType::Text, "1€", Some("/F1 10 Tf"));
    let ap = generate_field_appearance(&f, &mut modifier).unwrap();
    let (content, _) = appearance(&modifier, &ap);
    assert_eq!(shown_strings(&content), vec![vec![b'1', 0x80]]);
}

const TO_UNICODE_0041_GA: &[u8] = b"/CIDInit /ProcSet findresource begin
12 dict begin
begincmap
1 begincodespacerange
<0000> <FFFF>
endcodespacerange
1 beginbfchar
<0041> <AC00>
endbfchar
endcmap
end
end
";

/// A non-embedded Type0 `Identity-H` font, with the ToUnicode CMap stream
/// `to_unicode` refers to when it is given.
fn type0(to_unicode: Option<IndirectRef>) -> PdfObject {
    let descendant = dict(vec![
        ("Type", name("Font")),
        ("Subtype", name("CIDFontType2")),
        ("BaseFont", name("Batang")),
    ]);
    let mut font = dict(vec![
        ("Type", name("Font")),
        ("Subtype", name("Type0")),
        ("BaseFont", name("Batang")),
        ("Encoding", name("Identity-H")),
        (
            "DescendantFonts",
            PdfObject::Array(vec![PdfObject::Dict(descendant)]),
        ),
    ]);
    if let Some(cmap) = to_unicode {
        font.insert(b"ToUnicode".to_vec(), PdfObject::Reference(cmap));
    }
    PdfObject::Dict(font)
}

#[test]
fn a_type0_font_encodes_through_the_inverse_of_its_to_unicode() {
    let doc = source();
    let mut modifier = DocumentModifier::from_document(&doc).unwrap();
    let cmap = modifier.add_object(stream(TO_UNICODE_0041_GA));
    let font = modifier.add_object(type0(Some(cmap)));
    set_dr(&mut modifier, dict(vec![("K", PdfObject::Reference(font))]));
    let f = field(FieldType::Text, "가", Some("/K 12 Tf"));
    let ap = generate_field_appearance(&f, &mut modifier).unwrap();
    let (content, _) = appearance(&modifier, &ap);
    assert_eq!(shown_strings(&content), vec![vec![0x00, 0x41]]);
    assert_eq!(extract_drawn(&doc, modifier, ap).trim(), "가");
}

#[test]
fn a_value_the_dr_font_cannot_hold_gives_no_appearance() {
    let doc = source();
    let mut modifier = with_dr(
        &doc,
        vec![("Helv", helvetica_win_ansi()), ("K", type0(None))],
    );
    for (value, da) in [
        ("한글", "/Helv 12 Tf"),
        ("abc", "/Missing 12 Tf"),
        ("가", "/K 12 Tf"),
        ("abc", "0 g"),
    ] {
        assert_not_encodable(&mut modifier, &field(FieldType::Text, value, Some(da)));
    }
}

#[test]
fn a_field_without_dr_gives_no_appearance() {
    let doc = source();
    let mut modifier = DocumentModifier::from_document(&doc).unwrap();
    assert_not_encodable(
        &mut modifier,
        &field(FieldType::Text, "abc", Some("/Helv 12 Tf")),
    );
}

#[test]
fn a_simple_truetype_subset_encodes_only_the_glyphs_it_kept() {
    let face = ttf_parser::Face::parse(NOTO_SANS, 0).unwrap();
    let gids: Vec<u16> = "Hel"
        .chars()
        .map(|c| face.glyph_index(c).unwrap().0)
        .collect();
    let program = subset_font(NOTO_SANS, &gids).unwrap().data;
    let doc = source();
    let mut modifier = DocumentModifier::from_document(&doc).unwrap();
    let file = modifier.add_object(stream(&program));
    let descriptor = dict(vec![
        ("Type", name("FontDescriptor")),
        ("FontName", name("ABCDEF+NotoSans-Regular")),
        ("Flags", PdfObject::Integer(32)),
        ("FontFile2", PdfObject::Reference(file)),
    ]);
    let font = modifier.add_object(PdfObject::Dict(dict(vec![
        ("Type", name("Font")),
        ("Subtype", name("TrueType")),
        ("BaseFont", name("ABCDEF+NotoSans-Regular")),
        ("Encoding", name("WinAnsiEncoding")),
        ("FontDescriptor", PdfObject::Dict(descriptor)),
    ])));
    set_dr(&mut modifier, dict(vec![("N", PdfObject::Reference(font))]));
    let ap = generate_field_appearance(
        &field(FieldType::Text, "Hell", Some("/N 9 Tf")),
        &mut modifier,
    );
    assert!(ap.is_ok(), "{ap:?}");
    // `p` has a WinAnsi code and a glyph in Noto Sans, but not in the subset.
    assert_not_encodable(
        &mut modifier,
        &field(FieldType::Text, "Help", Some("/N 9 Tf")),
    );
}

#[test]
fn a_justpdf_type0_font_does_not_encode_what_it_drew_as_notdef() {
    let mut builder = DocumentBuilder::new();
    let hangul = builder.add_font(&FontProgram::parse(HANGUL.to_vec(), 0).unwrap());
    let mut page = PageBuilder::new(200.0, 100.0);
    page.begin_text();
    page.set_font(&hangul, 12.0);
    page.show_text("한😀");
    page.end_text();
    builder.add_page(page);
    let doc = PdfDocument::from_bytes(builder.build().unwrap()).unwrap();
    let pages = collect_pages(&doc).unwrap();
    let resources = match pages[0].resources_ref.as_ref().unwrap() {
        PdfObject::Reference(r) => doc.resolve(r).unwrap(),
        other => other.clone(),
    };
    let font_ref = resources
        .as_dict()
        .unwrap()
        .get_dict(b"Font")
        .unwrap()
        .get(hangul.resource_name().as_bytes())
        .cloned()
        .unwrap();
    let mut modifier = DocumentModifier::from_document(&doc).unwrap();
    set_dr(&mut modifier, dict(vec![("H", font_ref)]));

    let ap = generate_field_appearance(
        &field(FieldType::Text, "한", Some("/H 9 Tf")),
        &mut modifier,
    )
    .unwrap();
    let (content, _) = appearance(&modifier, &ap);
    assert_eq!(shown_strings(&content), vec![vec![0x00, 0x01]]);
    // ToUnicode maps CID 2 to the emoji, which was drawn as `.notdef`.
    assert_not_encodable(
        &mut modifier,
        &field(FieldType::Text, "😀", Some("/H 9 Tf")),
    );
}

#[test]
fn choice_fields_and_buttons_with_da_use_the_dr_font() {
    let doc = source();
    for field_type in [
        FieldType::ComboBox,
        FieldType::ListBox,
        FieldType::PushButton,
    ] {
        let mut modifier = with_dr(&doc, vec![("Helv", helvetica_win_ansi())]);
        let ap = generate_field_appearance(
            &field(field_type, "café", Some("/Helv 12 Tf 0 g")),
            &mut modifier,
        )
        .unwrap();
        let (content, ap_dict) = appearance(&modifier, &ap);
        assert_eq!(
            shown_strings(&content),
            vec![vec![0x63, 0x61, 0x66, 0xE9]],
            "{field_type:?}"
        );
        assert!(declared_font(&ap_dict, "Helv").is_some(), "{field_type:?}");
        assert!(
            declared_font(&ap_dict, "Helvetica").is_none(),
            "{field_type:?}"
        );
        assert_not_encodable(
            &mut modifier,
            &field(field_type, "한", Some("/Helv 12 Tf 0 g")),
        );
    }
}

#[test]
fn a_simple_font_with_to_unicode_encodes_through_it() {
    let doc = source();
    let mut modifier = DocumentModifier::from_document(&doc).unwrap();
    let cmap = modifier.add_object(stream(
        b"begincmap\n1 begincodespacerange\n<00> <FF>\nendcodespacerange\n\
1 beginbfchar\n<41> <AC00>\nendbfchar\nendcmap\n",
    ));
    let font = modifier.add_object(PdfObject::Dict(dict(vec![
        ("Type", name("Font")),
        ("Subtype", name("Type1")),
        ("BaseFont", name("Helvetica")),
        ("Encoding", name("WinAnsiEncoding")),
        ("ToUnicode", PdfObject::Reference(cmap)),
    ])));
    set_dr(&mut modifier, dict(vec![("F", PdfObject::Reference(font))]));
    // Text extraction reads code 0x41 as `가`, so `A` has no code left.
    let ap = generate_field_appearance(
        &field(FieldType::Text, "가b", Some("/F 9 Tf")),
        &mut modifier,
    )
    .unwrap();
    let (content, _) = appearance(&modifier, &ap);
    assert_eq!(shown_strings(&content), vec![vec![0x41, b'b']]);
    assert_not_encodable(&mut modifier, &field(FieldType::Text, "A", Some("/F 9 Tf")));
}

#[test]
fn a_subset_font_whose_program_is_not_truetype_gives_no_appearance() {
    let doc = source();
    let mut modifier = DocumentModifier::from_document(&doc).unwrap();
    let file = modifier.add_object(stream(b"not read"));
    let descriptor = dict(vec![
        ("Type", name("FontDescriptor")),
        ("FontName", name("ABCDEF+Minion")),
        ("Flags", PdfObject::Integer(32)),
        ("FontFile3", PdfObject::Reference(file)),
    ]);
    let font = modifier.add_object(PdfObject::Dict(dict(vec![
        ("Type", name("Font")),
        ("Subtype", name("Type1")),
        ("BaseFont", name("ABCDEF+Minion")),
        ("Encoding", name("WinAnsiEncoding")),
        ("FontDescriptor", PdfObject::Dict(descriptor)),
    ])));
    set_dr(&mut modifier, dict(vec![("M", PdfObject::Reference(font))]));
    assert_not_encodable(&mut modifier, &field(FieldType::Text, "a", Some("/M 9 Tf")));
}

#[test]
fn a_type0_font_with_a_predefined_cmap_other_than_identity_gives_no_appearance() {
    let doc = source();
    let mut modifier = DocumentModifier::from_document(&doc).unwrap();
    let cmap = modifier.add_object(stream(TO_UNICODE_0041_GA));
    let PdfObject::Dict(mut font) = type0(Some(cmap)) else {
        unreachable!()
    };
    font.insert(b"Encoding".to_vec(), name("UniKS-UCS2-H"));
    let font = modifier.add_object(PdfObject::Dict(font));
    set_dr(&mut modifier, dict(vec![("K", PdfObject::Reference(font))]));
    assert_not_encodable(
        &mut modifier,
        &field(FieldType::Text, "가", Some("/K 9 Tf")),
    );
}

/// The last fill-colour operator before the first `Tj` of `content`, with
/// its operands.
fn fill_colour_before_tj(content: &[u8]) -> String {
    let mut last = String::new();
    for op in justpdf_core::content::parse_content_stream(content).unwrap() {
        match op.operator.as_slice() {
            b"Tj" => break,
            b"g" | b"rg" | b"k" | b"sc" | b"scn" => {
                let operands: Vec<String> = op
                    .operands
                    .iter()
                    .map(|o| match o {
                        justpdf_core::content::Operand::Integer(i) => i.to_string(),
                        justpdf_core::content::Operand::Real(r) => r.to_string(),
                        other => format!("{other:?}"),
                    })
                    .collect();
                last = format!(
                    "{} {}",
                    operands.join(" "),
                    String::from_utf8_lossy(&op.operator)
                );
            }
            _ => {}
        }
    }
    last
}

#[test]
fn every_field_draws_its_text_in_the_colour_da_sets() {
    let doc = source();
    for field_type in [
        FieldType::Text,
        FieldType::ComboBox,
        FieldType::ListBox,
        FieldType::PushButton,
    ] {
        for (da, colour) in [("/Helv 12 Tf 1 0 0 rg", "1 0 0 rg"), ("/Helv 12 Tf", "0 g")] {
            let mut modifier = with_dr(&doc, vec![("Helv", helvetica_win_ansi())]);
            let ap = generate_field_appearance(&field(field_type, "Hi", Some(da)), &mut modifier)
                .unwrap();
            let (content, _) = appearance(&modifier, &ap);
            assert_eq!(
                fill_colour_before_tj(&content),
                colour,
                "{field_type:?} {da}"
            );
        }
    }
}
