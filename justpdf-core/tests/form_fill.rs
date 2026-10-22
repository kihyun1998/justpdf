//! Filling form fields: `NeedAppearances`, checkbox on-states from `/AP`, and
//! `/AS` on each widget (#204).

use justpdf_core::form::{parse_acroform, set_field_value, toggle_checkbox};
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

fn string(s: &str) -> PdfObject {
    PdfObject::String(s.as_bytes().to_vec())
}

fn rect() -> PdfObject {
    PdfObject::Array([0, 0, 20, 20].into_iter().map(PdfObject::Integer).collect())
}

/// An `/AP` whose `/N` has the appearance states `states`, each an empty
/// stream added to `modifier`.
fn ap(modifier: &mut DocumentModifier, states: &[&str]) -> PdfObject {
    let mut normal = PdfDict::new();
    for state in states {
        let r = modifier.add_object(PdfObject::Stream {
            dict: dict(vec![("Length", PdfObject::Integer(0))]),
            data: Vec::new(),
        });
        normal.insert(state.as_bytes().to_vec(), PdfObject::Reference(r));
    }
    PdfObject::Dict(dict(vec![("N", PdfObject::Dict(normal))]))
}

/// A one-page document whose AcroForm `/Fields` are the objects `fields`
/// builds in the modifier; the AcroForm is an indirect object when
/// `indirect`.
fn form_document(
    indirect: bool,
    fields: impl FnOnce(&mut DocumentModifier) -> Vec<IndirectRef>,
) -> PdfDocument {
    let mut builder = DocumentBuilder::new();
    builder.add_page(PageBuilder::new(200.0, 100.0));
    let source = PdfDocument::from_bytes(builder.build().unwrap()).unwrap();
    let mut modifier = DocumentModifier::from_document(&source).unwrap();
    let refs = fields(&mut modifier);
    let acroform = dict(vec![
        (
            "Fields",
            PdfObject::Array(refs.into_iter().map(PdfObject::Reference).collect()),
        ),
        ("DA", string("/Helv 0 Tf 0 g")),
    ]);
    let acroform = if indirect {
        PdfObject::Reference(modifier.add_object(PdfObject::Dict(acroform)))
    } else {
        PdfObject::Dict(acroform)
    };
    let catalog = modifier.catalog_ref();
    let Some(PdfObject::Dict(mut catalog_dict)) =
        modifier.find_object_pub(catalog.obj_num).cloned()
    else {
        panic!("catalog")
    };
    catalog_dict.insert(b"AcroForm".to_vec(), acroform);
    modifier.set_object(catalog.obj_num, PdfObject::Dict(catalog_dict));
    PdfDocument::from_bytes(modifier.build().unwrap()).unwrap()
}

/// `doc` after `edit` runs on a modifier of it with its parsed AcroForm,
/// built and reopened.
fn filled(
    doc: &PdfDocument,
    edit: impl FnOnce(&mut DocumentModifier, &justpdf_core::form::AcroForm),
) -> PdfDocument {
    let form = parse_acroform(doc).unwrap().unwrap();
    let mut modifier = DocumentModifier::from_document(doc).unwrap();
    edit(&mut modifier, &form);
    PdfDocument::from_bytes(modifier.build().unwrap()).unwrap()
}

fn object(doc: &PdfDocument, r: &IndirectRef) -> PdfDict {
    match doc.resolve(r).unwrap() {
        PdfObject::Dict(d) => d,
        other => panic!("expected a dictionary, got {other:?}"),
    }
}

fn text_field(modifier: &mut DocumentModifier) -> Vec<IndirectRef> {
    vec![modifier.add_object(PdfObject::Dict(dict(vec![
        ("Type", name("Annot")),
        ("Subtype", name("Widget")),
        ("FT", name("Tx")),
        ("T", string("name")),
        ("V", string("old")),
        ("Rect", rect()),
    ])))]
}

#[test]
fn setting_a_value_sets_need_appearances_and_keeps_the_acroform() {
    for indirect in [false, true] {
        let doc = form_document(indirect, text_field);
        assert!(!parse_acroform(&doc).unwrap().unwrap().need_appearances);
        let out = filled(&doc, |modifier, form| {
            set_field_value(modifier, form, "name", string("new")).unwrap();
        });
        let form = parse_acroform(&out).unwrap().unwrap();
        assert!(form.need_appearances, "indirect: {indirect}");
        assert_eq!(form.default_appearance.as_deref(), Some("/Helv 0 Tf 0 g"));
        assert_eq!(form.fields[0].value, Some(string("new")));
    }
}

#[test]
fn a_checkbox_turns_on_with_the_state_its_appearance_has() {
    let mut widget = None;
    let doc = form_document(false, |modifier| {
        let ap = ap(modifier, &["On", "Off"]);
        let r = modifier.add_object(PdfObject::Dict(dict(vec![
            ("Type", name("Annot")),
            ("Subtype", name("Widget")),
            ("FT", name("Btn")),
            ("T", string("agree")),
            ("V", name("Off")),
            ("AS", name("Off")),
            ("AP", ap),
            ("Rect", rect()),
        ])));
        widget = Some(r.clone());
        vec![r]
    });
    let widget = widget.unwrap();

    let on = filled(&doc, |modifier, form| {
        toggle_checkbox(modifier, form, "agree").unwrap();
    });
    let d = object(&on, &widget);
    assert_eq!(d.get(b"V"), Some(&name("On")));
    assert_eq!(d.get(b"AS"), Some(&name("On")));

    let off = filled(&on, |modifier, form| {
        toggle_checkbox(modifier, form, "agree").unwrap();
    });
    let d = object(&off, &widget);
    assert_eq!(d.get(b"V"), Some(&name("Off")));
    assert_eq!(d.get(b"AS"), Some(&name("Off")));
}

