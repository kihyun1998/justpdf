//! A nested content stream (Form XObject, transparency group, soft mask `/G`,
//! pattern tile) cannot change its caller's graphics state: an unmatched `q`
//! or `Q` inside it stays inside, and the caller's path survives it. Soft-mask
//! and transparency-group content starts with blend Normal, alpha 1 and no
//! soft mask (ISO 32000-2 §11.6.6) (#128).

use justpdf_core::PdfDocument;
use justpdf_render::{RenderOptions, render_page_to_pixmap, render_page_to_svg};

/// A PDF whose object `n` (1-based) is `objects[n - 1]`, each a dictionary
/// (or other object) and an optional stream body.
fn pdf(objects: Vec<(String, Option<String>)>) -> PdfDocument {
    let mut out = b"%PDF-1.7\n".to_vec();
    let mut offsets = Vec::new();
    for (i, (dict, stream)) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
        match stream {
            Some(data) => {
                out.extend_from_slice(
                    format!("<< {dict} /Length {} >>\nstream\n", data.len()).as_bytes(),
                );
                out.extend_from_slice(data.as_bytes());
                out.extend_from_slice(b"\nendstream");
            }
            None => out.extend_from_slice(dict.as_bytes()),
        }
        out.extend_from_slice(b"\nendobj\n");
    }
    let xref_at = out.len();
    out.extend_from_slice(
        format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes(),
    );
    for off in offsets {
        out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref_at}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );
    PdfDocument::from_bytes(out).unwrap()
}

/// A 200 × 200 page with `content`, `resources`, and `extra` as objects 5,
/// 6, ….
fn page(content: &str, resources: &str, extra: Vec<(&str, Option<&str>)>) -> PdfDocument {
    let mut objects = vec![
        ("<< /Type /Catalog /Pages 2 0 R >>".to_string(), None),
        (
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
            None,
        ),
        (
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] \
                 /Resources << {resources} >> /Contents 4 0 R >>"
            ),
            None,
        ),
        (String::new(), Some(content.to_string())),
    ];
    for (dict, body) in extra {
        objects.push((dict.to_string(), body.map(str::to_string)));
    }
    pdf(objects)
}

/// The RGB of the pixel at page point (`x`, `y`), a pixel per point.
fn rgb(doc: &PdfDocument, x: u32, y: u32) -> [u8; 3] {
    let px = render_page_to_pixmap(doc, 0, &RenderOptions::default()).unwrap();
    let i = (((px.height - 1 - y) * px.width + x) * 4) as usize;
    [px.data[i], px.data[i + 1], px.data[i + 2]]
}

const BLUE: [u8; 3] = [0, 0, 255];
const WHITE: [u8; 3] = [255, 255, 255];

/// Form object 5 as `/X0`, with `extra` dictionary entries and `body`.
fn with_form(content: &str, form_extra: &str, body: &str) -> PdfDocument {
    let dict = format!("/Type /XObject /Subtype /Form /BBox [0 0 200 200] {form_extra}");
    page(
        content,
        "/XObject << /X0 5 0 R >>",
        vec![(&*dict.leak(), Some(body))],
    )
}

/// A red tiling pattern `/P0` (object 5) whose tile content is `tile`.
fn with_tile(content: &str, tile: &str) -> PdfDocument {
    page(
        content,
        "/Pattern << /P0 5 0 R >>",
        vec![(
            "/Type /Pattern /PatternType 1 /PaintType 1 /TilingType 1 \
             /BBox [0 0 10 10] /XStep 10 /YStep 10 /Resources << >>",
            Some(tile),
        )],
    )
}

// ---- the floor ------------------------------------------------------------

#[test]
fn an_unmatched_q_in_a_form_stays_in_the_form() {
    let doc = with_form(
        "/X0 Do 0 0 1 rg 0 0 10 200 re f",
        "/Matrix [1 0 0 1 50 0]",
        "q",
    );
    assert_eq!(rgb(&doc, 5, 100), BLUE);
    assert_eq!(rgb(&doc, 55, 100), WHITE);
}

