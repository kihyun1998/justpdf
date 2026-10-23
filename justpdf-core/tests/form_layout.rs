//! Single-line layout of text field and combo box values: auto size, `/Q`,
//! vertical position from font metrics, clipping and `/Tx` marked content
//! (#325). The expected numbers are worked from pdf.js
//! `TextWidgetAnnotation._getAppearance` / `_computeFontSize` and the
//! Helvetica AFM (ascender 718, descender −207; H 722, i 222).

use justpdf_core::content::{Operand, parse_content_stream};
use justpdf_core::form::appearance::generate_field_appearance;
use justpdf_core::form::{FieldFlags, FieldType, FormField, parse_acroform};
use justpdf_core::page::Rect;
use justpdf_core::writer::{DocumentBuilder, DocumentModifier, PageBuilder};
use justpdf_core::{PdfDict, PdfDocument, PdfObject};

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

fn text(s: &str) -> PdfObject {
    PdfObject::String(s.as_bytes().to_vec())
}

fn helvetica() -> PdfObject {
    PdfObject::Dict(dict(vec![
        ("Type", name("Font")),
        ("Subtype", name("Type1")),
        ("BaseFont", name("Helvetica")),
        ("Encoding", name("WinAnsiEncoding")),
    ]))
}

/// A one-page document's modifier whose AcroForm has `/DR /Font` `fonts` and
/// the extra `entries`.
fn modifier(fonts: Vec<(&str, PdfObject)>, entries: Vec<(&str, PdfObject)>) -> DocumentModifier {
    let mut builder = DocumentBuilder::new();
    builder.add_page(PageBuilder::new(200.0, 100.0));
    let doc = PdfDocument::from_bytes(builder.build().unwrap()).unwrap();
    let mut m = DocumentModifier::from_document(&doc).unwrap();
    let mut acroform = dict(vec![(
        "DR",
        PdfObject::Dict(dict(vec![("Font", PdfObject::Dict(dict(fonts)))])),
    )]);
    for (k, v) in entries {
        acroform.insert(k.as_bytes().to_vec(), v);
    }
    let catalog = m.catalog_ref();
    let Some(PdfObject::Dict(mut c)) = m.find_object_pub(catalog.obj_num).cloned() else {
        panic!("catalog")
    };
    c.insert(b"AcroForm".to_vec(), PdfObject::Dict(acroform));
    m.set_object(catalog.obj_num, PdfObject::Dict(c));
    m
}

/// A 100 × 20 field of `field_type` showing `value` with `/DA` `da`, whose one
/// widget holds `widget_entries`.
fn field(
    m: &mut DocumentModifier,
    field_type: FieldType,
    value: &str,
    da: &str,
    widget_entries: Vec<(&str, PdfObject)>,
) -> FormField {
    let mut widget = dict(vec![("Type", name("Annot")), ("Subtype", name("Widget"))]);
    for (k, v) in widget_entries {
        widget.insert(k.as_bytes().to_vec(), v);
    }
    let widget = m.add_object(PdfObject::Dict(widget));
    FormField {
        name: "f".to_string(),
        partial_name: "f".to_string(),
        field_type,
        value: Some(text(value)),
        default_value: None,
        flags: FieldFlags::default(),
        options: Vec::new(),
        indices: Vec::new(),
        rect: Some(Rect {
            llx: 0.0,
            lly: 0.0,
            urx: 100.0,
            ury: 20.0,
        }),
        default_appearance: Some(da.to_string()),
        quadding: 0,
        max_len: 0,
        field_ref: widget.clone(),
        widgets: vec![widget],
        page_obj_num: None,
    }
}

fn content(m: &mut DocumentModifier, f: &FormField) -> Vec<u8> {
    let ap = generate_field_appearance(f, m).unwrap();
    let Some(PdfObject::Stream { dict, data }) = m.find_object_pub(ap.obj_num) else {
        panic!("appearance stream")
    };
    justpdf_core::stream::decode_stream(data, dict).unwrap()
}

fn number(o: &Operand) -> f64 {
    match o {
        Operand::Integer(i) => *i as f64,
        Operand::Real(r) => *r,
        other => panic!("expected a number, got {other:?}"),
    }
}

/// The operands of the first `operator` in `content`.
fn operands(content: &[u8], operator: &str) -> Vec<f64> {
    parse_content_stream(content)
        .unwrap()
        .into_iter()
        .find(|op| op.operator == operator.as_bytes())
        .unwrap_or_else(|| panic!("no {operator} in {}", String::from_utf8_lossy(content)))
        .operands
        .iter()
        .filter(|o| !matches!(o, Operand::Name(_)))
        .map(number)
        .collect()
}