/// A field `t` of type `/Btn` with flags `ff` and `/V /Off`, whose kid
/// widgets have the on-states `states`: (the field, its widgets).
fn button_group(
    modifier: &mut DocumentModifier,
    t: &str,
    ff: i64,
    states: &[&str],
) -> (IndirectRef, Vec<IndirectRef>) {
    let parent = modifier.add_object(PdfObject::Null);
    let widgets: Vec<IndirectRef> = states
        .iter()
        .map(|state| {
            let ap = ap(modifier, &[state, "Off"]);
            modifier.add_object(PdfObject::Dict(dict(vec![
                ("Type", name("Annot")),
                ("Subtype", name("Widget")),
                ("Parent", PdfObject::Reference(parent.clone())),
                ("AS", name("Off")),
                ("AP", ap),
                ("Rect", rect()),
            ])))
        })
        .collect();
    modifier.set_object(
        parent.obj_num,
        PdfObject::Dict(dict(vec![
            ("FT", name("Btn")),
            ("Ff", PdfObject::Integer(ff)),
            ("T", string(t)),
            ("V", name("Off")),
            (
                "Kids",
                PdfObject::Array(widgets.iter().cloned().map(PdfObject::Reference).collect()),
            ),
        ])),
    );
    (parent, widgets)
}

const RADIO: i64 = 1 << 15;

#[test]
fn a_radio_group_is_one_field_whose_value_sets_each_widgets_state() {
    let mut refs = None;
    let doc = form_document(false, |modifier| {
        let (parent, widgets) = button_group(modifier, "choice", RADIO, &["A", "B"]);
        refs = Some((parent.clone(), widgets));
        vec![parent]
    });
    let (parent, widgets) = refs.unwrap();

    let form = parse_acroform(&doc).unwrap().unwrap();
    let group: Vec<_> = form.fields.iter().filter(|f| f.name == "choice").collect();
    assert_eq!(group.len(), 1);
    assert_eq!(group[0].field_ref, parent);
    assert_eq!(group[0].widgets, widgets);
    assert_eq!(group[0].value, Some(name("Off")));

    let out = filled(&doc, |modifier, form| {
        set_field_value(modifier, form, "choice", name("B")).unwrap();
    });
    let field = object(&out, &parent);
    assert_eq!(field.get(b"V"), Some(&name("B")));
    assert_eq!(field.get(b"AS"), None);
    assert_eq!(object(&out, &widgets[0]).get(b"AS"), Some(&name("Off")));
    assert_eq!(object(&out, &widgets[1]).get(b"AS"), Some(&name("B")));
    assert_eq!(
        parse_acroform(&out).unwrap().unwrap().fields[0].value,
        Some(name("B"))
    );
}

#[test]
fn a_checkbox_with_kid_widgets_sets_each_widgets_state() {
    let mut refs = None;
    let doc = form_document(false, |modifier| {
        let (parent, widgets) = button_group(modifier, "copies", 0, &["Yes1", "Yes1"]);
        refs = Some((parent.clone(), widgets));
        vec![parent]
    });
    let (parent, widgets) = refs.unwrap();
    let out = filled(&doc, |modifier, form| {
        toggle_checkbox(modifier, form, "copies").unwrap();
    });
    assert_eq!(object(&out, &parent).get(b"V"), Some(&name("Yes1")));
    assert_eq!(object(&out, &parent).get(b"AS"), None);
    for widget in &widgets {
        assert_eq!(object(&out, widget).get(b"AS"), Some(&name("Yes1")));
    }
}

#[test]
fn a_merged_text_field_is_its_own_widget() {
    let doc = form_document(false, text_field);
    let form = parse_acroform(&doc).unwrap().unwrap();
    assert_eq!(form.fields.len(), 1);
    assert_eq!(
        form.fields[0].widgets,
        vec![form.fields[0].field_ref.clone()]
    );
}

#[test]
fn a_checkbox_with_only_a_down_appearance_takes_its_state_from_it() {
    let mut widget = None;
    let doc = form_document(false, |modifier| {
        let PdfObject::Dict(normal) = ap(modifier, &["Check", "Off"]) else {
            unreachable!()
        };
        let down = normal.get(b"N").unwrap().clone();
        let r = modifier.add_object(PdfObject::Dict(dict(vec![
            ("Type", name("Annot")),
            ("Subtype", name("Widget")),
            ("FT", name("Btn")),
            ("T", string("agree")),
            ("V", name("Off")),
            ("AP", PdfObject::Dict(dict(vec![("D", down)]))),
            ("Rect", rect()),
        ])));
        widget = Some(r.clone());
        vec![r]
    });
    let out = filled(&doc, |modifier, form| {
        toggle_checkbox(modifier, form, "agree").unwrap();
    });
    assert_eq!(
        object(&out, &widget.unwrap()).get(b"AS"),
        Some(&name("Check"))
    );
}
