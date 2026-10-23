//! Filling form fields: `NeedAppearances`, checkbox on-states from `/AP`,
//! `/AS` on each widget (#204), and each widget's normal appearance drawn
//! from the new value (#344) with its own `/DA` (#347).

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
    form_document_with(indirect, Vec::new(), fields)
}

/// [`form_document`] whose AcroForm also holds `entries`.
fn form_document_with(
    indirect: bool,
    entries: Vec<(&str, PdfObject)>,
    fields: impl FnOnce(&mut DocumentModifier) -> Vec<IndirectRef>,
) -> PdfDocument {
    let mut builder = DocumentBuilder::new();
    builder.add_page(PageBuilder::new(200.0, 100.0));
    let source = PdfDocument::from_bytes(builder.build().unwrap()).unwrap();
    let mut modifier = DocumentModifier::from_document(&source).unwrap();
    let refs = fields(&mut modifier);
    let mut acroform = dict(vec![
        (
            "Fields",
            PdfObject::Array(refs.into_iter().map(PdfObject::Reference).collect()),
        ),
        ("DA", string("/Helv 0 Tf 0 g")),
    ]);
    for (k, v) in entries {
        acroform.insert(k.as_bytes().to_vec(), v);
    }
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

/// An AcroForm `/DR` whose `/Helv` is a WinAnsi Helvetica.
fn dr() -> (&'static str, PdfObject) {
    let helv = PdfObject::Dict(dict(vec![
        ("Type", name("Font")),
        ("Subtype", name("Type1")),
        ("BaseFont", name("Helvetica")),
        ("Encoding", name("WinAnsiEncoding")),
    ]));
    let fonts = PdfObject::Dict(dict(vec![("Helv", helv)]));
    ("DR", PdfObject::Dict(dict(vec![("Font", fonts)])))
}

fn rect_of(w: i64, h: i64) -> PdfObject {
    PdfObject::Array([0, 0, w, h].into_iter().map(PdfObject::Integer).collect())
}

/// An `/AP` with an empty `/N`, `/D` and `/R` stream each, added to
/// `modifier`.
fn old_ap(modifier: &mut DocumentModifier) -> PdfObject {
    let mut entries = PdfDict::new();
    for key in ["N", "D", "R"] {
        let r = modifier.add_object(PdfObject::Stream {
            dict: dict(vec![("Length", PdfObject::Integer(0))]),
            data: Vec::new(),
        });
        entries.insert(key.as_bytes().to_vec(), PdfObject::Reference(r));
    }
    PdfObject::Dict(entries)
}

/// A widget dictionary with `entries`, an old `/AP` and `/Rect` `rect`
/// (none when `rect` is `None`).
fn widget(
    modifier: &mut DocumentModifier,
    entries: Vec<(&str, PdfObject)>,
    rect: Option<PdfObject>,
) -> PdfDict {
    let ap = old_ap(modifier);
    let mut d = dict(vec![
        ("Type", name("Annot")),
        ("Subtype", name("Widget")),
        ("AP", ap),
    ]);
    if let Some(rect) = rect {
        d.insert(b"Rect".to_vec(), rect);
    }
    for (k, v) in entries {
        d.insert(k.as_bytes().to_vec(), v);
    }
    d
}

/// A text field `name`, `/V (old)`, `/DA (/Helv 12 Tf 0 g)`, that is its own
/// widget with `/Rect` `rect`, in a document whose AcroForm has [`dr`].
fn dr_text_field(rect: Option<PdfObject>) -> (PdfDocument, IndirectRef) {
    let mut refs = None;
    let doc = form_document_with(false, vec![dr()], |modifier| {
        let d = widget(
            modifier,
            vec![
                ("FT", name("Tx")),
                ("T", string("name")),
                ("V", string("old")),
                ("DA", string("/Helv 12 Tf 0 g")),
            ],
            rect,
        );
        let r = modifier.add_object(PdfObject::Dict(d));
        refs = Some(r.clone());
        vec![r]
    });
    (doc, refs.unwrap())
}

/// The `/AP` of `widget` in `doc`, resolved.
fn ap_of(doc: &PdfDocument, widget: &IndirectRef) -> PdfDict {
    match object(doc, widget).get(b"AP").cloned() {
        Some(PdfObject::Reference(r)) => object(doc, &r),
        Some(PdfObject::Dict(d)) => d,
        other => panic!("expected an /AP, got {other:?}"),
    }
}

/// The normal appearance of `widget` in `doc`: (the strings it shows with
/// `Tj`, its `/BBox`).
fn normal_appearance(doc: &PdfDocument, widget: &IndirectRef) -> (Vec<Vec<u8>>, PdfObject) {
    let (content, bbox) = normal_content(doc, widget);
    let shown = justpdf_core::content::parse_content_stream(&content)
        .unwrap()
        .into_iter()
        .filter(|op| op.operator == b"Tj")
        .flat_map(|op| op.operands)
        .filter_map(|o| match o {
            justpdf_core::content::Operand::String(s) => Some(s),
            _ => None,
        })
        .collect();
    (shown, bbox)
}

/// The normal appearance of `widget` in `doc`: (its decoded content, its
/// `/BBox`).
fn normal_content(doc: &PdfDocument, widget: &IndirectRef) -> (Vec<u8>, PdfObject) {
    let n = ap_of(doc, widget).get(b"N").cloned().unwrap();
    let PdfObject::Reference(n) = n else {
        panic!("expected /N to be a reference, got {n:?}")
    };
    let PdfObject::Stream { dict, data } = doc.resolve(&n).unwrap() else {
        panic!("expected /N to be a stream")
    };
    let content = justpdf_core::stream::decode_stream(&data, &dict).unwrap();
    (content, dict.get(b"BBox").cloned().unwrap())
}

fn bbox(w: f64, h: f64) -> PdfObject {
    PdfObject::Array([0.0, 0.0, w, h].into_iter().map(PdfObject::Real).collect())
}

#[test]
fn filling_a_text_field_draws_its_new_value_as_the_only_appearance() {
    let (doc, widget) = dr_text_field(Some(rect_of(100, 20)));
    let old = ap_of(&doc, &widget);

    let out = filled(&doc, |modifier, form| {
        set_field_value(modifier, form, "name", string("new")).unwrap();
    });
    let ap = ap_of(&out, &widget);
    assert_ne!(ap.get(b"N"), old.get(b"N"));
    assert_eq!(ap.get(b"D"), None);
    assert_eq!(ap.get(b"R"), None);
    assert_eq!(normal_appearance(&out, &widget).0, vec![b"new".to_vec()]);
    assert!(parse_acroform(&out).unwrap().unwrap().need_appearances);
}

#[test]
fn each_widget_of_a_field_is_drawn_in_its_own_rect() {
    let mut refs = Vec::new();
    let doc = form_document_with(false, vec![dr()], |modifier| {
        let parent = modifier.add_object(PdfObject::Null);
        for (w, h) in [(100, 20), (60, 30)] {
            let d = widget(
                modifier,
                vec![("Parent", PdfObject::Reference(parent.clone()))],
                Some(rect_of(w, h)),
            );
            refs.push(modifier.add_object(PdfObject::Dict(d)));
        }
        modifier.set_object(
            parent.obj_num,
            PdfObject::Dict(dict(vec![
                ("FT", name("Tx")),
                ("T", string("name")),
                ("V", string("old")),
                ("DA", string("/Helv 12 Tf 0 g")),
                (
                    "Kids",
                    PdfObject::Array(refs.iter().cloned().map(PdfObject::Reference).collect()),
                ),
            ])),
        );
        vec![parent]
    });

    let out = filled(&doc, |modifier, form| {
        set_field_value(modifier, form, "name", string("new")).unwrap();
    });
    for (widget, (w, h)) in refs.iter().zip([(100.0, 20.0), (60.0, 30.0)]) {
        let (shown, b) = normal_appearance(&out, widget);
        assert_eq!(shown, vec![b"new".to_vec()]);
        assert_eq!(b, bbox(w, h));
    }
}

const COMBO: i64 = 1 << 17;

#[test]
fn combo_and_list_boxes_are_drawn_from_their_new_value() {
    for ff in [0, COMBO] {
        let mut refs = None;
        let doc = form_document_with(false, vec![dr()], |modifier| {
            let d = widget(
                modifier,
                vec![
                    ("FT", name("Ch")),
                    ("Ff", PdfObject::Integer(ff)),
                    ("T", string("pick")),
                    ("V", string("a")),
                    ("Opt", PdfObject::Array(vec![string("a"), string("b")])),
                    ("DA", string("/Helv 12 Tf 0 g")),
                ],
                Some(rect_of(100, 40)),
            );
            let r = modifier.add_object(PdfObject::Dict(d));
            refs = Some(r.clone());
            vec![r]
        });
        let widget = refs.unwrap();
        let old = ap_of(&doc, &widget);

        let out = filled(&doc, |modifier, form| {
            set_field_value(modifier, form, "pick", string("b")).unwrap();
        });
        let ap = ap_of(&out, &widget);
        assert_ne!(ap.get(b"N"), old.get(b"N"), "Ff {ff}");
        assert_eq!(ap.get(b"D"), None, "Ff {ff}");
        let shown = normal_appearance(&out, &widget).0;
        let expected: Vec<Vec<u8>> = if ff == COMBO {
            vec![b"b".to_vec()]
        } else {
            vec![b"a".to_vec(), b"b".to_vec()]
        };
        assert_eq!(shown, expected, "Ff {ff}");
        if ff == 0 {
            // The second of 12 pt rows in a 40 pt box is highlighted.
            let content = String::from_utf8(normal_content(&out, &widget).0).unwrap();
            assert!(content.contains("rg\n1 16 98 12 re"), "{content}");
        }
    }
}

#[test]
fn a_widget_that_cannot_be_drawn_keeps_its_appearance() {
    // `한글` is not in WinAnsi Helvetica; a widget with no `/Rect` has no area.
    for (value, rect) in [("한글", Some(rect_of(100, 20))), ("new", None)] {
        let (doc, widget) = dr_text_field(rect);
        let old = ap_of(&doc, &widget);

        let out = filled(&doc, |modifier, form| {
            set_field_value(modifier, form, "name", string(value)).unwrap();
        });
        assert_eq!(ap_of(&out, &widget), old, "{value}");
        assert!(parse_acroform(&out).unwrap().unwrap().need_appearances);
    }
}

#[test]
fn an_indirect_appearance_dictionary_is_left_for_the_widgets_sharing_it() {
    let mut refs = None;
    let doc = form_document_with(false, vec![dr()], |modifier| {
        let ap = old_ap(modifier);
        let shared = modifier.add_object(ap);
        let mut d = widget(
            modifier,
            vec![
                ("FT", name("Tx")),
                ("T", string("name")),
                ("V", string("old")),
                ("DA", string("/Helv 12 Tf 0 g")),
            ],
            Some(rect_of(100, 20)),
        );
        d.insert(b"AP".to_vec(), PdfObject::Reference(shared.clone()));
        let r = modifier.add_object(PdfObject::Dict(d));
        refs = Some((r.clone(), shared));
        vec![r]
    });
    let (widget, shared) = refs.unwrap();
    let old = object(&doc, &shared);

    let out = filled(&doc, |modifier, form| {
        set_field_value(modifier, form, "name", string("new")).unwrap();
    });
    assert_eq!(object(&out, &shared), old);
    assert_eq!(normal_appearance(&out, &widget).0, vec![b"new".to_vec()]);
    assert_eq!(ap_of(&out, &widget).get(b"D"), None);
}

#[test]
fn filling_a_button_keeps_its_appearances() {
    for ff in [0, RADIO] {
        let mut refs = None;
        let doc = form_document_with(false, vec![dr()], |modifier| {
            let ap = ap(modifier, &["On", "Off"]);
            let r = modifier.add_object(PdfObject::Dict(dict(vec![
                ("Type", name("Annot")),
                ("Subtype", name("Widget")),
                ("FT", name("Btn")),
                ("Ff", PdfObject::Integer(ff)),
                ("T", string("agree")),
                ("V", name("Off")),
                ("AS", name("Off")),
                ("AP", ap),
                ("Rect", rect()),
            ])));
            refs = Some(r.clone());
            vec![r]
        });
        let widget = refs.unwrap();
        let old = ap_of(&doc, &widget);

        let out = filled(&doc, |modifier, form| {
            set_field_value(modifier, form, "agree", name("On")).unwrap();
        });
        assert_eq!(ap_of(&out, &widget), old, "Ff {ff}");
    }
}

/// A text field `name`, `/V (old)`, `/DA (/Helv 12 Tf 0 g)`, whose kid widgets
/// have the `/DA` of `das` (none for `None`), in a document whose AcroForm has
/// [`dr`]: (the document, the widgets).
fn kids_with_da(das: &[Option<&str>]) -> (PdfDocument, Vec<IndirectRef>) {
    let mut refs = Vec::new();
    let doc = form_document_with(false, vec![dr()], |modifier| {
        let parent = modifier.add_object(PdfObject::Null);
        for da in das {
            let mut entries = vec![("Parent", PdfObject::Reference(parent.clone()))];
            if let Some(da) = da {
                entries.push(("DA", string(da)));
            }
            let d = widget(modifier, entries, Some(rect_of(100, 30)));
            refs.push(modifier.add_object(PdfObject::Dict(d)));
        }
        modifier.set_object(
            parent.obj_num,
            PdfObject::Dict(dict(vec![
                ("FT", name("Tx")),
                ("T", string("name")),
                ("V", string("old")),
                ("DA", string("/Helv 12 Tf 0 g")),
                (
                    "Kids",
                    PdfObject::Array(refs.iter().cloned().map(PdfObject::Reference).collect()),
                ),
            ])),
        );
        vec![parent]
    });
    (doc, refs)
}

/// The lines of `widget`'s normal appearance in `doc` that select a font.
fn font_selection(doc: &PdfDocument, widget: &IndirectRef) -> Vec<String> {
    String::from_utf8(normal_content(doc, widget).0)
        .unwrap()
        .lines()
        .filter(|l| l.contains("Tf"))
        .map(str::to_string)
        .collect()
}

#[test]
fn a_widget_with_its_own_da_is_drawn_with_it() {
    let (doc, widgets) = kids_with_da(&[Some("/Helv 20 Tf 1 0 0 rg"), None]);
    let out = filled(&doc, |modifier, form| {
        set_field_value(modifier, form, "name", string("new")).unwrap();
    });
    assert_eq!(font_selection(&out, &widgets[0]), ["/Helv 20 Tf 1 0 0 rg"]);
    assert_eq!(font_selection(&out, &widgets[1]), ["/Helv 12 Tf 0 g"]);
}

#[test]
fn a_widget_whose_own_da_font_is_not_in_dr_keeps_its_appearance() {
    let (doc, widgets) = kids_with_da(&[Some("/Nope 12 Tf 0 g"), None]);
    let old = ap_of(&doc, &widgets[0]);
    let out = filled(&doc, |modifier, form| {
        set_field_value(modifier, form, "name", string("new")).unwrap();
    });
    assert_eq!(ap_of(&out, &widgets[0]), old);
    assert_eq!(
        normal_appearance(&out, &widgets[1]).0,
        vec![b"new".to_vec()]
    );
}

const MULTI_SELECT: i64 = 1 << 21;

/// A list box `pick` (100 × 40, `/DA (/Helv 12 Tf 0 g)`) with `/Opt` `opt`
/// and the extra `entries`, in a document whose AcroForm has [`dr`].
fn list_box(opt: PdfObject, entries: Vec<(&str, PdfObject)>) -> (PdfDocument, IndirectRef) {
    let mut refs = None;
    let doc = form_document_with(false, vec![dr()], |modifier| {
        let mut all = vec![
            ("FT", name("Ch")),
            ("T", string("pick")),
            ("Opt", opt),
            ("DA", string("/Helv 12 Tf 0 g")),
        ];
        all.extend(entries);
        let d = widget(modifier, all, Some(rect_of(100, 40)));
        let r = modifier.add_object(PdfObject::Dict(d));
        refs = Some(r.clone());
        vec![r]
    });
    (doc, refs.unwrap())
}

fn strings(values: &[&str]) -> PdfObject {
    PdfObject::Array(values.iter().map(|v| string(v)).collect())
}

/// The highlight rectangle of row `row` (from the top) in a 100 × 40 list box.
fn highlight(row: i64) -> String {
    format!("1 {} 98 12 re", 40 - 12 * (row + 1))
}

#[test]
fn a_multiple_selection_highlights_every_selected_row() {
    let (doc, widget) = list_box(
        strings(&["a", "b", "c"]),
        vec![("Ff", PdfObject::Integer(MULTI_SELECT))],
    );
    let out = filled(&doc, |modifier, form| {
        set_field_value(modifier, form, "pick", strings(&["a", "c"])).unwrap();
    });
    let content = String::from_utf8(normal_content(&out, &widget).0).unwrap();
    assert!(content.contains(&highlight(0)), "{content}");
    assert!(!content.contains(&highlight(1)), "{content}");
    assert!(content.contains(&highlight(2)), "{content}");
}

#[test]
fn an_export_value_selects_its_option_and_the_display_text_is_drawn() {
    let opt = PdfObject::Array(vec![strings(&["x", "Ex"]), strings(&["y", "Why"])]);
    let (doc, widget) = list_box(opt, Vec::new());
    let out = filled(&doc, |modifier, form| {
        set_field_value(modifier, form, "pick", string("y")).unwrap();
    });
    let (shown, _) = normal_appearance(&out, &widget);
    assert_eq!(shown, vec![b"Ex".to_vec(), b"Why".to_vec()]);
    let content = String::from_utf8(normal_content(&out, &widget).0).unwrap();
    assert!(content.contains(&highlight(1)), "{content}");
    assert!(!content.contains(&highlight(0)), "{content}");
}

#[test]
fn filling_rewrites_an_existing_i_and_adds_none() {
    let indices = |v: &[i64]| PdfObject::Array(v.iter().map(|&i| PdfObject::Integer(i)).collect());
    let (doc, widget) = list_box(
        strings(&["a", "b", "c"]),
        vec![
            ("Ff", PdfObject::Integer(MULTI_SELECT)),
            ("I", indices(&[0])),
        ],
    );
    let out = filled(&doc, |modifier, form| {
        set_field_value(modifier, form, "pick", strings(&["b", "c"])).unwrap();
    });
    assert_eq!(object(&out, &widget).get(b"I"), Some(&indices(&[1, 2])));

    let (doc, widget) = list_box(strings(&["a", "b", "c"]), Vec::new());
    let out = filled(&doc, |modifier, form| {
        set_field_value(modifier, form, "pick", string("b")).unwrap();
    });
    assert_eq!(object(&out, &widget).get(b"I"), None);
}

#[test]
fn i_selects_before_v() {
    let (doc, _) = list_box(
        strings(&["a", "b", "c"]),
        vec![("I", PdfObject::Array(vec![PdfObject::Integer(2)]))],
    );
    let form = parse_acroform(&doc).unwrap().unwrap();
    assert_eq!(form.fields[0].selected_values(), ["c"]);

    let (doc, _) = list_box(
        strings(&["a", "b", "c"]),
        vec![
            ("V", string("a")),
            ("I", PdfObject::Array(vec![PdfObject::Integer(2)])),
        ],
    );
    let form = parse_acroform(&doc).unwrap().unwrap();
    assert_eq!(form.fields[0].selected_values(), ["c"]);
}