fn has(content: &[u8], operator: &str) -> bool {
    parse_content_stream(content)
        .unwrap()
        .iter()
        .any(|op| op.operator == operator.as_bytes())
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-3
}

/// The text box of a 100 × 20 widget with the default 1 pt border width.
const BOX: [f64; 4] = [1.0, 1.0, 98.0, 18.0];

/// The baseline pdf.js places a `size` Helvetica line at in [`BOX`].
fn baseline(size: f64) -> f64 {
    BOX[1] + (BOX[3] - (0.718 + 0.207) * size) / 2.0 + 0.207 * size
}

#[test]
fn an_auto_sized_short_value_gets_the_size_pdfjs_computes() {
    let mut m = modifier(vec![("Helv", helvetica())], Vec::new());
    let f = field(&mut m, FieldType::Text, "Hi", "/Helv 0 Tf 0 g", Vec::new());
    let c = content(&mut m, &f);
    // floor₂(min((18 − 2) / 1.35, (98 − 4) / 0.944)) = 11.85.
    assert_eq!(operands(&c, "Tf"), [11.85]);
    let td = operands(&c, "Td");
    assert!(close(td[0], BOX[0] + 2.0), "{td:?}");
    assert!(close(td[1], baseline(11.85)), "{td:?}");
}

#[test]
fn a_long_auto_sized_value_shrinks_to_fit_the_width() {
    let mut m = modifier(vec![("Helv", helvetica())], Vec::new());
    let short = field(&mut m, FieldType::Text, "Hi", "/Helv 0 Tf 0 g", Vec::new());
    let long = field(
        &mut m,
        FieldType::Text,
        &"Hi".repeat(20),
        "/Helv 0 Tf 0 g",
        Vec::new(),
    );
    let short_size = operands(&content(&mut m, &short), "Tf")[0];
    let long_size = operands(&content(&mut m, &long), "Tf")[0];
    assert!(long_size < short_size, "{long_size} {short_size}");
    // 20 × 0.944 em at that size fits the text box less its 2 + 2 padding.
    assert!(
        20.0 * 0.944 * long_size <= BOX[2] - 4.0 + 1e-9,
        "{long_size}"
    );
}

#[test]
fn q_centres_or_right_aligns_the_value_and_the_widget_q_comes_first() {
    let mut m = modifier(vec![("Helv", helvetica())], Vec::new());
    let width = 0.944 * 12.0;
    for (q, widget_q, x) in [
        (1, None, BOX[0] + (BOX[2] - width) / 2.0),
        (2, None, BOX[0] + BOX[2] - width - 2.0),
        (0, Some(2), BOX[0] + BOX[2] - width - 2.0),
    ] {
        let entries = widget_q
            .map(|q| vec![("Q", PdfObject::Integer(q))])
            .unwrap_or_default();
        let mut f = field(&mut m, FieldType::Text, "Hi", "/Helv 12 Tf 0 g", entries);
        f.quadding = q;
        let td = operands(&content(&mut m, &f), "Td");
        assert!(
            close(td[0], x),
            "Q {q} widget {widget_q:?}: {td:?}, expected {x}"
        );
    }
}

#[test]
fn the_acroform_q_applies_to_a_field_without_its_own() {
    let mut m = modifier(
        vec![("Helv", helvetica())],
        vec![("Q", PdfObject::Integer(1))],
    );
    let field = m.add_object(PdfObject::Dict(dict(vec![
        ("FT", name("Tx")),
        ("T", text("a")),
        (
            "Rect",
            PdfObject::Array([0, 0, 100, 20].map(PdfObject::Integer).to_vec()),
        ),
    ])));
    let own = m.add_object(PdfObject::Dict(dict(vec![
        ("FT", name("Tx")),
        ("T", text("b")),
        ("Q", PdfObject::Integer(2)),
    ])));
    let catalog = m.catalog_ref();
    let Some(PdfObject::Dict(mut c)) = m.find_object_pub(catalog.obj_num).cloned() else {
        panic!("catalog")
    };
    let Some(PdfObject::Dict(mut acroform)) = c.get(b"AcroForm").cloned() else {
        panic!("acroform")
    };
    acroform.insert(
        b"Fields".to_vec(),
        PdfObject::Array(vec![PdfObject::Reference(field), PdfObject::Reference(own)]),
    );
    c.insert(b"AcroForm".to_vec(), PdfObject::Dict(acroform));
    m.set_object(catalog.obj_num, PdfObject::Dict(c));
    let doc = PdfDocument::from_bytes(m.build().unwrap()).unwrap();
    let form = parse_acroform(&doc).unwrap().unwrap();
    let q: Vec<u8> = form.fields.iter().map(|f| f.quadding).collect();
    assert_eq!(q, [1, 2]);
}

