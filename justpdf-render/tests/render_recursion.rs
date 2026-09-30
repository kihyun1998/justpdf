//! Content-stream recursion in the renderer: pattern tiles, soft mask
//! forms and Form XObjects (#119).

mod common;

use common::{Obj, RED, Stream, WHITE, page, rgb};
use justpdf_core::PdfDocument;
use justpdf_render::render_page_to_svg;

/// Tiling pattern 5: a 10×10 cell whose tile is `tile`.
fn tiling_pattern(tile: &str) -> Obj<'_> {
    Stream(
        "/Type /Pattern /PatternType 1 /PaintType 1 /TilingType 1 \
         /BBox [0 0 10 10] /XStep 10 /YStep 10 /Resources << >>",
        tile,
    )
}

// ---- pattern tiles ------------------------------------------------------

/// The tile sets its own colour and fills; the fill paints that colour
/// rather than the pattern being drawn.
#[test]
fn tile_fill_paints_its_colour_not_the_pattern() {
    let doc = page(
        "<< /Pattern << /P0 5 0 R >> >>",
        "/Pattern cs /P0 scn 0 0 100 100 re f",
        vec![tiling_pattern("1 0 0 rg 0 0 5 5 re f")],
    );
    assert_eq!(rgb(&doc, 2, 7), RED);
    assert_eq!(rgb(&doc, 7, 7), WHITE);
}

/// The tile fills without setting a colour; the fill does not re-enter
/// the pattern.
#[test]
fn tile_fill_without_colour_does_not_reenter_the_pattern() {
    let doc = page(
        "<< /Pattern << /P0 5 0 R >> >>",
        "/Pattern cs /P0 scn 0 0 100 100 re f",
        vec![tiling_pattern("0 0 5 5 re f")],
    );
    assert_ne!(rgb(&doc, 2, 7), WHITE);
    assert_eq!(rgb(&doc, 7, 7), WHITE);
}

/// The tile selects its own pattern again; the inner fill paints the fill
/// colour instead of re-entering the pattern.
#[test]
fn tile_selecting_its_own_pattern_renders() {
    let doc = page(
        "<< /Pattern << /P0 5 0 R >> >>",
        "/Pattern cs /P0 scn 0 0 100 100 re f",
        vec![tiling_pattern("/Pattern cs /P0 scn 0 0 5 5 re f")],
    );
    assert_eq!(rgb(&doc, 2, 7), [0, 0, 0]);
    assert_eq!(rgb(&doc, 7, 7), WHITE);
}

/// A device colour operator after `scn` leaves the Pattern space: the fill
/// is a solid colour, including where the tile is empty.
#[test]
fn device_fill_colour_replaces_the_pattern() {
    for (op, expected) in [("1 0 0 rg", RED), ("1 g", WHITE), ("0 1 1 0 k", RED)] {
        let content = format!("/Pattern cs /P0 scn {op} 0 0 100 100 re f");
        let doc = page(
            "<< /Pattern << /P0 5 0 R >> >>",
            &content,
            vec![tiling_pattern("0 0 1 rg 0 0 5 5 re f")],
        );
        assert_eq!(rgb(&doc, 2, 7), expected, "{op} at the tile's square");
        assert_eq!(rgb(&doc, 7, 7), expected, "{op} outside the tile's square");
    }
}

/// The stroke counterpart: `RG`, `G`, `K` leave the Pattern space.
#[test]
fn device_stroke_colour_replaces_the_pattern() {
    for (op, expected) in [("1 0 0 RG", RED), ("0 G", [0, 0, 0]), ("0 1 1 0 K", RED)] {
        let content = format!("/Pattern CS /P0 SCN {op} 20 w 0 50 m 100 50 l S");
        let doc = page(
            "<< /Pattern << /P0 5 0 R >> >>",
            &content,
            vec![tiling_pattern("0 0 1 rg 0 0 5 5 re f")],
        );
        assert_eq!(rgb(&doc, 7, 47), expected, "{op}");
    }
}

// ---- soft masks ---------------------------------------------------------

