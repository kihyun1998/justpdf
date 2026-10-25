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