#[test]
fn a_12_pt_value_centres_its_ascent_to_descent_in_the_text_box() {
    let mut m = modifier(vec![("Helv", helvetica())], Vec::new());
    let f = field(&mut m, FieldType::Text, "Hi", "/Helv 12 Tf 0 g", Vec::new());
    let td = operands(&content(&mut m, &f), "Td");
    assert!(close(td[1], baseline(12.0)), "{td:?}");
}

#[test]
fn the_value_is_clipped_inside_tx_marked_content_with_the_background_inside() {
    let mut m = modifier(vec![("Helv", helvetica())], Vec::new());
    let mk = PdfObject::Dict(dict(vec![(
        "BG",
        PdfObject::Array(vec![PdfObject::Real(1.0)]),
    )]));
    let f = field(
        &mut m,
        FieldType::Text,
        "Hi",
        "/Helv 12 Tf 0 g",
        vec![("MK", mk)],
    );
    let c = String::from_utf8(content(&mut m, &f)).unwrap();
    assert!(c.starts_with("/Tx BMC\nq\n1 g\n0 0 100 20 re\nf\n"), "{c}");
    assert!(c.contains("1 1 98 18 re\nW\nn\nBT\n"), "{c}");
    assert!(c.ends_with("ET\nQ\nEMC\n"), "{c}");
}

#[test]
fn a_value_wider_than_the_box_keeps_its_size_and_is_clipped() {
    let mut m = modifier(vec![("Helv", helvetica())], Vec::new());
    let f = field(
        &mut m,
        FieldType::Text,
        &"Hi".repeat(20),
        "/Helv 12 Tf 0 g",
        Vec::new(),
    );
    let c = content(&mut m, &f);
    assert_eq!(operands(&c, "Tf"), [12.0]);
    assert_eq!(operands(&c, "re"), BOX);
    assert!(has(&c, "W"));
}

#[test]
fn a_password_field_draws_no_value() {
    let mut m = modifier(vec![("Helv", helvetica())], Vec::new());
    let mut f = field(
        &mut m,
        FieldType::Text,
        "secret",
        "/Helv 12 Tf 0 g",
        Vec::new(),
    );
    f.flags = FieldFlags(FieldFlags::PASSWORD);
    let c = content(&mut m, &f);
    assert!(
        !has(&c, "Tj") && !has(&c, "TJ"),
        "{}",
        String::from_utf8_lossy(&c)
    );
    assert_eq!(String::from_utf8(c).unwrap(), "/Tx BMC\nq\nQ\nEMC\n");
}

#[test]
fn an_empty_value_gives_empty_tx_marked_content() {
    let mut m = modifier(vec![("Helv", helvetica())], Vec::new());
    let f = field(&mut m, FieldType::Text, "", "/Helv 12 Tf 0 g", Vec::new());
    assert_eq!(
        String::from_utf8(content(&mut m, &f)).unwrap(),
        "/Tx BMC\nq\nQ\nEMC\n"
    );
}

#[test]
fn a_combo_box_is_auto_sized_in_a_text_box_without_its_arrow() {
    let mut m = modifier(vec![("Helv", helvetica())], Vec::new());
    let f = field(
        &mut m,
        FieldType::ComboBox,
        "Hi",
        "/Helv 0 Tf 0 g",
        Vec::new(),
    );
    let c = content(&mut m, &f);
    // The arrow area is h.min(20) = 20 wide; the text box is 100 − 20 − 2.
    let clip: Vec<f64> = parse_content_stream(&c)
        .unwrap()
        .windows(2)
        .find(|w| w[0].operator == b"re" && w[1].operator == b"W")
        .map(|w| w[0].operands.iter().map(number).collect())
        .expect("a clip");
    assert_eq!(clip, [1.0, 1.0, 78.0, 18.0]);
    assert_eq!(operands(&c, "Tf"), [11.85]);
}

