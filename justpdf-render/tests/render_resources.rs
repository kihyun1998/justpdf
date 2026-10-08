//! Resource names inside nested content streams: Form XObjects, pattern
//! tiles, soft mask forms and annotation appearances resolve names in their
//! own `/Resources`, then in each enclosing stream's, then the page's (#127).

mod common;

use common::{Plain, RED, Stream, WHITE, page, pdf, rgb};
use justpdf_render::render_page_to_svg;

const BLACK: [u8; 3] = [0, 0, 0];

/// A form XObject dictionary with `resources`.
fn form(resources: &str) -> String {
    format!("/Type /XObject /Subtype /Form /BBox [0 0 100 100] /Resources {resources}")
}

/// A form XObject dictionary without `/Resources`.
const BARE_FORM: &str = "/Type /XObject /Subtype /Form /BBox [0 0 100 100]";

const FILL_RED: &str = "1 0 0 rg 0 0 100 100 re f";
const FILL_BLUE: &str = "0 0 1 rg 0 0 100 100 re f";

/// A Type3 font with no `/CharProcs` named `base`, whose one glyph (`A`) is
/// `width` thousandths of an em wide. Its glyph is drawn as a filled
/// rectangle that wide.
fn font(base: &str, width: u32) -> String {
    format!(
        "<< /Type /Font /Subtype /Type3 /BaseFont /{base} /FontBBox [0 0 1000 1000] \
         /FontMatrix [0.001 0 0 0.001 0 0] /CharProcs << >> \
         /Encoding << /Differences [65 /A] >> /FirstChar 65 /LastChar 65 /Widths [{width}] >>"
    )
}

/// Shows `A` at 50 pt from (10, 40) in red. A 1000-wide glyph covers
/// (50, 50); a 100-wide one ends at x = 15.
const SHOW_A: &str = "BT 1 0 0 rg 10 40 Td (A) Tj ET";

// ---- Form XObjects -------------------------------------------------------

/// Form 5 draws `/Inner`, which only its own `/Resources` names.
#[test]
fn form_draws_a_name_only_its_resources_hold() {
    let doc = page(
        "<< /XObject << /Outer 5 0 R >> >>",
        "/Outer Do",
        vec![
            Stream(&form("<< /XObject << /Inner 6 0 R >> >>"), "/Inner Do"),
            Stream(&form("<< >>"), FILL_RED),
        ],
    );
    assert_eq!(rgb(&doc, 50, 50), RED);
}

/// `/Inner` names a blue form on the page and a red one in form 5; form 5
/// draws its own.
#[test]
fn form_name_wins_over_the_same_page_name() {
    let doc = page(
        "<< /XObject << /Outer 5 0 R /Inner 7 0 R >> >>",
        "/Outer Do",
        vec![
            Stream(&form("<< /XObject << /Inner 6 0 R >> >>"), "/Inner Do"),
            Stream(&form("<< >>"), FILL_RED),
            Stream(&form("<< >>"), FILL_BLUE),
        ],
    );
    assert_eq!(rgb(&doc, 50, 50), RED);
}

/// Form 6 has no `/Resources` and draws `/Inner`, which form 5 (its caller)
/// names red and the page names blue; the caller's is drawn.
#[test]
fn form_without_resources_resolves_in_its_callers_scope() {
    let doc = page(
        "<< /XObject << /Outer 5 0 R /Inner 8 0 R >> >>",
        "/Outer Do",
        vec![
            Stream(
                &form("<< /XObject << /Mid 6 0 R /Inner 7 0 R >> >>"),
                "/Mid Do",
            ),
            Stream(BARE_FORM, "/Inner Do"),
            Stream(&form("<< >>"), FILL_RED),
            Stream(&form("<< >>"), FILL_BLUE),
        ],
    );
    assert_eq!(rgb(&doc, 50, 50), RED);
}

/// Form 5's `/Resources` omit `/Inner`, which the page names; the page's is
/// drawn.
#[test]
fn name_missing_from_form_resources_resolves_in_the_page() {
    let doc = page(
        "<< /XObject << /Outer 5 0 R /Inner 6 0 R >> >>",
        "/Outer Do",
        vec![
            Stream(&form("<< >>"), "/Inner Do"),
            Stream(&form("<< >>"), FILL_RED),
        ],
    );
    assert_eq!(rgb(&doc, 50, 50), RED);
}

/// Form 5 sets fill alpha 0 through an ExtGState only its `/Resources`
/// name, then fills red.
#[test]
fn form_applies_an_extgstate_only_its_resources_hold() {
    let doc = page(
        "<< /XObject << /Outer 5 0 R >> >>",
        "/Outer Do",
        vec![Stream(
            &form("<< /ExtGState << /G0 << /ca 0 >> >> >>"),
            &format!("/G0 gs {FILL_RED}"),
        )],
    );
    assert_eq!(rgb(&doc, 50, 50), WHITE);
}