#[test]
fn an_unmatched_q_close_in_a_form_cannot_pop_the_callers_state() {
    let doc = with_form(
        "q 1 0 0 1 50 0 cm q /X0 Do Q 0 0 1 rg 0 0 10 200 re f Q",
        "",
        "Q Q",
    );
    assert_eq!(rgb(&doc, 55, 100), BLUE);
    assert_eq!(rgb(&doc, 5, 100), WHITE);
}

#[test]
fn an_unmatched_q_in_a_tile_stays_in_the_tile() {
    let doc = with_tile(
        "1 0 0 1 50 0 cm /Pattern cs /P0 scn 0 0 20 20 re f 0 0 1 rg 0 100 10 100 re f",
        "q 1 0 0 rg 0 0 5 5 re f",
    );
    assert_eq!(rgb(&doc, 55, 150), BLUE);
    assert_eq!(rgb(&doc, 5, 150), WHITE);
}

#[test]
fn a_form_that_fails_part_way_leaves_the_callers_state() {
    // The form shifts by 50 inside an unmatched q, then names an ExtGState
    // whose object (6) does not parse, which stops its content with an error.
    // The page's Q after the Do restores the page's own q: no shift.
    let doc = page(
        "q 1 0 0 1 50 0 cm /X0 Do Q 0 0 1 rg 0 0 10 200 re f",
        "/XObject << /X0 5 0 R >>",
        vec![
            (
                "/Type /XObject /Subtype /Form /BBox [0 0 200 200] \
                 /Resources << /ExtGState << /Bad 6 0 R >> >>",
                Some("q 1 0 0 1 50 0 cm /Bad gs"),
            ),
            ("<< /A", None),
        ],
    );
    assert_eq!(rgb(&doc, 5, 100), BLUE);
    assert_eq!(rgb(&doc, 55, 100), WHITE);
}

#[test]
fn the_path_kept_by_b_survives_a_pattern_fill() {
    // Fill with the pattern, then stroke blue with the same path.
    let doc = with_tile(
        "/Pattern cs /P0 scn 0 0 1 RG 20 w 20 20 160 160 re B",
        "1 0 0 rg 0 0 5 5 re f",
    );
    assert_eq!(rgb(&doc, 25, 100), BLUE, "the stroke on the left edge");
    // The tile paints only its 5 × 5 square of each 10 × 10 cell: the
    // page's path does not reach the tile's own `f`.
    assert_eq!(rgb(&doc, 102, 102), [255, 0, 0]);
    assert_eq!(rgb(&doc, 107, 107), WHITE);
}

#[test]
fn a_tile_starts_with_no_current_path() {
    // `B` keeps the page path [6 6 3 3] for its stroke while the tile runs.
    // The path lies inside the first 10 × 10 cell, and the tile fills only
    // [0 0 5 5], so inside the page path the pattern shows the cell's gap:
    // white, not red.
    let doc = with_tile(
        "/Pattern cs /P0 scn 0.1 w 6 6 3 3 re B",
        "1 0 0 rg 0 0 5 5 re f",
    );
    assert_eq!(rgb(&doc, 7, 7), WHITE);
}

// ---- mask and group forms start reset --------------------------------------

/// `/GS` sets `extra`; the page fills red with it.
fn red_under(gs: &str, extra: Vec<(&str, Option<&str>)>) -> [u8; 3] {
    let doc = page(
        "/GS gs 1 0 0 rg 0 0 200 200 re f",
        &format!("/ExtGState << /GS << {gs} >> >>"),
        extra,
    );
    rgb(&doc, 100, 100)
}

/// A luminosity soft-mask form (object 5) whose content is `body`.
fn mask_form(body: &'static str) -> (&'static str, Option<&'static str>) {
    (
        "/Type /XObject /Subtype /Form /BBox [0 0 200 200] \
         /Group << /S /Transparency /CS /DeviceGray >>",
        Some(body),
    )
}

#[test]
fn a_soft_mask_form_is_drawn_without_the_callers_alpha() {
    let got = red_under(
        "/ca 0.5 /SMask << /S /Luminosity /G 5 0 R >>",
        vec![mask_form("1 g 0 0 200 200 re f")],
    );
    assert_eq!(got, [255, 128, 128]);
}

