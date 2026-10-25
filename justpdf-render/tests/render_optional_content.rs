//! Optional content OFF under the document's default configuration is not
//! drawn: marked content, Form and image XObjects and annotations tagged with
//! `/OC`, through OCGs and OCMDs (#44).

mod common;

use common::{Obj, Plain, RED, Stream, WHITE, pdf, rgb};

const BLUE: [u8; 3] = [0, 0, 255];

/// A 100 × 100 page with `resources` and `content`. Object 5 is the OCG
/// `L`, ON or OFF in `/D` as `on` says; `extra` follows from object 6.
/// The page's `/Annots` is `annots` (empty for none).
fn doc_with(
    on: bool,
    resources: &str,
    content: &str,
    annots: &str,
    extra: Vec<Obj<'_>>,
) -> justpdf_core::PdfDocument {
    let state = if on { "/ON" } else { "/OFF" };
    let catalog = format!(
        "<< /Type /Catalog /Pages 2 0 R /OCProperties << /OCGs [5 0 R] /D << {state} [5 0 R] >> >> >>"
    );
    let page = format!(
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] /Resources {resources} \
         /Contents 4 0 R {annots} >>"
    );
    let mut objects = vec![
        Plain(&catalog),
        Plain("<< /Type /Pages /Kids [3 0 R] /Count 1 >>"),
        Plain(&page),
        Stream("", content),
        Plain("<< /Type /OCG /Name (L) >>"),
    ];
    objects.extend(extra);
    pdf(&objects)
}

const RED_FILL: &str = "1 0 0 rg 0 0 100 100 re f";

// ---- marked content -------------------------------------------------------------

#[test]
fn a_hidden_layer_named_in_page_properties_is_not_drawn() {
    let content = format!("/OC /L BDC {RED_FILL} EMC");
    let off = doc_with(
        false,
        "<< /Properties << /L 5 0 R >> >>",
        &content,
        "",
        vec![],
    );
    assert_eq!(rgb(&off, 50, 50), WHITE);
    let on = doc_with(
        true,
        "<< /Properties << /L 5 0 R >> >>",
        &content,
        "",
        vec![],
    );
    assert_eq!(rgb(&on, 50, 50), RED);
}

#[test]
fn a_name_in_a_forms_own_properties_is_resolved() {
    let form = format!("/OC /L BDC {RED_FILL} EMC");
    let doc = doc_with(
        false,
        "<< /XObject << /X0 6 0 R >> >>",
        "/X0 Do",
        "",
        vec![Stream(
            "/Type /XObject /Subtype /Form /BBox [0 0 100 100] \
             /Resources << /Properties << /L 5 0 R >> >>",
            &form,
        )],
    );
    assert_eq!(rgb(&doc, 50, 50), WHITE);
}

#[test]
fn nested_marked_content_does_not_end_the_hidden_section_early() {
    // Inside the hidden section a /Span BMC … EMC pair; after the section's
    // own EMC, a blue square in the lower-left quarter is drawn.
    let content = format!("/OC /L BDC /Span BMC EMC {RED_FILL} EMC 0 0 1 rg 0 0 50 50 re f");
    let doc = doc_with(
        false,
        "<< /Properties << /L 5 0 R >> >>",
        &content,
        "",
        vec![],
    );
    assert_eq!(rgb(&doc, 75, 25), WHITE);
    assert_eq!(rgb(&doc, 25, 75), BLUE);
}

#[test]
fn an_ocmd_follows_its_policy() {
    // /L is OFF: AllOff makes the content visible, AnyOn hides it.
    let content = format!("/OC /M BDC {RED_FILL} EMC");
    let with_policy = |p: &str| {
        let ocmd = format!("<< /Type /OCMD /OCGs [5 0 R] /P /{p} >>");
        let ocmd: &'static str = Box::leak(ocmd.into_boxed_str());
        doc_with(
            false,
            "<< /Properties << /M 6 0 R >> >>",
            &content,
            "",
            vec![Plain(ocmd)],
        )
    };
    assert_eq!(rgb(&with_policy("AllOff"), 50, 50), RED);
    assert_eq!(rgb(&with_policy("AnyOn"), 50, 50), WHITE);
}

