//! Form field appearances drawn with the widget's `/MK` background and border
//! colours at its `/BS` (or `/Border`) width (#333).

use justpdf_core::form::appearance::generate_field_appearance;
use justpdf_core::form::{FieldFlags, FieldType, FormField, parse_acroform, set_field_value};
use justpdf_core::page::Rect;
use justpdf_core::writer::{DocumentBuilder, DocumentModifier, PageBuilder};
use justpdf_core::{IndirectRef, PdfDict, PdfDocument, PdfObject};

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

fn reals(values: &[f64]) -> PdfObject {
    PdfObject::Array(values.iter().map(|&v| PdfObject::Real(v)).collect())
}

/// A one-page document's modifier.
fn modifier() -> DocumentModifier {
    let mut builder = DocumentBuilder::new();
    builder.add_page(PageBuilder::new(200.0, 100.0));
    let doc = PdfDocument::from_bytes(builder.build().unwrap()).unwrap();
    DocumentModifier::from_document(&doc).unwrap()
}

/// `/MK` with `/BG` `bg` and `/BC` `bc`, each left out when `None`.
fn mk(bg: Option<&[f64]>, bc: Option<&[f64]>) -> (&'static str, PdfObject) {
    let mut entries = Vec::new();
    if let Some(bg) = bg {
        entries.push(("BG", reals(bg)));
    }
    if let Some(bc) = bc {
        entries.push(("BC", reals(bc)));
    }
    ("MK", PdfObject::Dict(dict(entries)))
}

fn bs(width: f64) -> (&'static str, PdfObject) {
    (
        "BS",
        PdfObject::Dict(dict(vec![("W", PdfObject::Real(width))])),
    )
}

/// A field of `field_type` with value `value`, 100 × 20, whose one widget is a
/// dictionary with `entries` added to `modifier`.
fn field(
    modifier: &mut DocumentModifier,
    field_type: FieldType,
    value: PdfObject,
    entries: Vec<(&str, PdfObject)>,
) -> FormField {
    let mut widget = dict(vec![("Type", name("Annot")), ("Subtype", name("Widget"))]);
    for (k, v) in entries {
        widget.insert(k.as_bytes().to_vec(), v);
    }
    let widget = modifier.add_object(PdfObject::Dict(widget));
    FormField {
        name: "f".to_string(),
        partial_name: "f".to_string(),
        field_type,
        value: Some(value),
        default_value: None,
        flags: FieldFlags::default(),
        options: vec!["a".to_string()],
        rect: Some(Rect {
            llx: 0.0,
            lly: 0.0,
            urx: 100.0,
            ury: 20.0,
        }),
        default_appearance: None,
        quadding: 0,
        max_len: 0,
        field_ref: widget.clone(),
        widgets: vec![widget],
        page_obj_num: None,
    }
}

/// The decoded content of the appearance `r` points to.
fn content(modifier: &DocumentModifier, r: &IndirectRef) -> String {
    let Some(PdfObject::Stream { dict, data }) = modifier.find_object_pub(r.obj_num) else {
        panic!("expected an appearance stream")
    };
    String::from_utf8(justpdf_core::stream::decode_stream(data, dict).unwrap()).unwrap()
}

/// The appearance of `field` drawn in `modifier`, without the
/// `/Tx BMC q` a text field or combo box opens with (#325).
fn drawn(modifier: &mut DocumentModifier, field: &FormField) -> String {
    let ap = generate_field_appearance(field, modifier).unwrap();
    let c = content(modifier, &ap);
    c.strip_prefix("/Tx BMC\nq\n")
        .map(str::to_string)
        .unwrap_or(c)
}

/// Whether `c` fills or strokes a rectangle: a background or a border.
fn has_box(c: &str) -> bool {
    c.contains("re\nf\n") || c.contains("re\nS\n")
}

fn text(s: &str) -> PdfObject {
    PdfObject::String(s.as_bytes().to_vec())
}

#[test]
fn a_text_field_is_drawn_with_its_mk_colours_at_its_bs_width() {
    let mut m = modifier();
    let f = field(
        &mut m,
        FieldType::Text,
        text("x"),
        vec![mk(Some(&[1.0, 0.0, 0.0]), Some(&[0.0, 0.0, 1.0])), bs(3.0)],
    );
    let c = drawn(&mut m, &f);
    assert!(c.starts_with("1 0 0 rg\n0 0 100 20 re\nf\n"), "{c}");
    assert!(c.contains("3 w\n0 0 1 RG\n1.5 1.5 97 17 re\nS\n"), "{c}");
}

#[test]
fn a_widget_without_mk_has_no_background_or_border() {
    let mut m = modifier();
    let f = field(&mut m, FieldType::Text, text("x"), vec![bs(3.0)]);
    let c = drawn(&mut m, &f);
    assert!(!has_box(&c), "{c}");
}

#[test]
fn the_border_width_defaults_to_1_and_a_zero_width_draws_no_border() {
    let mut m = modifier();
    let f = field(
        &mut m,
        FieldType::Text,
        text("x"),
        vec![mk(None, Some(&[0.0]))],
    );
    let c = drawn(&mut m, &f);
    assert!(c.contains("1 w\n0 G\n0.5 0.5 99 19 re\nS\n"), "{c}");

    let border = ("Border", reals(&[0.0, 0.0, 2.0]));
    let f = field(
        &mut m,
        FieldType::Text,
        text("x"),
        vec![mk(None, Some(&[0.0])), border],
    );
    let c = drawn(&mut m, &f);
    assert!(c.contains("2 w\n0 G\n1 1 98 18 re\nS\n"), "{c}");

    let f = field(
        &mut m,
        FieldType::Text,
        text("x"),
        vec![mk(None, Some(&[0.0])), bs(0.0)],
    );
    let c = drawn(&mut m, &f);
    assert!(!c.contains("re\nS"), "{c}");
}