#[test]
fn a_second_soft_mask_is_drawn_without_the_first() {
    // /L lets the left half through; /F, set while /L is current, lets the
    // whole page through. The fill under /F paints the right half too.
    let doc = page(
        "/L gs /F gs 1 0 0 rg 0 0 200 200 re f",
        "/ExtGState << /L << /SMask << /S /Luminosity /G 5 0 R >> >> \
         /F << /SMask << /S /Luminosity /G 6 0 R >> >> >>",
        vec![
            mask_form("0 g 0 0 200 200 re f 1 g 0 0 100 200 re f"),
            mask_form("1 g 0 0 200 200 re f"),
        ],
    );
    assert_eq!(rgb(&doc, 175, 100), [255, 0, 0]);
    assert_eq!(rgb(&doc, 25, 100), [255, 0, 0]);
}

#[test]
fn a_group_is_composited_once_with_the_callers_alpha() {
    let doc = with_form(
        "/GS gs /X0 Do",
        "/Group << /S /Transparency >>",
        "1 0 0 rg 0 0 200 200 re f",
    );
    let doc = page_resources_with_gs(doc);
    assert_eq!(rgb(&doc, 100, 100), [255, 128, 128]);
}

#[test]
fn a_group_is_composited_through_the_callers_soft_mask() {
    // The soft mask at Do lets only the right half through. Inside the group
    // it is None, and the group's result is masked once when composited.
    let doc = page(
        "/M gs /X0 Do",
        "/ExtGState << /M << /SMask << /S /Luminosity /G 5 0 R >> >> >> \
         /XObject << /X0 6 0 R >>",
        vec![
            mask_form("0 g 0 0 200 200 re f 1 g 100 0 100 200 re f"),
            (
                "/Type /XObject /Subtype /Form /BBox [0 0 200 200] \
                 /Group << /S /Transparency >>",
                Some("1 0 0 rg 0 0 200 200 re f"),
            ),
        ],
    );
    assert_eq!(rgb(&doc, 175, 100), [255, 0, 0]);
    assert_eq!(rgb(&doc, 25, 100), WHITE);
}

/// `doc` rebuilt with `/GS << /ca 0.5 >>` beside its `/X0`.
fn page_resources_with_gs(_doc: PdfDocument) -> PdfDocument {
    page(
        "/GS gs /X0 Do",
        "/ExtGState << /GS << /ca 0.5 >> >> /XObject << /X0 5 0 R >>",
        vec![(
            "/Type /XObject /Subtype /Form /BBox [0 0 200 200] \
             /Group << /S /Transparency >>",
            Some("1 0 0 rg 0 0 200 200 re f"),
        )],
    )
}

// ---- SVG ---------------------------------------------------------------------

/// The `transform` of the SVG's only blue path.
fn blue_path_transform(doc: &PdfDocument) -> String {
    let svg = render_page_to_svg(doc, 0).unwrap();
    let line = svg
        .lines()
        .find(|l| l.contains("fill=\"rgb(0,0,255)\""))
        .unwrap_or_else(|| panic!("no blue path in {svg}"));
    let start = line.find("transform=\"").unwrap() + "transform=\"".len();
    line[start..start + line[start..].find('"').unwrap()].to_string()
}

#[test]
fn svg_an_unmatched_q_in_a_form_stays_in_the_form() {
    let doc = with_form(
        "/X0 Do 0 0 1 rg 0 0 10 200 re f",
        "/Matrix [1 0 0 1 50 0]",
        "q",
    );
    assert_eq!(blue_path_transform(&doc), "matrix(1,0,0,-1,0,200)");
}

#[test]
fn svg_an_unmatched_q_close_in_a_form_cannot_pop_the_callers_state() {
    let doc = with_form(
        "q 1 0 0 1 50 0 cm q /X0 Do Q 0 0 1 rg 0 0 10 200 re f Q",
        "",
        "Q Q",
    );
    assert_eq!(blue_path_transform(&doc), "matrix(1,0,0,-1,50,200)");
}