#[test]
fn a_hidden_section_left_open_in_a_form_stays_in_the_form() {
    // The form opens a hidden section and never closes it; the page's fill
    // after the Do is drawn.
    let doc = doc_with(
        false,
        "<< /XObject << /X0 6 0 R >> >>",
        &format!("/X0 Do {RED_FILL}"),
        "",
        vec![Stream(
            "/Type /XObject /Subtype /Form /BBox [0 0 100 100] \
             /Resources << /Properties << /L 5 0 R >> >>",
            "/OC /L BDC",
        )],
    );
    assert_eq!(rgb(&doc, 50, 50), RED);
}

// ---- XObjects and annotations -----------------------------------------------------

#[test]
fn a_form_xobject_in_a_hidden_layer_is_not_drawn() {
    let doc = doc_with(
        false,
        "<< /XObject << /X0 6 0 R >> >>",
        "/X0 Do",
        "",
        vec![Stream(
            "/Type /XObject /Subtype /Form /BBox [0 0 100 100] /OC 5 0 R",
            RED_FILL,
        )],
    );
    assert_eq!(rgb(&doc, 50, 50), WHITE);
}

#[test]
fn an_image_xobject_in_a_hidden_layer_is_not_drawn() {
    let doc = doc_with(
        false,
        "<< /XObject << /Im0 6 0 R >> >>",
        "q 100 0 0 100 0 0 cm /Im0 Do Q",
        "",
        vec![Stream(
            "/Type /XObject /Subtype /Image /Width 1 /Height 1 /ColorSpace /DeviceRGB \
             /BitsPerComponent 8 /OC 5 0 R /Filter /ASCIIHexDecode",
            "FF0000>",
        )],
    );
    assert_eq!(rgb(&doc, 50, 50), WHITE);
}

#[test]
fn an_annotation_in_a_hidden_layer_is_not_drawn() {
    let doc = doc_with(
        false,
        "<< >>",
        "",
        "/Annots [6 0 R]",
        vec![
            Plain(
                "<< /Type /Annot /Subtype /Square /Rect [0 0 100 100] /AP << /N 7 0 R >> /OC 5 0 R >>",
            ),
            Stream(
                "/Type /XObject /Subtype /Form /BBox [0 0 100 100]",
                RED_FILL,
            ),
        ],
    );
    assert_eq!(rgb(&doc, 50, 50), WHITE);
}

#[test]
fn a_hidden_section_left_open_by_the_page_does_not_hide_its_annotations() {
    // The page opens a hidden section and never closes it; the annotation,
    // in no layer, is still drawn.
    let doc = doc_with(
        false,
        "<< /Properties << /L 5 0 R >> >>",
        "/OC /L BDC",
        "/Annots [6 0 R]",
        vec![
            Plain("<< /Type /Annot /Subtype /Square /Rect [0 0 100 100] /AP << /N 7 0 R >> >>"),
            Stream(
                "/Type /XObject /Subtype /Form /BBox [0 0 100 100]",
                RED_FILL,
            ),
        ],
    );
    assert_eq!(rgb(&doc, 50, 50), RED);
}

// ---- SVG (#218) -------------------------------------------------------------------

const SVG_RED: &str = "fill=\"rgb(255,0,0)\"";

fn svg(doc: &justpdf_core::PdfDocument) -> String {
    justpdf_render::render_page_to_svg(doc, 0).unwrap()
}

fn svg_has_red(doc: &justpdf_core::PdfDocument) -> bool {
    svg(doc).contains(SVG_RED)
}