/// Form 5 paints a red shading only its `/Resources` name.
#[test]
fn form_paints_a_shading_only_its_resources_hold() {
    let doc = page(
        "<< /XObject << /Outer 5 0 R >> >>",
        "/Outer Do",
        vec![Stream(
            &form(
                "<< /Shading << /Sh0 << /ShadingType 2 /ColorSpace /DeviceRGB \
                 /Coords [0 0 100 0] /Extend [true true] /Function << /FunctionType 2 \
                 /Domain [0 1] /C0 [1 0 0] /C1 [1 0 0] /N 1 >> >> >> >>",
            ),
            "/Sh0 sh",
        )],
    );
    assert_eq!(rgb(&doc, 50, 50), RED);
}

// ---- pattern tiles -------------------------------------------------------

/// The tile draws `/T`, which only the pattern's `/Resources` name.
#[test]
fn tile_draws_a_name_only_the_pattern_resources_hold() {
    let doc = page(
        "<< /Pattern << /P0 5 0 R >> >>",
        "/Pattern cs /P0 scn 0 0 100 100 re f",
        vec![
            Stream(
                "/Type /Pattern /PatternType 1 /PaintType 1 /TilingType 1 \
                 /BBox [0 0 10 10] /XStep 10 /YStep 10 \
                 /Resources << /XObject << /T 6 0 R >> >>",
                "/T Do",
            ),
            Stream(&form("<< >>"), "1 0 0 rg 0 0 5 5 re f"),
        ],
    );
    assert_eq!(rgb(&doc, 2, 7), RED);
    assert_eq!(rgb(&doc, 7, 7), WHITE);
}

/// Pattern 5's tile fills without choosing a colour, so with pattern 5 —
/// the object the page selected — not with the blue pattern 6 that `/P0`
/// names in the tile's own `/Resources`. Pattern 5 is running, so the fill
/// paints the fill colour.
#[test]
fn tile_fill_keeps_the_pattern_object_the_caller_selected() {
    let doc = page(
        "<< /Pattern << /P0 5 0 R >> >>",
        "/Pattern cs /P0 scn 0 0 100 100 re f",
        vec![
            Stream(
                "/Type /Pattern /PatternType 1 /PaintType 1 /TilingType 1 \
                 /BBox [0 0 10 10] /XStep 10 /YStep 10 \
                 /Resources << /Pattern << /P0 6 0 R >> >>",
                "0 0 5 5 re f",
            ),
            Stream(
                "/Type /Pattern /PatternType 1 /PaintType 1 /TilingType 1 \
                 /BBox [0 0 10 10] /XStep 10 /YStep 10 /Resources << >>",
                "0 0 1 rg 0 0 10 10 re f",
            ),
        ],
    );
    assert_eq!(rgb(&doc, 2, 7), BLACK);
    assert_eq!(rgb(&doc, 7, 7), WHITE);
}

// ---- soft mask forms -----------------------------------------------------

/// The soft mask form draws `/W` (white everywhere), which only its own
/// `/Resources` name; the masked red fill shows.
#[test]
fn soft_mask_form_draws_a_name_only_its_resources_hold() {
    let doc = page(
        "<< /ExtGState << /GS0 << /SMask << /S /Luminosity /G 5 0 R >> >> >> >>",
        &format!("/GS0 gs {FILL_RED}"),
        vec![
            Stream(
                &form("<< /XObject << /W 6 0 R >> >> /Group << /S /Transparency >>"),
                "/W Do",
            ),
            Stream(&form("<< >>"), "1 g 0 0 100 100 re f"),
        ],
    );
    assert_eq!(rgb(&doc, 50, 50), RED);
}

/// Form 5 installs a soft mask whose form (6) has no `/Resources` and draws
/// `/W`, which form 5 names white and the page names black; the mask form
/// resolves in the scope the `gs` ran in, and the masked red fill shows.
#[test]
fn soft_mask_form_without_resources_resolves_in_the_gs_scope() {
    let doc = page(
        "<< /XObject << /Outer 5 0 R /W 8 0 R >> >>",
        "/Outer Do",
        vec![
            Stream(
                &form(
                    "<< /ExtGState << /GS0 << /SMask << /S /Luminosity /G 6 0 R >> >> >> \
                     /XObject << /W 7 0 R >> >>",
                ),
                &format!("/GS0 gs {FILL_RED}"),
            ),
            Stream(
                &format!("{BARE_FORM} /Group << /S /Transparency >>"),
                "/W Do",
            ),
            Stream(&form("<< >>"), "1 g 0 0 100 100 re f"),
            Stream(&form("<< >>"), "0 g 0 0 100 100 re f"),
        ],
    );
    assert_eq!(rgb(&doc, 50, 50), RED);
}

// ---- annotation appearances ----------------------------------------------