#[test]
fn colour_arrays_pick_their_colour_space_by_length() {
    let mut m = modifier();
    let f = field(
        &mut m,
        FieldType::Text,
        text("x"),
        vec![mk(Some(&[0.0, 0.1, 0.2, 0.3]), Some(&[0.5]))],
    );
    let c = drawn(&mut m, &f);
    assert!(c.starts_with("0 0.1 0.2 0.3 k\n0 0 100 20 re\nf\n"), "{c}");
    assert!(c.contains("0.5 G\n"), "{c}");

    let f = field(
        &mut m,
        FieldType::Text,
        text("x"),
        vec![mk(Some(&[]), None)],
    );
    let c = drawn(&mut m, &f);
    assert!(!has_box(&c), "{c}");
}

#[test]
fn every_rectangle_field_type_follows_mk() {
    let red_on_blue = || vec![mk(Some(&[1.0, 0.0, 0.0]), Some(&[0.0, 0.0, 1.0]))];
    for (field_type, value) in [
        (FieldType::ComboBox, text("a")),
        (FieldType::ListBox, text("a")),
        (FieldType::PushButton, text("Go")),
        (FieldType::Checkbox, name("Yes")),
    ] {
        let mut m = modifier();
        let f = field(&mut m, field_type, value.clone(), red_on_blue());
        let c = drawn(&mut m, &f);
        assert!(
            c.starts_with("1 0 0 rg\n0 0 100 20 re\nf\n"),
            "{field_type:?}: {c}"
        );
        assert!(
            c.contains("1 w\n0 0 1 RG\n0.5 0.5 99 19 re\nS\n"),
            "{field_type:?}: {c}"
        );

        let f = field(&mut m, field_type, value, Vec::new());
        let c = drawn(&mut m, &f);
        assert!(!c.contains("0 0 100 20 re\nf"), "{field_type:?}: {c}");
        assert!(!c.contains("0.5 0.5 99 19 re\nS"), "{field_type:?}: {c}");
    }
}

#[test]
fn a_radio_button_fills_and_strokes_its_circle_with_mk() {
    let mut m = modifier();
    let f = field(
        &mut m,
        FieldType::RadioButton,
        name("Off"),
        vec![mk(Some(&[1.0, 0.0, 0.0]), Some(&[0.0, 0.0, 1.0])), bs(4.0)],
    );
    let c = drawn(&mut m, &f);
    // The circle is centred at (50, 10) with radius 10 - 4/2 = 8.
    assert!(c.starts_with("1 0 0 rg\n58 10 m\n"), "{c}");
    assert!(c.contains("f\n4 w\n0 0 1 RG\n58 10 m\n"), "{c}");

    let f = field(&mut m, FieldType::RadioButton, name("Off"), Vec::new());
    assert_eq!(drawn(&mut m, &f), "");
}

#[test]
fn filling_draws_each_widget_with_its_own_mk() {
    let mut m = modifier();
    let parent = m.add_object(PdfObject::Null);
    let rect = PdfObject::Array(
        [0, 0, 100, 20]
            .into_iter()
            .map(PdfObject::Integer)
            .collect(),
    );
    let widgets: Vec<IndirectRef> = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]
        .iter()
        .map(|bg| {
            let w = dict(vec![
                ("Type", name("Annot")),
                ("Subtype", name("Widget")),
                ("Parent", PdfObject::Reference(parent.clone())),
                ("Rect", rect.clone()),
                mk(Some(bg), None),
            ]);
            m.add_object(PdfObject::Dict(w))
        })
        .collect();
    m.set_object(
        parent.obj_num,
        PdfObject::Dict(dict(vec![
            ("FT", name("Tx")),
            ("T", text("name")),
            (
                "Kids",
                PdfObject::Array(widgets.iter().cloned().map(PdfObject::Reference).collect()),
            ),
        ])),
    );
    let catalog = m.catalog_ref();
    let Some(PdfObject::Dict(mut catalog_dict)) = m.find_object_pub(catalog.obj_num).cloned()
    else {
        panic!("catalog")
    };
    catalog_dict.insert(
        b"AcroForm".to_vec(),
        PdfObject::Dict(dict(vec![(
            "Fields",
            PdfObject::Array(vec![PdfObject::Reference(parent)]),
        )])),
    );
    m.set_object(catalog.obj_num, PdfObject::Dict(catalog_dict));
    let doc = PdfDocument::from_bytes(m.build().unwrap()).unwrap();

    let form = parse_acroform(&doc).unwrap().unwrap();
    let mut m = DocumentModifier::from_document(&doc).unwrap();
    set_field_value(&mut m, &form, "name", text("new")).unwrap();
    for (widget, colour) in widgets.iter().zip(["1 0 0 rg", "0 1 0 rg"]) {
        let Some(PdfObject::Dict(w)) = m.find_object_pub(widget.obj_num) else {
            panic!("widget")
        };
        let Some(PdfObject::Dict(ap)) = w.get(b"AP") else {
            panic!("/AP")
        };
        let Some(PdfObject::Reference(n)) = ap.get(b"N") else {
            panic!("/N")
        };
        let c = content(&m, n);
        assert!(c.starts_with(&format!("/Tx BMC\nq\n{colour}")), "{c}");
    }
}