/// Each fixture with whether its red fill is visible: the raster and SVG
/// renderers agree on it.
fn fixtures() -> Vec<(&'static str, justpdf_core::PdfDocument, bool)> {
    let props = "<< /Properties << /L 5 0 R >> >>";
    let section = format!("/OC /L BDC {RED_FILL} EMC");
    let ocmd = |p: &str| {
        let ocmd: &'static str =
            Box::leak(format!("<< /Type /OCMD /OCGs [5 0 R] /P /{p} >>").into_boxed_str());
        doc_with(
            false,
            "<< /Properties << /M 6 0 R >> >>",
            &format!("/OC /M BDC {RED_FILL} EMC"),
            "",
            vec![Plain(ocmd)],
        )
    };
    vec![
        (
            "section OFF",
            doc_with(false, props, &section, "", vec![]),
            false,
        ),
        (
            "section ON",
            doc_with(true, props, &section, "", vec![]),
            true,
        ),
        (
            "name in the form's own /Properties",
            doc_with(
                false,
                "<< /XObject << /X0 6 0 R >> >>",
                "/X0 Do",
                "",
                vec![Stream(
                    "/Type /XObject /Subtype /Form /BBox [0 0 100 100] \
                     /Resources << /Properties << /L 5 0 R >> >>",
                    &section,
                )],
            ),
            false,
        ),
        (
            "form /OC OFF",
            doc_with(
                false,
                "<< /XObject << /X0 6 0 R >> >>",
                "/X0 Do",
                "",
                vec![Stream(
                    "/Type /XObject /Subtype /Form /BBox [0 0 100 100] /OC 5 0 R",
                    RED_FILL,
                )],
            ),
            false,
        ),
        ("OCMD AllOff", ocmd("AllOff"), true),
        ("OCMD AnyOn", ocmd("AnyOn"), false),
        (
            "OCMD /VE [/Not 5 0 R]",
            doc_with(
                false,
                "<< /Properties << /M 6 0 R >> >>",
                &format!("/OC /M BDC {RED_FILL} EMC"),
                "",
                vec![Plain("<< /Type /OCMD /VE [/Not 5 0 R] >>")],
            ),
            true,
        ),
        (
            "form drawn inside a hidden section",
            doc_with(
                false,
                "<< /Properties << /L 5 0 R >> /XObject << /X0 6 0 R >> >>",
                "/OC /L BDC /X0 Do EMC",
                "",
                vec![Stream(
                    "/Type /XObject /Subtype /Form /BBox [0 0 100 100]",
                    RED_FILL,
                )],
            ),
            false,
        ),
        (
            "hidden section left open in a form",
            doc_with(
                false,
                "<< /XObject << /X0 6 0 R >> >>",
                &format!("/X0 Do {RED_FILL}"),
                "",
                vec![Stream(
                    "/Type /XObject /Subtype /Form /BBox [0 0 100 100] \
                     /Resources << /Properties << /L 5 0 R >> >>",
                    "/OC /L BDC",
                )],
            ),
            true,
        ),
    ]
}

#[test]
fn svg_hides_what_is_hidden() {
    for (name, doc, visible) in fixtures() {
        assert_eq!(svg_has_red(&doc), visible, "{name}: {}", svg(&doc));
    }
}

#[test]
fn svg_and_raster_hide_the_same_content() {
    for (name, doc, _) in fixtures() {
        let raster = rgb(&doc, 50, 50) == RED;
        assert_eq!(svg_has_red(&doc), raster, "{name}");
    }
}

#[test]
fn svg_an_on_section_nested_in_an_off_section_stays_hidden() {
    // /On (object 6) is not in /D /OFF, so it is ON. Inside the OFF /L
    // section: an /On section with red, then a red fill; after /L's EMC, blue.
    let content =
        format!("/OC /L BDC /OC /On BDC {RED_FILL} EMC {RED_FILL} EMC 0 0 1 rg 0 0 50 50 re f");
    let doc = doc_with(
        false,
        "<< /Properties << /L 5 0 R /On 6 0 R >> >>",
        &content,
        "",
        vec![Plain("<< /Type /OCG /Name (On) >>")],
    );
    let out = svg(&doc);
    assert!(!out.contains(SVG_RED), "{out}");
    assert!(out.contains("fill=\"rgb(0,0,255)\""), "{out}");
}

