//! A tiling pattern's cell is its `/BBox`, clipped to it and repeated every
//! `/XStep` and `/YStep`, wherever the box lies and whatever its size (#399).

mod common;

use common::{Stream, page};
use justpdf_render::{RenderOptions, render_page_to_pixmap};

const RED: [u8; 3] = [255, 0, 0];
const WHITE: [u8; 3] = [255, 255, 255];

/// A page filled with tiling pattern `/T` (object 5) with `/BBox` `bbox`, a
/// step of 20 both ways, and cell content `cell`.
fn tiled(bbox: &str, cell: &str) -> justpdf_core::PdfDocument {
    let pattern = format!(
        "/PatternType 1 /PaintType 1 /TilingType 1 /BBox {bbox} \
         /XStep 20 /YStep 20 /Resources << >>"
    );
    page(
        "<< /Pattern << /T 5 0 R >> >>",
        "/Pattern cs /T scn 0 0 100 100 re f",
        vec![Stream(&pattern, cell)],
    )
}

/// RGB at user point (x, y) of a 100 × 100 pt page rendered at `dpi`.
fn at(doc: &justpdf_core::PdfDocument, dpi: f64, x: f64, y: f64) -> [u8; 3] {
    let opts = RenderOptions {
        dpi,
        ..Default::default()
    };
    let px = render_page_to_pixmap(doc, 0, &opts).unwrap();
    let s = dpi / 72.0;
    let (dx, dy) = ((x * s) as u32, ((100.0 - y) * s) as u32);
    let i = ((dy * px.width + dx) * 4) as usize;
    [px.data[i], px.data[i + 1], px.data[i + 2]]
}

/// Red exactly where `red` says, at each of `points`, at 72 and 144 dpi.
fn marks(doc: &justpdf_core::PdfDocument, points: &[(f64, f64, bool)]) {
    for dpi in [72.0, 144.0] {
        for &(x, y, red) in points {
            let want = if red { RED } else { WHITE };
            assert_eq!(at(doc, dpi, x, y), want, "({x}, {y}) at {dpi} dpi");
        }
    }
}

#[test]
fn a_cell_below_and_left_of_the_origin_is_drawn() {
    // A red 10 × 10 square centred on the pattern origin, repeated every 20.
    let doc = tiled("[-5 -5 5 5]", "1 0 0 rg -5 -5 10 10 re f");
    marks(
        &doc,
        &[
            (2.0, 2.0, true),
            (18.0, 18.0, true),
            (18.0, 2.0, true),
            (2.0, 18.0, true),
            (38.0, 42.0, true),
            (10.0, 10.0, false),
            (30.0, 50.0, false),
        ],
    );
}

#[test]
fn a_cell_larger_than_the_step_overlaps_its_neighbours() {
    // The box is 30 wide; its only mark lies at x 20..30, past the step, so
    // every copy puts it 20 further right: x 0..10, 20..30, 40..50, ...
    let doc = tiled("[0 0 30 10]", "1 0 0 rg 20 0 10 5 re f");
    marks(
        &doc,
        &[
            (5.0, 2.0, true),
            (15.0, 2.0, false),
            (45.0, 2.0, true),
            (45.0, 22.0, true),
            (5.0, 8.0, false),
        ],
    );
}

#[test]
fn a_cell_smaller_than_the_step_is_clipped_to_its_box() {
    // The content fills the whole step; the box keeps 10 × 10 of it.
    let doc = tiled("[0 0 10 10]", "1 0 0 rg 0 0 20 20 re f");
    marks(
        &doc,
        &[
            (5.0, 5.0, true),
            (15.0, 5.0, false),
            (5.0, 15.0, false),
            (25.0, 25.0, true),
            (35.0, 35.0, false),
        ],
    );
}

#[test]
fn a_box_offset_inside_the_step_lands_at_its_offset() {
    // Box 5..15 × 5..15, content filling it: red at 5..15 of each cell.
    let doc = tiled("[5 5 15 15]", "1 0 0 rg 0 0 20 20 re f");
    marks(
        &doc,
        &[
            (10.0, 10.0, true),
            (2.0, 10.0, false),
            (18.0, 10.0, false),
            (30.0, 30.0, true),
        ],
    );
}