#[test]
fn font_descriptor_metrics_come_before_the_fallback() {
    // A non-Standard-14 font: its descriptor's ascent/descent, else pdf.js's
    // 1.0 / 0.35.
    let font = |descriptor: Option<PdfObject>| {
        let mut d = dict(vec![
            ("Type", name("Font")),
            ("Subtype", name("TrueType")),
            ("BaseFont", name("Custom")),
            ("Encoding", name("WinAnsiEncoding")),
            ("FirstChar", PdfObject::Integer(32)),
            ("LastChar", PdfObject::Integer(126)),
            (
                "Widths",
                PdfObject::Array(vec![PdfObject::Integer(500); 95]),
            ),
        ]);
        if let Some(fd) = descriptor {
            d.insert(b"FontDescriptor".to_vec(), fd);
        }
        PdfObject::Dict(d)
    };
    let descriptor = PdfObject::Dict(dict(vec![
        ("Type", name("FontDescriptor")),
        ("FontName", name("Custom")),
        ("Ascent", PdfObject::Integer(800)),
        ("Descent", PdfObject::Integer(-200)),
    ]));
    for (fd, (asc, desc)) in [(Some(descriptor), (0.8, 0.2)), (None, (1.0, 0.35))] {
        let mut m = modifier(vec![("C", font(fd))], Vec::new());
        let f = field(&mut m, FieldType::Text, "Hi", "/C 12 Tf 0 g", Vec::new());
        let td = operands(&content(&mut m, &f), "Td");
        let expected = BOX[1] + (BOX[3] - (asc + desc) * 12.0) / 2.0 + desc * 12.0;
        assert!(
            close(td[1], expected),
            "{asc} {desc}: {td:?}, expected {expected}"
        );
    }
}

// Multiline and comb fields (#332): pdf.js `_getMultilineAppearance`,
// `_splitLine`, `_computeFontSize` (multiline branch), `_getCombAppearance`.
// Helvetica widths: a 556, b 556, space 278, digits 556.

/// The strings `content` shows, in order.
fn shown(content: &[u8]) -> Vec<String> {
    parse_content_stream(content)
        .unwrap()
        .into_iter()
        .filter(|op| op.operator == b"Tj")
        .flat_map(|op| op.operands)
        .filter_map(|o| match o {
            Operand::String(s) => Some(String::from_utf8(s).unwrap()),
            _ => None,
        })
        .collect()
}

/// The operands of every `Td` in `content`.
fn tds(content: &[u8]) -> Vec<[f64; 2]> {
    parse_content_stream(content)
        .unwrap()
        .into_iter()
        .filter(|op| op.operator == b"Td")
        .map(|op| [number(&op.operands[0]), number(&op.operands[1])])
        .collect()
}

/// `f` resized to `w` × `h`.
fn sized(mut f: FormField, w: f64, h: f64) -> FormField {
    f.rect = Some(Rect {
        llx: 0.0,
        lly: 0.0,
        urx: w,
        ury: h,
    });
    f
}

fn multiline(f: FormField) -> FormField {
    FormField {
        flags: FieldFlags(f.flags.0 | FieldFlags::MULTILINE),
        ..f
    }
}

#[test]
fn a_multiline_value_is_drawn_one_line_per_line_break() {
    let mut m = modifier(vec![("Helv", helvetica())], Vec::new());
    let f = field(
        &mut m,
        FieldType::Text,
        "one\ntwo",
        "/Helv 12 Tf 0 g",
        Vec::new(),
    );
    let f = multiline(sized(f, 100.0, 60.0));
    let c = content(&mut m, &f);
    assert_eq!(shown(&c), ["one", "two"]);
    // Text box 58 high, less 2 × 1 padding: 56; round(56 / (12 × 1.35)) = 3
    // lines, so the line height is 56 / 3.
    let td = tds(&c);
    assert!(close(td.last().unwrap()[1], -56.0 / 3.0), "{td:?}");
}

#[test]
fn a_multiline_value_wraps_at_the_last_space_or_before_the_overflowing_character() {
    let mut m = modifier(vec![("Helv", helvetica())], Vec::new());
    // Text box 38 wide: lines wrap past 38 − 4 = 34.
    for (value, lines) in [
        ("aaa bbb", vec!["aaa ", "bbb"]),
        ("aaaaaaaa", vec!["aaaaa", "aaa"]),
    ] {
        let f = field(
            &mut m,
            FieldType::Text,
            value,
            "/Helv 12 Tf 0 g",
            Vec::new(),
        );
        let f = multiline(sized(f, 40.0, 60.0));
        assert_eq!(shown(&content(&mut m, &f)), lines, "{value}");
    }
}