/// The appearance stream draws `/R`, which only its own `/Resources` name.
#[test]
fn annotation_appearance_draws_a_name_only_its_resources_hold() {
    let appearance = form("<< /XObject << /R 7 0 R >> >>");
    let doc = pdf(&[
        Plain("<< /Type /Catalog /Pages 2 0 R >>"),
        Plain("<< /Type /Pages /Kids [3 0 R] /Count 1 >>"),
        Plain(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] /Resources << >> \
             /Contents 4 0 R /Annots [5 0 R] >>",
        ),
        Stream("", ""),
        Plain("<< /Type /Annot /Subtype /Square /Rect [0 0 100 100] /AP << /N 6 0 R >> >>"),
        Stream(&appearance, "/R Do"),
        Stream(&form("<< >>"), FILL_RED),
    ]);
    assert_eq!(rgb(&doc, 50, 50), RED);
}

// ---- fonts ---------------------------------------------------------------

/// Form 5 shows text in `/F1`, which only its own `/Resources` name.
#[test]
fn form_shows_text_in_a_font_only_its_resources_hold() {
    let resources = format!("<< /Font << /F1 {} >> >>", font("Wide", 1000));
    let doc = page(
        "<< /XObject << /Outer 5 0 R >> >>",
        "/Outer Do",
        vec![Stream(&form(&resources), &format!("/F1 50 Tf {SHOW_A}"))],
    );
    assert_eq!(rgb(&doc, 50, 50), RED);
}

/// `/F1` is a narrow font on the page and a wide one in form 5. The page
/// shows text in its own (y 70–100), form 5 shows text in its own (y 30–70),
/// then the page selects `/F1` again and shows text in its own (y 0–30).
#[test]
fn form_font_wins_over_the_same_page_font_name() {
    let page_resources = format!(
        "<< /XObject << /Outer 5 0 R >> /Font << /F1 {} >> >>",
        font("Narrow", 100)
    );
    let form_resources = format!("<< /Font << /F1 {} >> >>", font("Wide", 1000));
    let doc = page(
        &page_resources,
        "/F1 50 Tf BT 1 0 0 rg 10 0 Td (A) Tj ET /Outer Do          /F1 50 Tf BT 1 0 0 rg 10 80 Td (A) Tj ET",
        vec![Stream(
            &form(&form_resources),
            &format!("/F1 50 Tf {SHOW_A}"),
        )],
    );
    assert_eq!(rgb(&doc, 12, 90), RED);
    assert_eq!(rgb(&doc, 50, 90), WHITE);
    assert_eq!(rgb(&doc, 50, 50), RED);
    assert_eq!(rgb(&doc, 12, 15), RED);
    assert_eq!(rgb(&doc, 50, 15), WHITE);
}

/// The page selects its wide `/F1` and draws form 5, which shows text
/// without `Tf`: the text is in the page's font, not the narrow font `/F1`
/// names in form 5's `/Resources`.
#[test]
fn form_text_keeps_the_font_object_the_caller_selected() {
    let page_resources = format!(
        "<< /XObject << /Outer 5 0 R >> /Font << /F1 {} >> >>",
        font("Wide", 1000)
    );
    let form_resources = format!("<< /Font << /F1 {} >> >>", font("Narrow", 100));
    let doc = page(
        &page_resources,
        "/F1 50 Tf /Outer Do",
        vec![Stream(&form(&form_resources), SHOW_A)],
    );
    assert_eq!(rgb(&doc, 50, 50), RED);
}

// ---- SVG -----------------------------------------------------------------

/// The SVG renderer draws a name only form 5's `/Resources` hold, and
/// prefers it over the page's blue form of the same name.
#[test]
fn svg_form_resolves_names_in_its_resources() {
    let doc = page(
        "<< /XObject << /Outer 5 0 R /Inner 7 0 R >> >>",
        "/Outer Do",
        vec![
            Stream(&form("<< /XObject << /Inner 6 0 R >> >>"), "/Inner Do"),
            Stream(&form("<< >>"), FILL_RED),
            Stream(&form("<< >>"), FILL_BLUE),
        ],
    );
    let svg = render_page_to_svg(&doc, 0).unwrap();
    assert!(svg.contains("rgb(255,0,0)"), "{svg}");
    assert!(!svg.contains("rgb(0,0,255)"), "{svg}");
}

/// The SVG renderer shows the page's text in the page's `/F1` and form 5's
/// text in the `/F1` of form 5's `/Resources`.
#[test]
fn svg_form_text_uses_the_font_in_its_resources() {
    let page_resources = format!(
        "<< /XObject << /Outer 5 0 R >> /Font << /F1 {} >> >>",
        font("Narrow", 100)
    );
    let form_resources = format!("<< /Font << /F1 {} >> >>", font("Wide", 1000));
    let doc = page(
        &page_resources,
        "/F1 50 Tf BT 10 0 Td (A) Tj ET /Outer Do",
        vec![Stream(
            &form(&form_resources),
            &format!("/F1 50 Tf {SHOW_A}"),
        )],
    );
    let svg = render_page_to_svg(&doc, 0).unwrap();
    assert_eq!(svg.matches("font-family=\"Narrow\"").count(), 1, "{svg}");
    assert_eq!(svg.matches("font-family=\"Wide\"").count(), 1, "{svg}");
}