#[test]
fn svg_an_image_xobject_in_a_hidden_layer_is_not_drawn() {
    let image = |oc: &str| {
        let dict: &'static str = Box::leak(
            format!(
                "/Type /XObject /Subtype /Image /Width 1 /Height 1 /ColorSpace /DeviceRGB \
                 /BitsPerComponent 8 {oc} /Filter /ASCIIHexDecode"
            )
            .into_boxed_str(),
        );
        doc_with(
            false,
            "<< /XObject << /Im0 6 0 R >> >>",
            "q 100 0 0 100 0 0 cm /Im0 Do Q",
            "",
            vec![Stream(dict, "FF0000>")],
        )
    };
    assert!(!svg(&image("/OC 5 0 R")).contains("<image"));
    assert!(svg(&image("")).contains("<image"));
}

// ---- state inside hidden content --------------------------------------------------

/// ISO 32000-1 §8.11.3.1: inside hidden content nothing is drawn, but state
/// operators still apply and text still advances. Each case is a page with
/// `/L` OFF drawing `hidden` inside `/OC /L BDC … EMC` and then `after`; it
/// renders as `reference` followed by `after`, raster and SVG alike.
fn assert_hidden_like(hidden: &str, reference: &str, after: &str) {
    let resources = "<< /Properties << /L 5 0 R >> /Font << /F1 6 0 R >> >>";
    let font = || {
        vec![Plain(
            "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
        )]
    };
    let doc = doc_with(
        false,
        resources,
        &format!("/OC /L BDC {hidden} EMC {after}"),
        "",
        font(),
    );
    let expected = doc_with(
        false,
        resources,
        &format!("{reference} {after}"),
        "",
        font(),
    );
    let raster = |d: &justpdf_core::PdfDocument| {
        justpdf_render::render_page_to_pixmap(d, 0, &justpdf_render::RenderOptions::default())
            .unwrap()
            .data
    };
    assert!(raster(&doc) == raster(&expected), "raster: {hidden:?}");
    assert_eq!(svg(&doc), svg(&expected), "svg: {hidden:?}");
}

const BLUE_SQUARE: &str = "0 0 1 rg 0 0 10 10 re f";

#[test]
fn a_transform_inside_hidden_content_still_applies() {
    assert_hidden_like("1 0 0 1 50 0 cm", "1 0 0 1 50 0 cm", BLUE_SQUARE);
    assert_hidden_like(
        "q 1 0 0 1 50 0 cm",
        "q 1 0 0 1 50 0 cm",
        &format!("{BLUE_SQUARE} Q"),
    );
}

#[test]
fn a_colour_inside_hidden_content_still_applies() {
    assert_hidden_like("0 0 1 rg", "0 0 1 rg", "0 0 50 50 re f");
}

#[test]
fn a_clip_inside_hidden_content_still_applies() {
    assert_hidden_like("0 0 50 100 re W n", "0 0 50 100 re W n", RED_FILL);
}

#[test]
fn a_path_painted_inside_hidden_content_is_consumed_but_not_drawn() {
    assert_hidden_like("0 0 100 100 re f", "", BLUE_SQUARE);
    assert_hidden_like("0 0 100 100 re B", "", BLUE_SQUARE);
}

#[test]
fn hidden_text_still_advances() {
    assert_hidden_like(
        "BT /F1 24 Tf 10 50 Td (Hidden) Tj",
        "BT /F1 24 Tf 10 50 Td 3 Tr (Hidden) Tj 0 Tr",
        "(V) Tj ET",
    );
}

#[test]
fn a_shading_inside_hidden_content_is_not_drawn() {
    let resources = "<< /Properties << /L 5 0 R >> /Shading << /Sh0 << /ShadingType 2 \
                     /ColorSpace /DeviceRGB /Coords [0 0 100 0] /Function << /FunctionType 2 \
                     /Domain [0 1] /C0 [1 0 0] /C1 [1 0 0] /N 1 >> /Extend [true true] >> >> >>";
    let content = "/OC /L BDC /Sh0 sh EMC";
    assert_eq!(
        rgb(&doc_with(false, resources, content, "", vec![]), 50, 50),
        WHITE
    );
    assert_eq!(
        rgb(&doc_with(true, resources, content, "", vec![]), 50, 50),
        RED
    );
}