#[test]
fn an_auto_sized_multiline_value_fits_its_wrapped_lines_in_the_height() {
    let mut m = modifier(vec![("Helv", helvetica())], Vec::new());
    // round(56 / (12 × 1.35)) = 3 ≥ 2 lines: floor₂(56 / 3 / 1.35) = 13.82,
    // and both lines fit at that size.
    let f = field(
        &mut m,
        FieldType::Text,
        "one\ntwo",
        "/Helv 0 Tf 0 g",
        Vec::new(),
    );
    let f = multiline(sized(f, 100.0, 60.0));
    assert_eq!(operands(&content(&mut m, &f), "Tf"), [13.82]);

    // A value that wraps into more lines than 3 gets a smaller size whose
    // wrapped lines still fit the 56 pt height.
    let long = "word ".repeat(40);
    let f = field(&mut m, FieldType::Text, &long, "/Helv 0 Tf 0 g", Vec::new());
    let f = multiline(sized(f, 100.0, 60.0));
    let c = content(&mut m, &f);
    let size = operands(&c, "Tf")[0];
    assert!(size < 13.82, "{size}");
    assert!(
        shown(&c).len() as f64 * size <= 56.0 + 1e-9,
        "{size} × {}",
        shown(&c).len()
    );
}

#[test]
fn a_comb_field_centres_one_character_in_each_cell() {
    let mut m = modifier(vec![("Helv", helvetica())], Vec::new());
    let mut f = field(
        &mut m,
        FieldType::Text,
        "12345",
        "/Helv 12 Tf 0 g",
        Vec::new(),
    );
    f.flags = FieldFlags(FieldFlags::COMB);
    f.max_len = 5;
    let c = content(&mut m, &f);
    assert_eq!(shown(&c), ["1", "2", "3", "4", "5"]);
    // Cells are 98 / 5 = 19.6 wide; a digit is 0.556 × 12 = 6.672 wide.
    let td = tds(&c);
    let steps: Vec<f64> = td[1..].iter().map(|t| t[0]).collect();
    let first = (19.6 - 6.672) / 2.0;
    assert!(close(steps[0], first), "{td:?}");
    for step in &steps[1..] {
        assert!(close(*step, 19.6), "{td:?}");
    }
    assert!(close(td[0][0], BOX[0]), "{td:?}");
}

#[test]
fn a_comb_needs_a_max_len_and_no_multiline() {
    let mut m = modifier(vec![("Helv", helvetica())], Vec::new());
    let mut f = field(
        &mut m,
        FieldType::Text,
        "12345",
        "/Helv 12 Tf 0 g",
        Vec::new(),
    );
    f.flags = FieldFlags(FieldFlags::COMB);
    assert_eq!(shown(&content(&mut m, &f)), ["12345"]);

    f.max_len = 5;
    f.flags = FieldFlags(FieldFlags::COMB | FieldFlags::MULTILINE);
    assert_eq!(shown(&content(&mut m, &f)), ["12345"]);
}

#[test]
fn max_len_is_inherited_from_a_parent_field() {
    let mut m = modifier(vec![("Helv", helvetica())], Vec::new());
    let parent = m.add_object(PdfObject::Null);
    let kid = m.add_object(PdfObject::Dict(dict(vec![
        ("T", text("kid")),
        ("Parent", PdfObject::Reference(parent.clone())),
        (
            "Rect",
            PdfObject::Array([0, 0, 100, 20].map(PdfObject::Integer).to_vec()),
        ),
    ])));
    m.set_object(
        parent.obj_num,
        PdfObject::Dict(dict(vec![
            ("FT", name("Tx")),
            ("T", text("p")),
            ("MaxLen", PdfObject::Integer(7)),
            ("Kids", PdfObject::Array(vec![PdfObject::Reference(kid)])),
        ])),
    );
    let catalog = m.catalog_ref();
    let Some(PdfObject::Dict(mut c)) = m.find_object_pub(catalog.obj_num).cloned() else {
        panic!("catalog")
    };
    let Some(PdfObject::Dict(mut acroform)) = c.get(b"AcroForm").cloned() else {
        panic!("acroform")
    };
    acroform.insert(
        b"Fields".to_vec(),
        PdfObject::Array(vec![PdfObject::Reference(parent)]),
    );
    c.insert(b"AcroForm".to_vec(), PdfObject::Dict(acroform));
    m.set_object(catalog.obj_num, PdfObject::Dict(c));
    let doc = PdfDocument::from_bytes(m.build().unwrap()).unwrap();
    let form = parse_acroform(&doc).unwrap().unwrap();
    assert_eq!(form.fields[0].name, "p.kid");
    assert_eq!(form.fields[0].max_len, 7);
}
