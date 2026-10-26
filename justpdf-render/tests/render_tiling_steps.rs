//! A tiling pattern takes any finite positive step: `/XStep` and `/YStep` are
//! in pattern space, which `/Matrix` scales (#402).

mod common;

use common::{RED, Stream, WHITE, page, rgb};

/// A page filled with tiling pattern `/T` (object 5) of steps `step` and
/// matrix `matrix`, whose cell has a red quarter-step square at its origin,
/// over a blue fill colour set before the pattern.
fn tiled(step: &str, bbox: &str, matrix: &str, cell: &str) -> justpdf_core::PdfDocument {
    let pattern = format!(
        "/PatternType 1 /PaintType 1 /TilingType 1 /BBox {bbox} \
         /XStep {step} /YStep {step} /Matrix {matrix} /Resources << >>"
    );
    page(
        "<< /Pattern << /T 5 0 R >> >>",
        "0 0 1 rg /Pattern cs /T scn 0 0 100 100 re f",
        vec![Stream(&pattern, cell)],
    )
}

/// RGB at user point (x, y) of the page at 72 dpi.
fn at(doc: &justpdf_core::PdfDocument, x: u32, y: u32) -> [u8; 3] {
    rgb(doc, x, 100 - y)
}

#[test]
fn a_step_below_one_unit_is_tiled() {
    // Steps of 0.5 under a matrix of 40: a 20 pt cell with a 10 pt square.
    let doc = tiled(
        "0.5",
        "[0 0 0.5 0.5]",
        "[40 0 0 40 0 0]",
        "1 0 0 rg 0 0 0.25 0.25 re f",
    );
    assert_eq!(at(&doc, 5, 5), RED);
    assert_eq!(at(&doc, 15, 5), WHITE);
    assert_eq!(at(&doc, 5, 15), WHITE);
    assert_eq!(at(&doc, 25, 25), RED);
}

#[test]
fn a_negative_step_below_one_unit_is_tiled_by_its_size() {
    let doc = tiled(
        "-0.5",
        "[0 0 0.5 0.5]",
        "[40 0 0 40 0 0]",
        "1 0 0 rg 0 0 0.25 0.25 re f",
    );
    assert_eq!(at(&doc, 5, 5), RED);
    assert_eq!(at(&doc, 15, 5), WHITE);
}

#[test]
fn a_zero_step_is_not_a_pattern() {
    let doc = tiled(
        "0",
        "[0 0 20 20]",
        "[1 0 0 1 0 0]",
        "1 0 0 rg 0 0 10 10 re f",
    );
    // Rejected: the fill falls back to the colour set before the pattern.
    assert_eq!(at(&doc, 5, 5), [0, 0, 255]);
    assert_eq!(at(&doc, 15, 15), [0, 0, 255]);
}
