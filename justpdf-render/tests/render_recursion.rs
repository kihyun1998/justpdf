//! Content-stream recursion in the renderer: pattern tiles, soft mask
//! forms and Form XObjects (#119).

use justpdf_core::PdfDocument;
use justpdf_render::{RenderOptions, render_page_to_pixmap, render_page_to_svg};

/// One object body of a test PDF: a plain object or a stream.
enum Obj<'a> {
    Plain(&'a str),
    Stream(&'a str, &'a str),
}
use Obj::{Plain, Stream};

/// Builds a PDF whose object `n` (1-based) is `objects[n - 1]`, with a
/// correct xref table and `/Root 1 0 R`.
fn pdf(objects: &[Obj]) -> PdfDocument {
    let mut out = b"%PDF-1.7\n".to_vec();
    let mut offsets = Vec::new();
    for (i, obj) in objects.iter().enumerate() {
        offsets.push(out.len());
        let body = match obj {
            Plain(body) => body.to_string(),
            Stream(dict, content) => format!(
                "<< {dict} /Length {} >>\nstream\n{content}\nendstream",
                content.len()
            ),
        };
        out.extend_from_slice(format!("{} 0 obj\n{body}\nendobj\n", i + 1).as_bytes());
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

/// A 100×100 page: catalog 1, pages 2, page 3 with `resources`, content 4.
/// Further objects start at 5.
fn page(resources: &str, content: &str, rest: Vec<Obj>) -> PdfDocument {
    let page_dict = format!(
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] /Resources {resources} /Contents 4 0 R >>"
    );
    let mut objects = vec![
        Plain("<< /Type /Catalog /Pages 2 0 R >>"),
        Plain("<< /Type /Pages /Kids [3 0 R] /Count 1 >>"),
        Plain(&page_dict),
        Stream("", content),
    ];
    objects.extend(rest);
    pdf(&objects)
}

/// RGB of device pixel (x, y) at 72 dpi; y counts down from the top.
fn rgb(doc: &PdfDocument, x: u32, y: u32) -> [u8; 3] {
    let px = render_page_to_pixmap(doc, 0, &RenderOptions::default()).unwrap();
    let i = ((y * px.width + x) * 4) as usize;
    [px.data[i], px.data[i + 1], px.data[i + 2]]
}

const RED: [u8; 3] = [255, 0, 0];
const WHITE: [u8; 3] = [255, 255, 255];

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