/// A pattern fill under a soft mask draws its tile without that mask. The
/// mask form is white on the right half, where the tile's square shows.
#[test]
fn tile_is_drawn_without_the_page_soft_mask() {
    let doc = page(
        "<< /Pattern << /P0 5 0 R >> /ExtGState << /GS0 << /SMask << /S /Luminosity /G 6 0 R >> >> >> >>",
        "/GS0 gs /Pattern cs /P0 scn 0 0 100 100 re f",
        vec![
            tiling_pattern("1 0 0 rg 0 0 5 5 re f"),
            Stream(
                "/Type /XObject /Subtype /Form /BBox [0 0 100 100]",
                "1 g 50 0 50 100 re f",
            ),
        ],
    );
    assert_eq!(rgb(&doc, 52, 7), RED);
}

/// The soft mask form applies the ExtGState that installs it; the inner
/// application is skipped and the masked fill still paints.
#[test]
fn soft_mask_form_applying_its_own_gstate_renders() {
    let doc = page(
        "<< /ExtGState << /GS0 << /SMask << /S /Luminosity /G 5 0 R >> >> >> >>",
        "/GS0 gs 1 0 0 rg 0 0 100 100 re f",
        vec![Stream(
            "/Type /XObject /Subtype /Form /BBox [0 0 100 100]",
            "/GS0 gs 1 g 0 0 100 100 re f",
        )],
    );
    assert_eq!(rgb(&doc, 50, 50), RED);
}

// ---- Form XObjects ------------------------------------------------------

/// Form 5 shifts right by 20, fills a 10×10 square and draws itself; only
/// the first square is painted.
fn self_drawing_form() -> PdfDocument {
    page(
        "<< /XObject << /X0 5 0 R >> >>",
        "/X0 Do",
        vec![Stream(
            "/Type /XObject /Subtype /Form /BBox [0 0 100 100]",
            "1 0 0 1 20 0 cm 1 0 0 rg 0 0 10 10 re f /X0 Do",
        )],
    )
}

#[test]
fn form_drawing_itself_stops_at_the_first_repeat() {
    let doc = self_drawing_form();
    assert_eq!(rgb(&doc, 25, 95), RED);
    assert_eq!(rgb(&doc, 45, 95), WHITE);
}

/// Form 5 draws form 6, which draws form 5 again; the cycle stops when
/// form 5 repeats, after one square.
#[test]
fn form_cycle_through_another_form_stops_at_the_first_repeat() {
    let doc = page(
        "<< /XObject << /X0 5 0 R /X1 6 0 R >> >>",
        "/X0 Do",
        vec![
            Stream(
                "/Type /XObject /Subtype /Form /BBox [0 0 100 100]",
                "1 0 0 1 20 0 cm 1 0 0 rg 0 0 10 10 re f /X1 Do",
            ),
            Stream(
                "/Type /XObject /Subtype /Form /BBox [0 0 100 100]",
                "/X0 Do",
            ),
        ],
    );
    assert_eq!(rgb(&doc, 25, 95), RED);
    assert_eq!(rgb(&doc, 45, 95), WHITE);
}

/// Form 5 draws form 6 twice; the second draw of 6 is not a repeat of an
/// ancestor.
#[test]
fn form_drawn_twice_by_its_parent_is_not_a_cycle() {
    let doc = page(
        "<< /XObject << /X0 5 0 R /X1 6 0 R >> >>",
        "/X0 Do",
        vec![
            Stream(
                "/Type /XObject /Subtype /Form /BBox [0 0 100 100]",
                "/X1 Do 1 0 0 1 20 0 cm /X1 Do",
            ),
            Stream(
                "/Type /XObject /Subtype /Form /BBox [0 0 10 10]",
                "1 0 0 rg 0 0 10 10 re f",
            ),
        ],
    );
    assert_eq!(rgb(&doc, 5, 95), RED);
    assert_eq!(rgb(&doc, 25, 95), RED);
}

#[test]
fn svg_form_drawing_itself_stops_at_the_first_repeat() {
    let svg = render_page_to_svg(&self_drawing_form(), 0).unwrap();
    assert_eq!(svg.matches("<path").count(), 1, "{svg}");
}
